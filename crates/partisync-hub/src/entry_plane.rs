//! entry 哈希平面（SPEC M3-WP01 §1/§2/裁定 5）：单 fjall Database + 256 keyspace。
//!
//! - 权威行存储：put/get/remove 直达 `shard_of(entry_id)` 对应 keyspace；
//! - 删除 = 墓碑（`FLAG_DELETED`），行保留至 WP03 对账窗口；
//! - children 投影与读时修复归 WP01-T04/T05（tree_plane/split）；
//! - v0.1 单写者串行（进程内 `&self` 独占）；并发写语义归 WP02。

use std::path::Path;

use fjall::{Database, Keyspace, KeyspaceCreateOptions, PersistMode};

use crate::encode::{decode_entry_row, encode_entry_row, EntryRow};
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

fn keyspace_name(shard: usize) -> String {
    format!("e-{shard:03}")
}

/// entry 哈希平面：256 个 keyspace，`shard_of` 路由。
pub struct HashPlane {
    db: Database,
    keyspaces: Vec<Keyspace>,
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
        let mut keyspaces = Vec::with_capacity(SHARD_COUNT);
        for shard in 0..SHARD_COUNT {
            let ks = db.keyspace(&keyspace_name(shard), KeyspaceCreateOptions::default)?;
            keyspaces.push(ks);
        }
        Ok(Self { db, keyspaces })
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
        let mut counts = Vec::with_capacity(SHARD_COUNT);
        for ks in &self.keyspaces {
            counts.push(ks.iter().count() as u64);
        }
        Ok(counts)
    }

    /// 全平面扫描（内省/演示面用）：按分片序产出全部行（含墓碑）。
    ///
    /// # Errors
    /// 引擎读取失败或值损坏。
    pub fn iter_entries(&self) -> Result<Vec<EntryRow>> {
        let mut rows = Vec::new();
        for ks in &self.keyspaces {
            for guard in ks.iter() {
                let (k, v) = guard.into_inner().map_err(HubError::from)?;
                let mut row = decode_entry_row(&v)?;
                row.entry_id.copy_from_slice(&k);
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
        let ks = &self.keyspaces[shard_of(&row.entry_id) as usize];
        ks.insert(row.entry_id.as_slice(), encode_entry_row(row)?)?;
        Ok(())
    }

    /// 读取 entry 行（墓碑行原样返回，调用方以 [`EntryRow::is_deleted`] 判别）。
    ///
    /// # Errors
    /// 引擎读取失败或值损坏。
    pub fn get(&self, entry_id: &[u8; 16]) -> Result<Option<EntryRow>> {
        let ks = &self.keyspaces[shard_of(entry_id) as usize];
        match ks.get(entry_id.as_slice())? {
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
        let ks = &self.keyspaces[shard_of(entry_id) as usize];
        let mut row = match ks.get(entry_id.as_slice())? {
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
        ks.insert(entry_id.as_slice(), encode_entry_row(&row)?)?;
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
}
