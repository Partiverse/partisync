//! entry 哈希平面（SPEC M3-WP01 §1/§2/裁定 5）：单 fjall Database + 前缀共享 keyspace。
//!
//! - 权威行存储：put/get/remove 直达 `shard_of(entry_id)` 路由前缀；
//! - 键 = `0x00 ␟ shard 1B ␟ entry_id 16B`（17B，前缀隔离；收敛后
//!   keyspace 共享见 [`crate::ksconv`]——SPEC M3-WP02 裁定 3）；
//! - 删除 = 墓碑（`FLAG_DELETED`），行保留至 WP03 对账窗口；
//! - children 投影与读时修复归 WP01-T05（tree_plane/split/lib.rs）；
//! - v0.1 单写者串行（进程内 `&self` 独占）；并发写语义归 WP02。

use std::path::Path;

use fjall::{Database, Keyspace, KeyspaceCreateOptions, PersistMode};

use crate::encode::{decode_entry_row, encode_entry_row, EntryRow};
use crate::ksconv::{entry_key, ENTRY_PREFIX, KS_ENTRY};
use crate::shard::{shard_of, SHARD_COUNT};

/// 哈希平面统一错误。
#[derive(Debug)]
pub enum HubError {
    /// 底层存储引擎错误。
    Fjall(fjall::Error),
    /// 行编码错误。
    Encode(crate::encode::EncodeError),
    /// 路由命中的分区 keyspace 缺失（内部不变量破坏，不可恢复）。
    PartitionMissing(u64),
    /// 操作目标条目不存在或已墓碑（rename 语义，WP01-T05）。
    EntryMissing,
}

impl core::fmt::Display for HubError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Fjall(e) => write!(f, "hub storage: {e}"),
            Self::Encode(e) => write!(f, "hub storage: {e}"),
            Self::PartitionMissing(pid) => write!(f, "hub storage: partition {pid} missing"),
            Self::EntryMissing => write!(f, "hub storage: entry not found or tombstoned"),
        }
    }
}

impl std::error::Error for HubError {}

impl From<fjall::Error> for HubError {
    fn from(e: fjall::Error) -> Self {
        Self::Fjall(e)
    }
}

impl From<crate::encode::EncodeError> for HubError {
    fn from(e: crate::encode::EncodeError) -> Self {
        Self::Encode(e)
    }
}

/// 平面结果别名。
pub type Result<T> = std::result::Result<T, HubError>;

/// entry 哈希平面：前缀共享 keyspace，`shard_of` 路由 + 前缀隔离。
///
/// 物理布局收敛（SPEC M3-WP02 裁定 3）：所有 256 分片共用一个
/// `m-entry` keyspace，键按 `shard` 前缀排序——行为契约不变
/// （路由仍按 `shard_of` 确定性分布），仅消除 open 时逐 keyspace
/// fsync 的 O(256) 开销。
pub struct HashPlane {
    db: Database,
    /// 共享 keyspace（`m-entry`）；256 分片逻辑隔离靠前缀字节。
    ks: Keyspace,
}

impl HashPlane {
    /// 打开（或创建）位于 `root` 的哈希平面；重开恢复全部分片。
    ///
    /// # Errors
    /// 引擎打开或 keyspace 创建失败时返回 [`HubError::Fjall`]。
    pub fn open(root: &Path) -> Result<Self> {
        Self::attach(Database::open(fjall::Config::new(root))?)
    }

    /// 挂接到既有 Database（Hub 单库多平面共享，fjall 单路径排他锁）。
    ///
    /// # Errors
    /// keyspace 创建失败时返回 [`HubError::Fjall`]。
    pub fn attach(db: Database) -> Result<Self> {
        let ks = db.keyspace(KS_ENTRY, KeyspaceCreateOptions::default)?;
        Ok(Self { db, ks })
    }

    /// 落盘（ durability 测试与优雅关闭用）。
    ///
    /// # Errors
    /// 引擎刷盘失败时返回 [`HubError::Fjall`]。
    pub fn persist(&self) -> Result<()> {
        self.db.persist(PersistMode::SyncData)?;
        Ok(())
    }

    /// 分片驻留计数（内省/演示面用）：按分片号升序返回 256 项。
    ///
    /// # Errors
    /// 任一分片扫描失败。
    pub fn stats(&self) -> Result<Vec<u64>> {
        let mut counts = vec![0u64; SHARD_COUNT];
        for guard in self.ks.iter() {
            let (k, _) = guard.into_inner().map_err(HubError::from)?;
            if k.len() == 18 && k[0] == ENTRY_PREFIX {
                counts[k[1] as usize] += 1;
            }
        }
        Ok(counts)
    }

    /// 全平面扫描（内省/演示面用）：按键序产出全部行（含墓碑）。
    ///
    /// # Errors
    /// 引擎读取失败或值损坏。
    pub fn iter_entries(&self) -> Result<Vec<EntryRow>> {
        let mut rows = Vec::new();
        for guard in self.ks.iter() {
            let (k, v) = guard.into_inner().map_err(HubError::from)?;
            if k.len() == 18 && k[0] == ENTRY_PREFIX {
                let mut row = decode_entry_row(&v)?;
                row.entry_id.copy_from_slice(&k[2..]);
                rows.push(row);
            }
        }
        Ok(rows)
    }

