//! range 分区路由（SPEC M3-WP01 §3）：`[start_key, end_key) → partition`。
//!
//! 分区表持久化于独立 meta keyspace（元数据先行），路由层内存缓存。
//! 端区界隐式：分区 i 的 end = 分区 i+1 的 start；末分区 end 无界。

use std::collections::HashMap;
use std::sync::RwLock;

use fjall::{Database, Keyspace, KeyspaceCreateOptions};

/// 分区元数据状态位（meta 值第 9 字节）。
pub const STATUS_ACTIVE: u8 = 0;
/// 分裂中（崩溃恢复时按协议收尾——SPEC §3 分裂协议）。
pub const STATUS_SPLITTING: u8 = 1;

/// meta keyspace 名。
pub const META_KEYSPACE: &str = "t-meta";
/// meta 内 next-pid 计数器键。
pub const META_NEXT_PID: &[u8] = &[0x00];
/// meta 分区行键前缀（后接 start_key）。
pub const META_PARTITION_PREFIX: u8 = 0x01;

/// 单个 range 分区的内存路由项。
#[derive(Debug, Clone)]
pub struct Partition {
    /// 起始键（含）。
    pub start: Vec<u8>,
    /// 分区号（keyspace 名 `t-p{pid}`）。
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
    /// # Errors
    /// meta 读取或 keyspace 打开失败。
    pub fn load(meta: &Keyspace, db: &Database) -> fjall::Result<(Self, HashMap<u64, Keyspace>)> {
        let mut parts = Vec::new();
        let mut keyspaces = HashMap::new();
        for guard in meta.prefix([META_PARTITION_PREFIX]) {
            let (k, v) = guard.into_inner()?;
            let start = k[1..].to_vec();
            let mut pid_buf = [0u8; 8];
            pid_buf.copy_from_slice(&v[0..8]);
            let id = u64::from_be_bytes(pid_buf);
            let splitting = v[8] == STATUS_SPLITTING;
            let ks = db.keyspace(&format!("t-p{id}"), KeyspaceCreateOptions::default)?;
            keyspaces.insert(id, ks);
            parts.push(Partition {
                start,
                id,
                splitting,
            });
        }
        parts.sort_by(|a, b| a.start.cmp(&b.start));
        let counts = parts
            .iter()
            .map(|p| keyspaces[&p.id].iter().count() as u64)
            .collect();
        Ok((Self { parts, counts }, keyspaces))
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

    /// 全量替换计数（恢复搬移后重算）。
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
