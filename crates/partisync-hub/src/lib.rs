//! Hub：分片元数据(fjall)、openraft 复制、联邦路由、分层与 EC
//!
//! 行为契约见 docs/specs/ 对应工作包规格。当前实现：M3-WP01 T03/T04
//! （entry 哈希平面 + children range 平面 + 动态分裂 + Hub 门面）。
//! rename/subtree/读时修复归 T05；基准报告归 T06。

pub mod encode;
pub mod entry_plane;
pub mod router;
pub mod shard;
pub mod split;
pub mod tree_plane;

pub use encode::{
    decode_entry_row, encode_entry_row, EncodeError, EntryRow, FLAG_DELETED, KIND_DIR, KIND_FILE,
};
pub use entry_plane::{HashPlane, HubError};
pub use router::Partition;
pub use shard::{shard_of, SHARD_COUNT};
pub use tree_plane::{
    ChildCursor, ChildItem, ChildRow, ChildrenPage, TreePlane, DEFAULT_SPLIT_THRESHOLD,
};

use std::path::Path;

/// Hub 门面（SPEC M3-WP01 §4）：entry 权威平面 + children 投影平面。
///
/// 跨平面为投影、无事务（SPEC 裁定 3）——投影暂缺由 T05 读时修复兜底、
/// WP03 对账收口。v0.1 单写者串行。
pub struct Hub {
    /// entry 哈希平面（权威行）。
    pub entry: HashPlane,
    /// children 聚簇平面（投影）。
    pub tree: TreePlane,
}

impl Hub {
    /// 打开（默认分裂阈值 4M 行）。
    ///
    /// # Errors
    /// 任一平面打开失败。
    pub fn open(root: &Path) -> entry_plane::Result<Self> {
        Self::open_with_threshold(root, DEFAULT_SPLIT_THRESHOLD)
    }

    /// 以指定分裂阈值打开（测试注入小阈值触发多轮分裂）。
    ///
    /// # Errors
    /// 同 [`Self::open`]。
    pub fn open_with_threshold(root: &Path, threshold: u64) -> entry_plane::Result<Self> {
        // fjall 单路径排他锁：两平面共享同一 Database（单库多 keyspace，裁定 5）
        let db = fjall::Database::open(fjall::Config::new(root))?;
        Ok(Self {
            entry: HashPlane::attach(db.clone())?,
            tree: TreePlane::attach(db, threshold)?,
        })
    }

    /// 写入 entry：权威行 + children 投影（有父目录时）。
    ///
    /// # Errors
    /// 任一平面写入失败。
    pub fn put_entry(&self, row: &EntryRow) -> entry_plane::Result<()> {
        self.entry.put(row)?;
        if let Some(parent) = row.parent_id {
            self.tree.put_child(
                &parent,
                &row.name,
                &ChildRow {
                    entry_id: row.entry_id,
                    kind: row.kind,
                    deleted: row.is_deleted(),
                },
            )?;
        }
        Ok(())
    }

    /// 读取 entry 权威行（墓碑原样返回）。
    ///
    /// # Errors
    /// 引擎读取失败或值损坏。
    pub fn get_entry(&self, entry_id: &[u8; 16]) -> entry_plane::Result<Option<EntryRow>> {
        self.entry.get(entry_id)
    }

    /// 删除 entry：权威行置墓碑（保留至 WP03 对账窗口）+ children 投影删除。
    ///
    /// # Errors
    /// 任一平面操作失败。
    pub fn remove_entry(&self, entry_id: &[u8; 16]) -> entry_plane::Result<()> {
        let row = self.entry.get(entry_id)?;
        self.entry.remove(entry_id)?;
        if let Some(row) = row {
            if let Some(parent) = row.parent_id {
                self.tree.remove_child(&parent, &row.name)?;
            }
        }
        Ok(())
    }

    /// keyset 分页列出目录 children。
    ///
    /// # Errors
    /// 引擎迭代失败。
    pub fn list_children(
        &self,
        dir_id: &[u8; 16],
        cursor: Option<&ChildCursor>,
        limit: u32,
    ) -> entry_plane::Result<ChildrenPage> {
        self.tree.list_children(dir_id, cursor, limit)
    }

    /// 落盘。
    ///
    /// # Errors
    /// 任一平面刷盘失败。
    pub fn persist(&self) -> entry_plane::Result<()> {
        self.entry.persist()?;
        self.tree.persist()
    }
}