    /// 写入（upsert）entry 权威行。
    ///
    /// # Errors
    /// 编码或引擎写入失败。
    pub fn put(&self, row: &EntryRow) -> Result<()> {
        let k = entry_key(shard_of(&row.entry_id), &row.entry_id);
        self.ks.insert(k, encode_entry_row(row)?)?;
        Ok(())
    }

    /// 读取 entry 行（墓碑行原样返回，调用方以 [`EntryRow::is_deleted`] 判别）。
    ///
    /// # Errors
    /// 引擎读取失败或值损坏。
    pub fn get(&self, entry_id: &[u8; 16]) -> Result<Option<EntryRow>> {
        let k = entry_key(shard_of(entry_id), entry_id);
        match self.ks.get(k.as_slice())? {
            None => Ok(None),
            Some(slice) => {
                let mut row = decode_entry_row(&slice)?;
                row.entry_id = *entry_id;
                Ok(Some(row))
            }
        }
    }

    /// 删除：置墓碑位（行保留，WP03 对账窗口用）；从未写入过的 id 落最小墓碑。
    ///
    /// # Errors
    /// 引擎读写失败或既有值损坏。
    pub fn remove(&self, entry_id: &[u8; 16]) -> Result<()> {
        let k = entry_key(shard_of(entry_id), entry_id);
        let mut row = match self.ks.get(k.as_slice())? {
            Some(slice) => decode_entry_row(&slice)?,
            None => EntryRow {
                entry_id: *entry_id,
                parent_id: None,
                kind: 0xFF,
                name: String::new(),
                content_id: None,
                size: 0,
                mtime_ns: 0,
                flags: 0,
            },
        };
        row.flags |= crate::encode::FLAG_DELETED;
        self.ks.insert(k, encode_entry_row(&row)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::{KIND_DIR, KIND_FILE};

    fn tmp_root(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("hub-t03-{tag}-{}", partisync_core::Ulid::now()))
    }

    fn row(id: u8, name: &str) -> EntryRow {
        let mut entry_id = [0u8; 16];
        entry_id[0] = id;
        EntryRow {
            entry_id,
            parent_id: None,
            kind: KIND_FILE,
            name: name.into(),
            content_id: None,
            size: 42,
            mtime_ns: 1,
            flags: 0,
        }
    }

    #[test]
    fn put_get_roundtrip_and_missing() {
        let plane = HashPlane::open(&tmp_root("roundtrip")).expect("open");
        let r = row(3, "alpha.txt");
        plane.put(&r).expect("put");
        let got = plane.get(&r.entry_id).expect("get").expect("present");
        assert_eq!(got.name, "alpha.txt");
        assert_eq!(got.kind, KIND_FILE);
        assert!(!got.is_deleted());
        let missing = plane.get(&[0xEE; 16]).expect("get");
        assert!(missing.is_none());
    }

    #[test]
    fn remove_sets_tombstone_and_survives_reopen() {
        let root = tmp_root("tombstone");
        {
            let plane = HashPlane::open(&root).expect("open");
            let r = row(9, "gone.bin");
            plane.put(&r).expect("put");
            plane.remove(&r.entry_id).expect("remove");
            let got = plane
                .get(&r.entry_id)
                .expect("get")
                .expect("tombstone kept");
            assert!(got.is_deleted());
            plane.persist().expect("persist");
        }
        let plane = HashPlane::open(&root).expect("reopen");
        let got = plane
            .get(&[9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
            .expect("get")
            .expect("survives reopen");
        assert!(got.is_deleted());
    }

    #[test]
    fn resurrect_after_remove() {
        let plane = HashPlane::open(&tmp_root("resurrect")).expect("open");
        let mut r = row(5, "back.bin");
        r.kind = KIND_DIR;
        plane.put(&r).expect("put");
        plane.remove(&r.entry_id).expect("remove");
        r.flags = 0;
        plane.put(&r).expect("re-put");
        let got = plane.get(&r.entry_id).expect("get").expect("present");
        assert!(!got.is_deleted());
        assert_eq!(got.kind, KIND_DIR);
    }

    #[test]
    fn unknown_id_remove_writes_minimal_tombstone() {
        let plane = HashPlane::open(&tmp_root("minimal")).expect("open");
        let id = [0xAB; 16];
        plane.remove(&id).expect("remove");
        let got = plane.get(&id).expect("get").expect("tombstone");
        assert!(got.is_deleted());
        assert_eq!(got.name, "");
    }

    #[test]
    fn stats_reflect_shard_distribution() {
        let plane = HashPlane::open(&tmp_root("stats")).expect("open");
        // 均匀分布：256 分片各至少 1 行（1000 行足够覆盖）
        for i in 0..1000u64 {
            let mut id = [0u8; 16];
            id[..8].copy_from_slice(&i.to_be_bytes());
            let r = row(0, "x");
            let mut r2 = r.clone();
            r2.entry_id = id;
            plane.put(&r2).expect("put");
        }
        let counts = plane.stats().expect("stats");
        assert_eq!(counts.len(), 256);
        let total: u64 = counts.iter().sum();
        assert_eq!(total, 1000);
        // blake3 路由近似均匀：1000 行覆盖绝大多数分片（生日界：期望空位 ~6）
        let covered = counts.iter().filter(|c| **c > 0).count();
        assert!(covered >= 200, "expected broad coverage, got {covered}");
    }
}
