//! 修复限流与修复队列（SPEC M3-WP04 §3 裁定 3）。
//!
//! 两件事：
//!
//! 1. [`RepairLimiter`] —— 令牌桶；后台修复读取/写入走 `acquire(bytes).await`
//!    限流，**≤10% 估计带宽**（SPEC 验收「修复限流」），
//!    桶容量 = 1 秒配额（允许瞬时短突发）；
//! 2. [`RepairQueue`] —— 持久化修复任务队列（fjall `m-pack-repair`
//!    keyspace）；`enqueue` / `take_due` / `mark_done` / `mark_failed`
//!    —— 重启可续跑（SPEC 验收「重启续跑」）。
//!
//! 设计：限流与队列均 **无锁**（&self 内部状态只经 `acquire().await`
//! 串行访问 + fjall 自身持久化）；多消费者并发 `take_due` 由 fjall
//! 串行化保证。

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use fjall::{Database, Keyspace, KeyspaceCreateOptions, PersistMode};
use serde::{Deserialize, Serialize};

/// 默认桶容量：1 秒配额（短突发吸收）。
pub const DEFAULT_BUCKET_CAPACITY_SECS: u64 = 1;

/// 默认失败回退：指数退避起点 1s，最大 5min。
pub const DEFAULT_BACKOFF_BASE_NS: u64 = 1_000_000_000;
pub const DEFAULT_BACKOFF_MAX_NS: u64 = 300_000_000_000;

/// 修复队列 keyspace 名（fjall keyspace 共享前缀约定，见 hub）。
pub const KS_REPAIR: &str = "m-pack-repair";

/// 修复统一错误。
#[derive(Debug)]
pub enum RepairError {
    /// fjall 底层错误。
    Fjall(fjall::Error),
    /// JSON 序列化/反序列化错误。
    Serialize(serde_json::Error),
}

impl core::fmt::Display for RepairError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Fjall(e) => write!(f, "repair storage: {e}"),
            Self::Serialize(e) => write!(f, "repair serialize: {e}"),
        }
    }
}

impl std::error::Error for RepairError {}

impl From<fjall::Error> for RepairError {
    fn from(e: fjall::Error) -> Self {
        Self::Fjall(e)
    }
}

impl From<serde_json::Error> for RepairError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialize(e)
    }
}

/// 平面结果别名。
pub type Result<T> = std::result::Result<T, RepairError>;

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

// ============================================================================
// 令牌桶限流器
// ============================================================================

/// 令牌桶状态（单调时间）。
struct LimiterState {
    tokens: f64,
    last_refill_ns: u64,
}

/// 修复限流器（令牌桶）：预算 = 每秒字节数；容量 = 1 秒配额。
pub struct RepairLimiter {
    state: Mutex<LimiterState>,
    rate_per_ns: f64,
    capacity: f64,
}

impl RepairLimiter {
    /// 新建限流器；`budget_bytes_per_sec` = 修复总带宽上限（如磁盘
    /// 估计带宽的 10%）。`capacity_bytes` 默认 1 秒配额。
    #[must_use]
    pub fn new(budget_bytes_per_sec: u64) -> Self {
        let capacity = budget_bytes_per_sec as f64 * DEFAULT_BUCKET_CAPACITY_SECS as f64;
        Self::with_capacity(budget_bytes_per_sec, capacity as u64)
    }

    /// 自定义桶容量（测试用：可缩短到毫秒级以加速验证）。
    #[must_use]
    pub fn with_capacity(budget_bytes_per_sec: u64, capacity_bytes: u64) -> Self {
        let rate_per_ns = budget_bytes_per_sec as f64 / 1_000_000_000.0;
        Self {
            state: Mutex::new(LimiterState {
                tokens: capacity_bytes as f64,
                last_refill_ns: now_ns(),
            }),
            rate_per_ns,
            capacity: capacity_bytes as f64,
        }
    }

    fn refill(state: &mut LimiterState, rate_per_ns: f64, capacity: f64, now: u64) {
        if now > state.last_refill_ns {
            let elapsed_ns = (now - state.last_refill_ns) as f64;
            let new_tokens = state.tokens + elapsed_ns * rate_per_ns;
            state.tokens = new_tokens.min(capacity);
            state.last_refill_ns = now;
        }
    }

