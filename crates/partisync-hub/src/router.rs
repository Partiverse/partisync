//! range 分区路由（SPEC M3-WP01 §3）：`[start_key, end_key) → partition`。
//!
//! 分区表持久化于 meta keyspace（元数据先行），路由层内存缓存。
//! 端区界隐式：分区 i 的 end = 分区 i+1 的 start；末分区 end 无界。
//!
//! keyspace 收敛（SPEC M3-WP02 裁定 3）：meta 行落在 `m-meta` 前缀
//! `[0x02, 0x01, start_key]`，分区数据共享 `m-child`——行数按
//! `[start, next_start)` range 计，不再逐 keyspace 打开。

use std::sync::RwLock;

use fjall::Keyspace;

use crate::ksconv::{meta_key, META_PREFIX};

/// 分区元数据状态位（meta 值第 9 字节）。
pub const STATUS_ACTIVE: u8 = 0;
/// 分裂中（崩溃恢复时按协议收尾——SPEC §3 分裂协议）。
pub const STATUS_SPLITTING: u8 = 1;

/// meta keyspace 名（WP01 旧布局 `t-meta`；收敛后为 `m-meta`，见
/// [`crate::ksconv::KS_META`]——迁移探测仍按此名识别旧库）。
pub const META_KEYSPACE: &str = "t-meta";
/// meta 内 next-pid 计数器键（收敛后前缀化为 `[0x02, 0x00]`）。
pub const META_NEXT_PID: &[u8] = &[0x00];
/// meta 分区行键前缀（后接 start_key；收敛后外层再加 [`META_PREFIX`]）。
pub const META_PARTITION_PREFIX: u8 = 0x01;

/// 单个 range 分区的内存路由项。
#[derive(Debug, Clone)]
pub struct Partition {
    /// 起始键（含，前缀化存储键）。
    pub start: Vec<u8>,
    /// 分区号（收敛后仅作元数据标识；WP01 曾映射 keyspace 名 `t-p{pid}`）。
    pub id: u64,
    /// 分裂中标记（meta 持久态的缓存）。
    pub splitting: bool,
}

/// 路由器：按键序排列的分区表 + 各分区行数近似计数。
pub struct Router {
    parts: Vec<Partition>,
    counts: Vec<u64>,
}

/// 路由器锁别名（v0.1 单写者；读多写少）。
pub type SharedRouter = RwLock<Router>;

impl Router {
    /// 空路由器（open 初始化用，首分区创建前）。
    #[must_use]
    pub fn empty() -> Self {
        Self {
            parts: Vec::new(),
            counts: Vec::new(),
        }
    }

    /// 首分区登记（start = 空）。
    pub fn seed_first(&mut self, pid: u64) {
        self.parts.push(Partition {
            start: Vec::new(),
            id: pid,
            splitting: false,
        });
        self.counts.push(0);
    }

