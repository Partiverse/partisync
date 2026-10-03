//! 装配层（M9-WP01-T01；SPEC M9-WP01 §2）：挂载写事件 → sync 管线。
//!
//! 双缺失补齐（M9-roadmap-proposal N1 实测）：① 把 fuse 写事件折叠为
//! `EventRecord`（wiring_e2e 契约：provider=`partifuse`、path 前导
//! `/`、Rename 的 Created 必带 size）应用 graph 并经 `capture` 写
//! oplog；② 同进程 bisync tick 驱动 `session::bisync`（不动点收敛 +
//! ACK 裁剪 + `note_applied` 水位——F1 派生面）。
//!
//! 事件源 = apply 成功点（R5：WAL execute 后即压实，日志不留痕）；
//! 收端掉线/重排时以最新事件为准（fold 前 metadata 现读 backing）。
//! crash 窗口（append→apply→emit 之间断电）丢事件由 Merkle 全量对账
//! 兜底（SEMANTICS §同步接线节登记，T02 落档）。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use partisync_fuse::events::FuseWriteEvent;
use partisync_graph::store::Store;
use partisync_sync::capture;
use partisync_sync::event::{EventApplier, EventKind, EventRecord, GraphApplier};
use partisync_sync::session::{self, BisyncOpts};
use tokio::sync::mpsc::UnboundedReceiver;

/// 装配层 provider 口径（wiring_e2e 契约同款）。
pub const PROVIDER: &str = "partifuse";

/// 默认 bisync tick 周期（毫秒）。tick 轮询先行（一期非目标：实时监听）。
pub const DEFAULT_TICK_MS: u64 = 5_000;

/// 装配配置。
#[derive(Debug, Clone)]
pub struct WiringOpts {
    /// FUSE backing 根（fold 读 metadata 的基准；同根校验的指纹源）。
    pub backing: PathBuf,
    /// 本端 graph 库路径。
    pub graph_db: PathBuf,
    /// 对端 graph 库路径（None = 只装 graph/oplog，不驱动同步循环）。
    pub peer_db: Option<PathBuf>,
    /// bisync tick 周期（毫秒）。
    pub tick_ms: u64,
}

/// 已初始化的装配会话（init 完成同根校验/播种/tick 启动；serve 消费事件）。
pub struct WiringSession {
    store: Store,
    applier: GraphApplier,
    backing: PathBuf,
    seq: Arc<AtomicU64>,
}

impl WiringSession {
    /// 初始化：打开 graph（含同根校验/首播种）、若有 peer 启动 bisync
    /// tick。**bin 侧必须 block_on 本函数且成功后才挂载**（SPEC §2.1：
    /// 校验失败拒绝启动——带病装配比拒绝装配危险）。
    ///
    /// # Errors
    /// graph 打开失败 / 同根校验失败（graph 已绑定其他卷指纹）→ 字符串
    /// 错误（bin 侧 fail-fast 退出，挂载不启动）。
    pub async fn init(opts: &WiringOpts) -> Result<Self, String> {
        let store = open_bound_store(&opts.backing, &opts.graph_db).await?;
        if let Some(peer_db) = &opts.peer_db {
            let peer = open_bound_store(&opts.backing, peer_db).await?;
            spawn_tick(store.clone(), peer, opts.tick_ms);
        }
        Ok(Self {
            applier: GraphApplier::new(store.clone()),
            store,
            backing: opts.backing.clone(),
            seq: Arc::new(AtomicU64::new(1)),
        })
    }

    /// 事件消费循环：折叠 → 应用 → capture，直到通道关闭（umount 后
    /// bin 侧随 runtime 关停）。
    ///
    /// # Errors
    /// 当前恒为 `Ok`——单事件失败不中止装配（挂载面优先存活），错误面
    /// 走 stderr，丢失窗口由 bisync 全量对账兜底。
    pub async fn serve(self, mut rx: UnboundedReceiver<FuseWriteEvent>) -> Result<(), String> {
        while let Some(event) = rx.recv().await {
            if let Err(e) = self.apply_event(event).await {
                eprintln!("[partifuse-wiring] 事件应用失败（留待对账）: {e}");
            }
        }
        Ok(())
    }

    /// 单事件：折叠 → GraphApplier 应用 → capture 写 oplog。
    async fn apply_event(&self, event: FuseWriteEvent) -> Result<(), String> {
        let records = fold(&self.store, &self.backing, &event, &self.seq).await?;
        for record in &records {
            self.applier
                .apply_event(record)
                .await
                .map_err(|e| format!("graph 应用失败: {e}"))?;
        }
        // capture（图谱变更 → oplog；origin = 本机 device，capture 现行机制）
        match event {
            FuseWriteEvent::Upsert { path } => {
                capture::record_entry_upsert(&self.store, &rec_path(&path))
                    .await
                    .map_err(|e| format!("capture upsert 失败: {e}"))?;
            }
            FuseWriteEvent::Remove { path } => {
                capture::record_entry_remove(&self.store, &rec_path(&path))
                    .await
                    .map_err(|e| format!("capture remove 失败: {e}"))?;
            }
            FuseWriteEvent::Rename { from, to } => {
                capture::record_entry_remove(&self.store, &rec_path(&from))
                    .await
                    .map_err(|e| format!("capture rename(from) 失败: {e}"))?;
                capture::record_entry_upsert(&self.store, &rec_path(&to))
                    .await
                    .map_err(|e| format!("capture rename(to) 失败: {e}"))?;
            }
        }
        Ok(())
    }
}

