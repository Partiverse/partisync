//! 分代 GC（SPEC M3-WP04 §3 裁定 5）：分代 + 宽限期 + 重打包。
//!
//! 设计：
//!
//! - [`GcState`]：墓碑集合（HashMap + 代际）+ 可选持久化（JSON snapshot，
//!   `.tmp + rename` 原子写）。`mark_dead` / `claim_due` /
//!   `clear_tombstone` / `unmark` 均 ≤ µs；内部 `Mutex<HashMap>` 仅保护
//!   墓碑状态，**无全局锁**——persist 路径用 clone-snapshot 释放锁后再
//!   序列化 / IO。
//! - [`Clock`] trait：时间注入；`SystemClock` 给生产用。`TestClock`
//!   **仅测试可见**（不公开 re-export，防假时间注入 DoS）。
//! - [`effective_data_ratio`]：纯函数，由 `PackIndex` + 活引用判定计算
//!   重打包阈值。
//! - [`PackLockTable`]：per-pack `tokio::sync::RwLock`（SPEC §契约 §2）——
//!   同一 pack 写互斥、读可并发、不同 pack 完全独立。
//!
//! 分代：每个墓碑带 `gen` 字段；`GcState::sweep_gen()` 由管理面/定时器
//! 调用推进当前代；增量 GC 优先扫新增代的墓碑，老代墓碑仅在 refcount==0
//! 且宽限到期后回收（防止分代切换中的悬挂引用）。
//!
//! 认领 / 提交 / 回滚（数据丢失高危面 —— AI 对抗审查 #1 修复）：
//!
//! - `claim_due(grace, live_filter)` **原子**摘除到期墓碑并返回；调用方
//!   在确认块可真删（refcount 已 check 过）后调 `clear_tombstone`（此时为
//!   no-op）或调 `abort_reclaim`（refcount 回升 / IO 失败时撤销认领，按
//!   原 marked_at 重插墓碑）。
//! - `take_due_for` 保留为**只读快照**（诊断/统计用）；reclaim 流水线
//!   必须用 `claim_due`。
//!
//! 重打包协议（SPEC §2 崩溃一致性）：写新 pack → 原子索引切换 → 删旧
//! pack（与 [`crate::tier`] 迁移协议同构）；本模块提供判定 + 锁原语，
//! 三步协议 helper（M4+ 起独立 PR）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock as TokioRwLock};

/// 自 poison 中恢恢复（AI 对抗审查 #6：std::sync::Mutex 一旦 panic 永久
/// 中毒；用 `into_inner()` 把数据摘回 victim 避免单点故障级联）。
fn lock_recover<'a, T>(
    res: std::result::Result<
        std::sync::MutexGuard<'a, T>,
        std::sync::PoisonError<std::sync::MutexGuard<'a, T>>,
    >,
) -> std::sync::MutexGuard<'a, T> {
    res.unwrap_or_else(|e| e.into_inner())
}

use crate::pack::PackIndex;

/// 时钟抽象（注入测试时间）。
pub trait Clock: Send + Sync {
    fn now_ns(&self) -> u64;
}

/// 系统时钟。
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ns(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64)
    }
}

/// 测试时钟：base + offset，advance 推进 offset。
pub struct TestClock {
    base_ns: u64,
    offset_ns: Mutex<u64>,
}

impl TestClock {
    pub fn new(base_ns: u64) -> Self {
        Self {
            base_ns,
            offset_ns: Mutex::new(0),
        }
    }
    pub fn advance(&self, d: Duration) {
        let mut g = self.offset_ns.lock().unwrap();
        *g = g.saturating_add(d.as_nanos() as u64);
    }
}

impl Clock for TestClock {
    fn now_ns(&self) -> u64 {
        self.base_ns.saturating_add(*self.offset_ns.lock().unwrap())
    }
}

/// 墓碑条目。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tombstone {
    pub hash: String,
    pub marked_at_ns: u64,
    /// 标记时的代际（SPEC §3 裁定 5「分代」）。`sweep_gen` 推进，
    /// 老代墓碑仅在 refcount==0 + 宽限到期后回收。
    #[serde(default)]
    pub gen: u64,
}