    /// 尝试获取 `n` 字节令牌；够则扣减并返回 `true`，否则 `false`。
    pub fn try_acquire(&self, n: u64) -> bool {
        let mut state = self.state.lock().expect("limiter mutex");
        Self::refill(&mut state, self.rate_per_ns, self.capacity, now_ns());
        if state.tokens >= n as f64 {
            state.tokens -= n as f64;
            true
        } else {
            false
        }
    }

    /// 阻塞获取 `n` 字节令牌（每 10ms 轮询一次）。
    pub async fn acquire(&self, n: u64) {
        loop {
            if self.try_acquire(n) {
                return;
            }
            // 估算还需等待：缺额 / 速率 → 转 ms（最小 10ms 防止热循环）
            let wait_ns = {
                let state = self.state.lock().expect("limiter mutex");
                let need = (n as f64 - state.tokens).max(0.0);
                ((need / self.rate_per_ns) * 1_000_000.0) as u64 + 10_000_000
            };
            tokio::time::sleep(std::time::Duration::from_nanos(wait_ns)).await;
        }
    }
}

// ============================================================================
// 持久化修复队列
// ============================================================================

/// 单条修复任务。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairItem {
    /// 所属 pack id（hex hash of pack bytes；当前用 build_pack 内容的 hash）。
    pub pack_id: String,
    /// 缺失分片号 0..TOTAL_SHARDS（去重视为 pack 范围）。
    pub shard_idx: u8,
    /// 已尝试次数。
    pub retry_count: u32,
    /// 上次尝试 unix 纳秒；0 = 从未尝试。
    pub last_attempt_ns: u64,
    /// 当前退避（unix 纳秒）；下次允许时间 = last_attempt + backoff。
    pub backoff_ns: u64,
}

fn item_key(pack_id: &str, shard_idx: u8) -> Vec<u8> {
    let mut k = Vec::with_capacity(pack_id.len() + 2);
    k.extend_from_slice(pack_id.as_bytes());
    k.push(b':');
    k.push(shard_idx);
    k
}

/// 修复队列（fjall 持久化）。
pub struct RepairQueue {
    db: Database,
    ks: Keyspace,
}

impl RepairQueue {
    /// 打开（或创建）位于 `root` 的修复队列。
    ///
    /// # Errors
    /// fjall 打开或 keyspace 创建失败。
    pub fn open(root: &Path) -> Result<Self> {
        let db = Database::open(fjall::Config::new(root))?;
        let ks = db.keyspace(KS_REPAIR, KeyspaceCreateOptions::default)?;
        Ok(Self { db, ks })
    }

    /// 挂接到既有 Database（与 hub 共享 fjall 路径排他锁模型对齐）。
    ///
    /// # Errors
    /// keyspace 创建失败。
    pub fn attach(db: Database) -> Result<Self> {
        let ks = db.keyspace(KS_REPAIR, KeyspaceCreateOptions::default)?;
        Ok(Self { db, ks })
    }

    /// 显式落盘（验收口径）。
    ///
    /// # Errors
    /// fjall 刷盘失败。
    pub fn persist(&self) -> Result<()> {
        self.db.persist(PersistMode::SyncData)?;
        Ok(())
    }

    /// 入队一条修复任务（已存在则覆盖——幂等）。
    ///
    /// # Errors
    /// fjall 序列化或写入失败。
    pub fn enqueue(&self, item: &RepairItem) -> Result<()> {
        let key = item_key(&item.pack_id, item.shard_idx);
        let val = serde_json::to_vec(item)?;
        self.ks.insert(key, val)?;
        Ok(())
    }

    /// 取最多 `limit` 条到期任务（last_attempt + backoff ≤ now）。
    /// 返回的任务保持 fjall 中的位置；调用方处理成功后必须 `mark_done`
    /// 或失败 `mark_failed`（更新 backoff）以推进。
    ///
    /// # Errors
    /// fjall 迭代或反序列化失败。
    pub fn take_due(&self, limit: usize, now: u64) -> Result<Vec<RepairItem>> {
        let mut out = Vec::with_capacity(limit);
        for guard in self.ks.iter() {
            if out.len() >= limit {
                break;
            }
            let v = guard.value()?;
            let bytes: &[u8] = v.as_ref();
            let item: RepairItem = serde_json::from_slice(bytes)?;
            let due = item.last_attempt_ns.saturating_add(item.backoff_ns) <= now;
            if due {
                out.push(item);
            }
        }
        Ok(out)
    }

