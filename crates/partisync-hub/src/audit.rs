//! Hub 审计日志（M8-WP03-T01，SPEC M8-WP03 §2.1）。
//!
//! 形态：append-only JSONL（`r-audit` keyspace，key = BE seq）+ 逐行
//! 链式哈希（`hash = blake3(seq|ts|actor|action|object|result|prev)`，
//! genesis prev = 全零 hex）。写入经**有界队列异步 sink**——队满语义 =
//! **阻塞写**（std `SyncSender` 原生语义，不丢审计；裁定见任务卡）；
//! apply 内核只 `record()`，持久化在独立写协程。
//!
//! 诚实边界：apply ACK 与审计持久化之间有窄窗口（异步 sink 契约），
//! crash 丢尾部未刷行时业务态已提交而审计缺失——完整对账归 export 期
//! verify 与 M9 外部审计议题。actor v0.1 = hub 节点（`hub-{node_id}`），
//! 用户/设备级 actor 随租户面（M8-WP03-T03）演化。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// 审计 keyspace（key = seq BE 8B，值 = JSONL 行）。
pub const KS_AUDIT: &str = "r-audit";
/// 链 genesis prev_hash。
pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// 审计行（链式；`hash` 覆盖前七字段）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuditRow {
    /// 单调序号（跨重启连续）。
    pub seq: u64,
    /// 毫秒时间戳。
    pub ts_ms: u64,
    /// 执行方（v0.1 = `hub-{node_id}`）。
    pub actor: String,
    /// 动作（put/remove/rename/shadow_upsert/…）。
    pub action: String,
    /// 对象（entry id hex / 空间 id / 行数）。
    pub object: String,
    /// 结果（ok / entry_missing / exists / …）。
    pub result: String,
    /// 前行哈希（genesis = 全零）。
    pub prev_hash: String,
    /// 本行哈希。
    pub hash: String,
}

impl AuditRow {
    /// 复算本行哈希（导出侧独立校验用）。
    pub fn compute_hash(&self) -> String {
        let preimage = format!(
            "{}|{}|{}|{}|{}|{}|{}",
            self.seq, self.ts_ms, self.actor, self.action, self.object, self.result, self.prev_hash
        );
        blake3::hash(preimage.as_bytes()).to_hex().to_string()
    }
}

fn seq_key(seq: u64) -> [u8; 8] {
    seq.to_be_bytes()
}

/// 生成/消费共享计数（flush 用：produced == persisted 即刷净）。
struct AuditCounters {
    produced: AtomicU64,
    persisted: AtomicU64,
}

/// 生产端链状态（单把锁串行化两 raft 组的 apply 线程）。
struct ChainState {
    seq: u64,
    prev_hash: String,
}

/// 审计 sink：apply 侧 `record()` → 有界队列 → 写协程落盘 `r-audit`。
pub struct AuditSink {
    tx: SyncSender<AuditRow>,
    counters: Arc<AuditCounters>,
    state: Mutex<ChainState>,
    actor: String,
}

impl AuditSink {
    /// 打开 sink（默认队列容量 1024）：从 keyspace 现有尾部恢复链状态
    /// （跨重启 seq 连续）。
    ///
    /// # Errors
    /// keyspace 打开或尾部读取失败。
    pub fn new(db: &fjall::Database, actor: String) -> fjall::Result<Arc<Self>> {
        Self::with_capacity(db, actor, 1024)
    }

    /// 指定队列容量打开（测试注入小容量）。
    ///
    /// # Errors
    /// 同 [`Self::new`]。
    pub fn with_capacity(
        db: &fjall::Database,
        actor: String,
        capacity: usize,
    ) -> fjall::Result<Arc<Self>> {
        let ks = db.keyspace(KS_AUDIT, fjall::KeyspaceCreateOptions::default)?;
        // 链状态恢复：BE seq 序下最后一行
        let mut state = ChainState {
            seq: 0,
            prev_hash: GENESIS.to_owned(),
        };
        let mut persisted = 0u64;
        for guard in ks.iter() {
            let (_, v) = guard.into_inner()?;
            persisted += 1;
            if let Ok(row) = serde_json::from_slice::<AuditRow>(&v) {
                state.seq = row.seq;
                state.prev_hash = row.hash;
            }
        }
        let counters = Arc::new(AuditCounters {
            produced: AtomicU64::new(persisted),
            persisted: AtomicU64::new(persisted),
        });
        let (tx, rx) = sync_channel::<AuditRow>(capacity);
        let shared = Arc::clone(&counters);
        let writer_ks = ks.clone();
        std::thread::spawn(move || {
            for row in rx {
                if let Ok(json) = serde_json::to_vec(&row) {
                    let _ = writer_ks.insert(seq_key(row.seq), json);
                }
                shared.persisted.fetch_add(1, Ordering::SeqCst);
            }
        });
        Ok(Arc::new(Self {
            tx,
            counters,
            state: Mutex::new(state),
            actor,
        }))
    }