/// 打开 graph 库并做同根校验：库内已登记卷指纹与 backing 规范路径
/// 不一致 → 拒绝（防「A 卷的库配 B 卷的 backing」带病装配）；空库
/// 首次播种（设备行 + 卷指纹 = backing 规范路径）。
async fn open_bound_store(backing: &Path, db: &Path) -> Result<Store, String> {
    let store = Store::open(db)
        .await
        .map_err(|e| format!("打开 graph 失败: {e}"))?;
    let fingerprint = fingerprint_of(backing);
    let known = store
        .volume_fingerprints()
        .await
        .map_err(|e| format!("读卷指纹失败: {e}"))?;
    if known.is_empty() {
        store
            .seed_device_volume("partifuse", "PartiFuse 本机", &fingerprint)
            .await
            .map_err(|e| format!("播种设备/卷失败: {e}"))?;
    } else if !known.iter().any(|f| f == &fingerprint) {
        return Err(format!(
            "同根校验失败：graph 库已绑定卷 {known:?}，与 backing 指纹 {fingerprint:?} 不一致（拒绝装配）"
        ));
    }
    Ok(store)
}

/// 卷指纹 = backing 规范绝对路径（同机装配口径；跨机指纹归卷同步 WP）。
fn fingerprint_of(backing: &Path) -> String {
    std::fs::canonicalize(backing)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| backing.to_string_lossy().into_owned())
}

/// graph 口径路径（前导 '/'；wiring_e2e 判例——无前导 = 叶 key 漂移）。
fn rec_path(rel: &str) -> String {
    format!("/{}", rel.trim_start_matches('/'))
}

/// 事件 → `EventRecord` 序列（T05 判例：Rename 的 Created 必带源条目
/// size——取 backing 终态 metadata；metadata 不可得（事件过期）时该
/// 事件降级为空序列，以最新事件为准）。
async fn fold(
    store: &Store,
    backing: &Path,
    event: &FuseWriteEvent,
    seq: &AtomicU64,
) -> Result<Vec<EventRecord>, String> {
    let n = seq.fetch_add(1, Ordering::Relaxed);
    let cursor = format!("pf-{n}");
    match event {
        FuseWriteEvent::Upsert { path } => {
            let abs = backing.join(path);
            let Ok(md) = std::fs::symlink_metadata(&abs) else {
                return Ok(vec![]); // 已被后续 remove/replace 覆盖
            };
            if md.is_dir() {
                return Ok(vec![]); // 目录链由 GraphApplier ensure_dir_chain 按需物化
            }
            let p = rec_path(path);
            let kind = if store
                .entry_by_path(&p)
                .await
                .map_err(|e| format!("查行失败: {e}"))?
                .is_some()
            {
                EventKind::Modified
            } else {
                EventKind::Created
            };
            Ok(vec![record(
                &p,
                kind,
                md_len(&md),
                md_mtime_ns(&md),
                &cursor,
            )])
        }
        FuseWriteEvent::Remove { path } => Ok(vec![record(
            &rec_path(path),
            EventKind::Removed,
            0,
            0,
            &cursor,
        )]),
        FuseWriteEvent::Rename { from, to } => {
            let md = std::fs::symlink_metadata(backing.join(to));
            let Ok(md) = md else {
                return Ok(vec![]); // 已被后续操作覆盖
            };
            Ok(vec![
                record(&rec_path(from), EventKind::Removed, 0, 0, &cursor),
                record(
                    &rec_path(to),
                    EventKind::Created,
                    md_len(&md),
                    md_mtime_ns(&md),
                    &cursor,
                ),
            ])
        }
    }
}

fn record(path: &str, kind: EventKind, size: u64, mtime_ns: u64, cursor: &str) -> EventRecord {
    EventRecord {
        provider: PROVIDER.into(),
        space: "default".into(),
        path: path.into(),
        kind,
        cursor: cursor.into(),
        payload: serde_json::json!({ "size": size, "mtime_ns": mtime_ns }),
    }
}

fn md_len(md: &std::fs::Metadata) -> u64 {
    md.len()
}

fn md_mtime_ns(md: &std::fs::Metadata) -> u64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(0))
}

/// bisync tick：周期驱动双向 push（不动点 + max_rounds 有界退出）。
/// 错误打 stderr 后继续 tick（同步循环不因单轮失败死亡——对账兜底）。
fn spawn_tick(store: Store, peer: Store, tick_ms: u64) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(tick_ms));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            match session::bisync(&store, &peer, BisyncOpts::default()).await {
                Ok(stats) if stats.pushed + stats.pulled > 0 => {
                    eprintln!(
                        "[partifuse-wiring] bisync round {}：pushed={} pulled={} conflicts={}",
                        stats.rounds, stats.pushed, stats.pulled, stats.conflicts
                    );
                }
                Ok(_) => {}
                Err(e) => eprintln!("[partifuse-wiring] bisync 失败（下轮重试）: {e}"),
            }
        }
    });
}