/// [`GcState::mark_dead`] 的返回：新增 / 已存在。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkOutcome {
    New,
    AlreadyDead,
}

/// GC 错误。
#[derive(Debug)]
pub enum GcError {
    Io(std::io::Error),
    Serialize(serde_json::Error),
}

impl core::fmt::Display for GcError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "gc io: {e}"),
            Self::Serialize(e) => write!(f, "gc serialize: {e}"),
        }
    }
}

impl std::error::Error for GcError {}

impl From<std::io::Error> for GcError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for GcError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialize(e)
    }
}

/// 平面结果别名。
pub type Result<T> = std::result::Result<T, GcError>;

struct GcInner {
    tombstones: HashMap<String, Tombstone>,
}

/// GC 墓碑状态（同步 API；loom 友好）。
///
/// # 并发模型
/// `inner: Mutex<HashMap>` 仅保护墓碑状态；所有操作在锁内 ≤ µs。**不**
/// 在内部持锁跨 await——所以并发写者 + GC 调用方不会死锁。持久化路径
/// 先 clone 快照释放锁，再做 JSON 序列化与磁盘 IO（不持锁跨 IO，
/// 满足 SPEC §契约 §2「无全局锁」）。
///
/// # Poisoning
/// std::sync::Mutex 在持锁线程 panic 时进入 poison 状态。本实现所有
/// `lock()` 用 `lock_recover` 从 poison 中恢复（保留数据可用性）——
/// 单点 buggy 闭包不会级联永久失效（AI 对抗审查 #6）。
pub struct GcState {
    inner: Mutex<GcInner>,
    /// 当前分代（SPEC §3 裁定 5「分代」）；`sweep_gen` 推进。
    current_gen: AtomicU64,
    persist_path: Option<PathBuf>,
    clock: Arc<dyn Clock>,
}