    /// 由 meta keyspace 重建路由器（open 恢复路径）。
    ///
    /// 收敛后 meta 行为 `[0x02, 0x01, start_key]`（`m-meta`），数据全部驻留
    /// 共享 `data` keyspace（`m-child`）；行数按 `[start, next_start)`
    /// range 计——不再逐个打开 `t-p{id}` keyspace（256+ 次 manifest fsync
    /// 是 WP01 open=12.8s 的来源）。
    ///
    /// # Errors
    /// meta 读取或 data range 计数失败。
    pub fn load(meta: &Keyspace, data: &Keyspace) -> fjall::Result<Self> {
        let mut parts = Vec::new();
        for guard in meta.prefix([META_PREFIX, META_PARTITION_PREFIX]) {
            let (k, v) = guard.into_inner()?;
            let start = k[2..].to_vec();
            let mut pid_buf = [0u8; 8];
            pid_buf.copy_from_slice(&v[0..8]);
            let id = u64::from_be_bytes(pid_buf);
            let splitting = v[8] == STATUS_SPLITTING;
            parts.push(Partition {
                start,
                id,
                splitting,
            });
        }
        parts.sort_by(|a, b| a.start.cmp(&b.start));
        let counts = parts
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let end = parts.get(i + 1).map(|n| n.start.as_slice());
                partition_count(data, &p.start, end)
            })
            .collect::<fjall::Result<Vec<u64>>>()?;
        Ok(Self { parts, counts })
    }

    /// 键所属分区下标（最后一个 start ≤ key 的分区；表空则 panic——open 必建首分区）。
    #[must_use]
    pub fn lookup(&self, key: &[u8]) -> usize {
        match self.parts.binary_search_by(|p| p.start.as_slice().cmp(key)) {
            Ok(i) => i,
            Err(0) => 0,
            Err(i) => i - 1,
        }
    }

    /// 分区表（按键序）。
    #[must_use]
    pub fn partitions(&self) -> &[Partition] {
        &self.parts
    }

    /// 各分区近似行数（与 [`Self::partitions`] 下标对齐）。
    #[must_use]
    pub fn counts(&self) -> &[u64] {
        &self.counts
    }

    /// 原位递增分区计数（put 路径）。
    pub fn bump(&mut self, idx: usize) {
        self.counts[idx] += 1;
    }

    /// 直设分区计数（分裂误触发校正）。
    pub fn set_count(&mut self, idx: usize, v: u64) {
        self.counts[idx] = v;
    }

    /// 全量替换计数（恢复收尾后重算）。
    pub fn replace_counts(&mut self, counts: Vec<u64>) {
        self.counts = counts;
    }

    /// 分裂完成后登记新分区（idx 之后插入）并修正两侧计数。
    pub fn insert_split(&mut self, idx: usize, p: Partition, src_count: u64, moved: u64) {
        self.parts.insert(idx + 1, p);
        self.counts[idx] = src_count - moved;
        self.counts.insert(idx + 1, moved);
    }

    /// 分裂收尾：清除 splitting 标记。
    pub fn clear_splitting(&mut self, idx: usize) {
        self.parts[idx].splitting = false;
    }
}

/// meta 分区行键（收敛后完整存储键）= `[0x02, 0x01] + start_key`。
#[must_use]
pub fn partition_meta_key(start: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(1 + start.len());
    raw.push(META_PARTITION_PREFIX);
    raw.extend_from_slice(start);
    meta_key(&raw)
}

/// 共享 `data` keyspace 上按分区逻辑区间 `[start, end)` 计行
/// （end=None 即无界）。
///
/// # Errors
/// 迭代失败。
pub fn partition_count(data: &Keyspace, start: &[u8], end: Option<&[u8]>) -> fjall::Result<u64> {
    use std::ops::Bound;
    let hi: Bound<Vec<u8>> = match end {
        None => Bound::Unbounded,
        Some(e) => Bound::Excluded(e.to_vec()),
    };
    let mut n = 0u64;
    for guard in data.range((Bound::Included(start.to_vec()), hi)) {
        guard.into_inner()?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn router(starts: &[&[u8]]) -> Router {
        let parts = starts
            .iter()
            .enumerate()
            .map(|(i, s)| Partition {
                start: s.to_vec(),
                id: i as u64,
                splitting: false,
            })
            .collect();
        let counts = vec![0; starts.len()];
        Router { parts, counts }
    }

    #[test]
    fn lookup_routes_to_last_start_le_key() {
        let r = router(&[b"", b"m", b"z"]);
        assert_eq!(r.lookup(b"a"), 0);
        assert_eq!(r.lookup(b"m"), 1);
        assert_eq!(r.lookup(b"mm"), 1);
        assert_eq!(r.lookup(b"z"), 2);
        assert_eq!(r.lookup(b"zzz"), 2);
    }

    #[test]
    fn empty_key_routes_to_first() {
        let r = router(&[b"", b"m"]);
        assert_eq!(r.lookup(b""), 0);
    }
}