    /// 记录一条审计（队满时阻塞——不丢审计；写协程死亡时 eprintln 降级）。
    pub fn record(&self, action: &str, object: &str, result: &str) {
        let mut state = self.state.lock().expect("audit chain poisoned");
        let seq = state.seq + 1;
        let ts_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let mut row = AuditRow {
            seq,
            ts_ms,
            actor: self.actor.clone(),
            action: action.to_owned(),
            object: object.to_owned(),
            result: result.to_owned(),
            prev_hash: state.prev_hash.clone(),
            hash: String::new(),
        };
        row.hash = row.compute_hash();
        let prev = row.hash.clone();
        match self.tx.send(row) {
            Ok(()) => {
                state.seq = seq;
                state.prev_hash = prev;
                self.counters.produced.fetch_add(1, Ordering::SeqCst);
            }
            Err(e) => {
                // 写协程已死（fatal 路径）：审计降级为 stderr，不阻塞 apply
                eprintln!("audit sink dead, row dropped: {e:?}");
            }
        }
    }

    /// 等待 produced == persisted（刷净）。超时返回 false。
    pub fn flush(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            let p = self.counters.persisted.load(Ordering::SeqCst);
            if p == self.counters.produced.load(Ordering::SeqCst) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        self.counters.persisted.load(Ordering::SeqCst)
            == self.counters.produced.load(Ordering::SeqCst)
    }
}

/// 链校验（从 keyspace 全量重算）：seq 连续、hash 逐行复算、prev 衔接。
///
/// # Errors
/// 任一行被篡改/删除/乱序，或引擎读取失败。
pub fn verify(db: &fjall::Database) -> Result<(u64, String), String> {
    let ks = db
        .keyspace(KS_AUDIT, fjall::KeyspaceCreateOptions::default)
        .map_err(|e| e.to_string())?;
    let mut expect_seq = 1u64;
    let mut prev = GENESIS.to_owned();
    let mut count = 0u64;
    let mut tail = GENESIS.to_owned();
    for guard in ks.iter() {
        let (_, v) = guard.into_inner().map_err(|e| e.to_string())?;
        let row: AuditRow = serde_json::from_slice(&v).map_err(|e| format!("row decode: {e}"))?;
        if row.seq != expect_seq {
            return Err(format!(
                "seq gap/overflow: expect {expect_seq}, got {}",
                row.seq
            ));
        }
        if row.prev_hash != prev {
            return Err(format!("chain break at seq {}: prev mismatch", row.seq));
        }
        if row.hash != row.compute_hash() {
            return Err(format!("hash mismatch at seq {} (row tampered)", row.seq));
        }
        prev = row.hash.clone();
        tail = row.hash;
        expect_seq += 1;
        count += 1;
    }
    Ok((count, tail))
}

/// 导出信息（链头链尾可独立复算）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditExportInfo {
    /// 行数。
    pub rows: u64,
    /// 链头（首行 hash；空链 = genesis）。
    pub head: String,
    /// 链尾（末行 hash）。
    pub tail: String,
}

/// 导出 audit-ready JSONL（先全量链校验，再逐行写文件）。
///
/// # Errors
/// 链校验失败或文件写入失败。
pub fn export(db: &fjall::Database, path: &std::path::Path) -> Result<AuditExportInfo, String> {
    let (rows, tail) = verify(db)?;
    let head = if rows == 0 {
        GENESIS.to_owned()
    } else {
        let ks = db
            .keyspace(KS_AUDIT, fjall::KeyspaceCreateOptions::default)
            .map_err(|e| e.to_string())?;
        let first = ks
            .iter()
            .next()
            .ok_or("empty audit log")?
            .into_inner()
            .map_err(|e| e.to_string())?;
        let row: AuditRow = serde_json::from_slice(&first.1).map_err(|e| e.to_string())?;
        row.hash
    };
    let jsonl = {
        let ks = db
            .keyspace(KS_AUDIT, fjall::KeyspaceCreateOptions::default)
            .map_err(|e| e.to_string())?;
        let mut out = String::new();
        for guard in ks.iter() {
            let (_, v) = guard.into_inner().map_err(|e| e.to_string())?;
            out.push_str(&String::from_utf8_lossy(&v));
            out.push('\n');
        }
        out
    };
    std::fs::write(path, jsonl).map_err(|e| format!("write export: {e}"))?;
    Ok(AuditExportInfo { rows, head, tail })
}