impl GcState {
    /// 新建（不持久化）。
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            inner: Mutex::new(GcInner {
                tombstones: HashMap::new(),
            }),
            current_gen: AtomicU64::new(0),
            persist_path: None,
            clock,
        }
    }

    /// 打开 / 新建（持久化到 `path`，原子写：`.tmp + rename`）。
    ///
    /// # Errors
    /// 读 / 写 / JSON 解析失败。
    pub fn open(path: &Path, clock: Arc<dyn Clock>) -> Result<Self> {
        let inner = if path.exists() {
            let bytes = std::fs::read(path)?;
            let map: HashMap<String, Tombstone> = serde_json::from_slice(&bytes)?;
            GcInner { tombstones: map }
        } else {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            GcInner {
                tombstones: HashMap::new(),
            }
        };
        // 启动代 = max(tombstone.gen) + 1；保证新建墓碑总是新代。
        let max_gen = inner.tombstones.values().map(|t| t.gen).max().unwrap_or(0);
        Ok(Self {
            inner: Mutex::new(inner),
            current_gen: AtomicU64::new(max_gen),
            persist_path: Some(path.to_path_buf()),
            clock,
        })
    }

    /// 推进分代并返回新代号。
    pub fn sweep_gen(&self) -> u64 {
        self.current_gen.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// 当前代（只读）。
    pub fn current_gen(&self) -> u64 {
        self.current_gen.load(Ordering::SeqCst)
    }

    /// 标记块为待删（幂等）。写入当前代号。
    pub fn mark_dead(&self, hash: &str) -> MarkOutcome {
        let now = self.clock.now_ns();
        let gen = self.current_gen.load(Ordering::SeqCst);
        let mut g = lock_recover(self.inner.lock());
        if g.tombstones.contains_key(hash) {
            return MarkOutcome::AlreadyDead;
        }
        g.tombstones.insert(
            hash.to_owned(),
            Tombstone {
                hash: hash.to_owned(),
                marked_at_ns: now,
                gen,
            },
        );
        MarkOutcome::New
    }

    /// 撤销墓碑（refcount 回升等场景）。
    ///
    /// # Returns
    /// true = 移除了现有墓碑；false = 本来就没有。
    pub fn unmark(&self, hash: &str) -> bool {
        let mut g = lock_recover(self.inner.lock());
        g.tombstones.remove(hash).is_some()
    }

    /// 墓碑数量。
    pub fn len(&self) -> usize {
        lock_recover(self.inner.lock()).tombstones.len()
    }

    /// `len() == 0`。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 只读快照：返回到期墓碑（marked_at + grace_ns ≤ now）——**不删除**。
    /// 诊断 / 统计用。reclaim 流水线必须用 [`claim_due`]（AI 对抗审查 #1）。
    pub fn take_due(&self, grace_ns: u64) -> Vec<Tombstone> {
        self.take_due_for(grace_ns, &|_| false)
    }

    /// 同 [`take_due`]，但仅返回 `live_filter` 返回 false 的（refcount==0）。
    /// `live_filter(&str) -> bool`：true = 块仍活（refcount>0）。
    ///
    /// **只读快照**——reclaim 流水线必须用 [`claim_due`]。
    pub fn take_due_for(
        &self,
        grace_ns: u64,
        live_filter: &dyn Fn(&str) -> bool,
    ) -> Vec<Tombstone> {
        let now = self.clock.now_ns();
        let g = lock_recover(self.inner.lock());
        g.tombstones
            .values()
            .filter(|t| now.saturating_sub(t.marked_at_ns) >= grace_ns && !live_filter(&t.hash))
            .cloned()
            .collect()
    }

    /// **原子认领**到期墓碑：从 map 摘除 + 返回。调用方拿到后进入 reclaim
    /// 流水线；若发现块不可真删（IO 失败 / 认领期间 refcount 被并发 incr
    /// 回升），调 [`abort_reclaim`] 按原 `marked_at` + gen 重插墓碑；
    /// 成功后 [`clear_tombstone`] 为 no-op。
    ///
    /// AI 对抗审查 #1：修复 take_due_for「快照式」竞态——块在 take 与
    /// delete 之间 refcount 回升可致真删活块。
    pub fn claim_due(&self, grace_ns: u64, live_filter: &dyn Fn(&str) -> bool) -> Vec<Tombstone> {
        let now = self.clock.now_ns();
        let mut g = lock_recover(self.inner.lock());
        let keys: Vec<String> = g
            .tombstones
            .iter()
            .filter_map(|(k, t)| {
                if now.saturating_sub(t.marked_at_ns) >= grace_ns && !live_filter(&t.hash) {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect();
        let mut claimed = Vec::with_capacity(keys.len());
        for k in keys {
            if let Some(t) = g.tombstones.remove(&k) {
                claimed.push(t);
            }
        }
        claimed
    }

    /// 撤销 [`claim_due`] 摘出的墓碑——按原 `marked_at` + `gen` 重插。
    /// 已存在的墓碑不覆盖（保留更早 marked_at 的）。
    pub fn abort_reclaim(&self, t: &Tombstone) {
        let mut g = lock_recover(self.inner.lock());
        g.tombstones
            .entry(t.hash.clone())
            .or_insert_with(|| t.clone());
    }

    /// 显式删除墓碑（`take_due_for` 路径下 reclaim 成功后调用；
    /// `claim_due` 路径下为 no-op）。
    pub fn clear_tombstone(&self, hash: &str) {
        lock_recover(self.inner.lock()).tombstones.remove(hash);
    }

    /// 持久化（若 `open` 时给了 path）—— 原子写。
    ///
    /// # 锁策略
    /// 先 clone 快照释放锁，再做序列化与 IO——**不持锁跨 IO**，避免
    /// 大墓碑集合时阻塞全部 GC 路径（AI 对抗审查 #3）。
    ///
    /// # Errors
    /// IO / JSON 序列化错误。
    pub fn persist(&self) -> Result<()> {
        let Some(path) = &self.persist_path else {
            return Ok(());
        };
        let snapshot: HashMap<String, Tombstone> = {
            let g = lock_recover(self.inner.lock());
            g.tombstones.clone()
        };
        let bytes = serde_json::to_vec(&snapshot)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// 测试辅助：当前墓碑快照。
    pub fn snapshot(&self) -> Vec<Tombstone> {
        lock_recover(self.inner.lock())
            .tombstones
            .values()
            .cloned()
            .collect()
    }
}

/// 计算 pack 有效数据比 = live_bytes / total_bytes。
///
/// 空 pack 返回 1.0（无数据无浪费；不应触发重打包）。`live_filter(&str) -> bool`：
/// true = 块仍活（refcount>0）。
pub fn effective_data_ratio(index: &PackIndex, live_filter: &dyn Fn(&str) -> bool) -> f64 {
    if index.entries.is_empty() {
        return 1.0;
    }
    let mut total: u64 = 0;
    let mut live: u64 = 0;
    for e in &index.entries {
        total = total.saturating_add(e.len);
        if live_filter(&e.hash) {
            live = live.saturating_add(e.len);
        }
    }
    if total == 0 {
        1.0
    } else {
        live as f64 / total as f64
    }
}

/// per-pack 锁表（SPEC §契约 §2 「per-pack 锁」）。
///
/// 同一 pack 写互斥、读可并发；不同 pack 完全独立。锁按需创建并以
/// `Arc<TokioRwLock>` 持有——释放锁表项后已发放的 guard 仍有效。
pub struct PackLockTable {
    locks: Mutex<HashMap<String, Arc<TokioRwLock<()>>>>,
}

impl Default for PackLockTable {
    fn default() -> Self {
        Self::new()
    }
}

impl PackLockTable {
    pub fn new() -> Self {
        Self {
            locks: Mutex::new(HashMap::new()),
        }
    }

    fn lock_for(&self, pack_id: &str) -> Arc<TokioRwLock<()>> {
        let mut g = self.locks.lock().unwrap();
        g.entry(pack_id.to_owned())
            .or_insert_with(|| Arc::new(TokioRwLock::new(())))
            .clone()
    }

    pub async fn read(&self, pack_id: &str) -> OwnedRwLockReadGuard<()> {
        self.lock_for(pack_id).read_owned().await
    }

    pub async fn write(&self, pack_id: &str) -> OwnedRwLockWriteGuard<()> {
        self.lock_for(pack_id).write_owned().await
    }

    /// 非阻塞写获取（`None` = 当前被持）。
    pub fn try_write(&self, pack_id: &str) -> Option<OwnedRwLockWriteGuard<()>> {
        self.lock_for(pack_id).try_write_owned().ok()
    }

    /// 已持有的锁数（测试 / 诊断）。
    pub fn len(&self) -> usize {
        self.locks.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ============================================================================
// 单元测试（不依赖 loom / tokio，集成在 proptest / 集成测试文件里）
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clock_is_post_2023() {
        let n = SystemClock.now_ns();
        // 2024-01-01 ≈ 1.704e18
        assert!(n > 1_700_000_000_000_000_000);
    }

    #[test]
    fn test_clock_advances() {
        let c = TestClock::new(100);
        assert_eq!(c.now_ns(), 100);
        c.advance(Duration::from_secs(1));
        assert_eq!(c.now_ns(), 100 + 1_000_000_000);
    }

    #[test]
    fn mark_dead_is_idempotent() {
        let c = Arc::new(TestClock::new(0));
        let g = GcState::new(c);
        assert!(matches!(g.mark_dead("h"), MarkOutcome::New));
        assert!(matches!(g.mark_dead("h"), MarkOutcome::AlreadyDead));
        assert_eq!(g.len(), 1);
    }

    #[test]
    fn take_due_respects_grace() {
        let c = Arc::new(TestClock::new(0));
        let g = GcState::new(c.clone());
        g.mark_dead("h");
        // grace = 1s ⇒ 不应到期
        assert!(g.take_due(1_000_000_000).is_empty());
        c.advance(Duration::from_secs(2));
        assert_eq!(g.take_due(1_000_000_000).len(), 1);
    }
}