    /// 标记完成（删除条目）。
    ///
    /// # Errors
    /// fjall 删除失败。
    pub fn mark_done(&self, pack_id: &str, shard_idx: u8) -> Result<()> {
        self.ks.remove(item_key(pack_id, shard_idx))?;
        Ok(())
    }

    /// 标记失败：retry_count+1、退避指数（base * 2^min(retry, 8)，封顶 max）。
    ///
    /// # Errors
    /// fjall 读写失败。
    pub fn mark_failed(&self, pack_id: &str, shard_idx: u8, now: u64) -> Result<()> {
        let key = item_key(pack_id, shard_idx);
        let cur = self.ks.get(&key)?.ok_or_else(|| {
            fjall::Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "repair item missing",
            ))
        })?;
        let bytes: &[u8] = cur.as_ref();
        let mut item: RepairItem = serde_json::from_slice(bytes)?;
        item.retry_count = item.retry_count.saturating_add(1);
        let shift = item.retry_count.min(8);
        item.backoff_ns =
            (DEFAULT_BACKOFF_BASE_NS.saturating_mul(1u64 << shift)).min(DEFAULT_BACKOFF_MAX_NS);
        item.last_attempt_ns = now;
        let val = serde_json::to_vec(&item)?;
        self.ks.insert(key, val)?;
        Ok(())
    }

    /// 当前队列长度（统计 / 测试用）。
    #[must_use]
    pub fn len(&self) -> usize {
        self.ks.iter().count()
    }

    /// 队列是否为空。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limiter_try_acquire_respects_capacity() {
        let lim = RepairLimiter::with_capacity(1000, 1000); // 1KB/s, 1KB 桶
        assert!(lim.try_acquire(1000));
        assert!(!lim.try_acquire(1));
    }

    #[test]
    fn limiter_refills_over_time() {
        // 1KB/s + 1KB 桶；20ms ≈ 20 字节补充，try_acquire(1) 应恢复成功。
        let lim = RepairLimiter::with_capacity(1_000, 1_000);
        assert!(lim.try_acquire(1_000));
        assert!(!lim.try_acquire(1));
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(lim.try_acquire(1));
    }

    #[test]
    fn queue_enqueue_take_done_roundtrip() {
        let dir = std::env::temp_dir().join(format!("cas-repair-{}", partisync_core::Ulid::now()));
        let q = RepairQueue::open(&dir).expect("open");
        let now = now_ns();
        q.enqueue(&RepairItem {
            pack_id: "p1".into(),
            shard_idx: 3,
            retry_count: 0,
            last_attempt_ns: 0,
            backoff_ns: 0,
        })
        .unwrap();
        q.persist().unwrap();
        assert_eq!(q.len(), 1);
        let due = q.take_due(10, now).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].shard_idx, 3);
        q.mark_done("p1", 3).unwrap();
        assert!(q.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn queue_persists_across_restart() {
        let dir = std::env::temp_dir().join(format!("cas-repair-{}", partisync_core::Ulid::now()));
        {
            let q = RepairQueue::open(&dir).expect("open1");
            q.enqueue(&RepairItem {
                pack_id: "p1".into(),
                shard_idx: 5,
                retry_count: 0,
                last_attempt_ns: 0,
                backoff_ns: 0,
            })
            .unwrap();
            q.persist().unwrap();
        }
        let q2 = RepairQueue::open(&dir).expect("open2");
        assert_eq!(q2.len(), 1);
        let due = q2.take_due(10, now_ns()).unwrap();
        assert_eq!(due[0].shard_idx, 5);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn queue_mark_failed_grows_backoff() {
        let dir = std::env::temp_dir().join(format!("cas-repair-{}", partisync_core::Ulid::now()));
        let q = RepairQueue::open(&dir).expect("open");
        let now = now_ns();
        q.enqueue(&RepairItem {
            pack_id: "p1".into(),
            shard_idx: 0,
            retry_count: 0,
            last_attempt_ns: 0,
            backoff_ns: 0,
        })
        .unwrap();
        q.mark_failed("p1", 0, now).unwrap();
        let due = q.take_due(10, now).unwrap(); // 刚失败 → 未到期
        assert!(due.is_empty());
        let later = now + 60 * 1_000_000_000;
        let due2 = q.take_due(10, later).unwrap();
        assert_eq!(due2.len(), 1);
        assert_eq!(due2[0].retry_count, 1);
        assert!(due2[0].backoff_ns > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
