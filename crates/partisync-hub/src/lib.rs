//! Hub：分片元数据(fjall)、openraft 复制、联邦路由、分层与 EC
//!
//! 行为契约见 docs/specs/ 对应工作包规格。当前实现：M3-WP01 T03/T04/T05
//! （entry 哈希平面 + children range 平面 + 动态分裂 + Hub 门面 + 树操作/
//! 读时修复）；M3-WP02 T02/T03（keyspace 收敛 + openraft storage-v2 →
//! fjall 存储适配器）；基准报告归 T06/T07。

pub mod acl;
pub mod encode;
pub mod entry_plane;
pub mod iroh_channel;
pub mod ksconv;
pub mod net;
pub mod raft_store;
pub mod registry;
pub mod replica;
pub mod router;
pub mod service;
pub mod shard;
pub mod split;
pub mod tree_plane;

pub use encode::{
    decode_entry_row, encode_entry_row, EncodeError, EntryRow, FLAG_DELETED, KIND_DIR, KIND_FILE,
};
pub use entry_plane::{HashPlane, HubError};
pub use net::{
    read_frame, serve, write_frame, NetFactory, NetRequest, NetResponse, TcpNetwork, FRAME_VERSION,
    MAX_FRAME_BYTES,
};
pub use raft_store::{
    group_prefix, open_raft_stores, HubData, HubResponse, HubTypeConfig, RaftLogReaderStore,
    RaftLogStore, RaftSnapshotBuilderStore, RaftStateMachineStore, KS_RAFT_LOG, KS_RAFT_META,
    KS_RAFT_SM,
};
pub use router::Partition;
pub use shard::{shard_of, SHARD_COUNT};
pub use tree_plane::{
    ChildCursor, ChildItem, ChildRow, ChildrenPage, TreePlane, DEFAULT_SPLIT_THRESHOLD,
};

use std::collections::{HashSet, VecDeque};
use std::path::Path;

/// 单次读操作修复补写上限（行）——SPEC §4：防大目录读放大击穿 300B 写放大
/// 口径（256 = 单目录 p99 LIST 页深×页大小，SEC-AUDIT-2026-M3-003 §二.2.4）。
/// 超出部分留待后续读触发继续修复；正确性最终由 WP03 对账兜底。
pub const READ_REPAIR_CAP: u32 = 256;

/// subtree 逐目录展开的单页大小（内部分页常数）。
const SUBTREE_PAGE: u32 = 256;

/// Hub 门面（SPEC M3-WP01 §4）：entry 权威平面 + children 投影平面。
///
/// 跨平面为投影、无事务（SPEC 裁定 3）——崩溃间隙产生的投影暂缺/残留由
/// 读时修复兜底（[`Self::get_entry`]/[`Self::list_children`]，T05）、
/// WP03 对账收口。v0.1 单写者串行；调用方负责同目录命名唯一性（POSIX
/// 语义），同槽异主属调用方违约，投影以最后写者为准。
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
        Self::attach(db, threshold)
    }

    /// 挂接到既有 Database（raft 门面 [`crate::service::HubService`] 复用
    /// 同一读取/修复实现）。
    ///
    /// # Errors
    /// 任一平面打开失败。
    pub(crate) fn attach(db: fjall::Database, threshold: u64) -> entry_plane::Result<Self> {
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
        put_entry_impl(&self.entry, &self.tree, row)
    }

    /// 读取 entry 权威行（墓碑原样返回），并做读时修复（SPEC §4）：
    /// 以权威行核对其投影槽位——缺失则补齐、陈旧则改写、本条目墓碑残留则清除。
    ///
    /// # Errors
    /// 引擎读取失败或值损坏。
    pub fn get_entry(&self, entry_id: &[u8; 16]) -> entry_plane::Result<Option<EntryRow>> {
        let Some(row) = self.entry.get(entry_id)? else {
            return Ok(None);
        };
        self.repair_slot_for(&row)?;
        Ok(Some(row))
    }

    /// 读时修复（单行）：以权威行核对其 children 槽位并补齐/清理。
    fn repair_slot_for(&self, row: &EntryRow) -> entry_plane::Result<()> {
        let Some(parent) = row.parent_id else {
            return Ok(()); // 根条目无投影
        };
        let slot = self.tree.get_child(&parent, &row.name)?;
        if row.is_deleted() {
            // 墓碑：清除本条目残留的投影行（异主槽位是调用方命名违约，不动）
            if slot.as_ref().is_some_and(|c| c.entry_id == row.entry_id) {
                self.tree.remove_child(&parent, &row.name)?;
            }
        } else {
            let desired = ChildRow {
                entry_id: row.entry_id,
                kind: row.kind,
                deleted: false,
            };
            // 槽位缺席或内容陈旧 → 补齐/改写；槽位已被其他活跃条目持有 → 不动
            let foreign = slot.as_ref().is_some_and(|c| c.entry_id != row.entry_id);
            if !foreign && slot.as_ref() != Some(&desired) {
                self.tree.put_child(&parent, &row.name, &desired)?;
            }
        }
        Ok(())
    }

    /// 删除 entry：权威行置墓碑（保留至 WP03 对账窗口）+ children 投影删除。
    ///
    /// # Errors
    /// 任一平面操作失败。
    pub fn remove_entry(&self, entry_id: &[u8; 16]) -> entry_plane::Result<()> {
        let row = self.entry.get(entry_id)?;
        remove_entry_impl(&self.entry, &self.tree, entry_id, row.as_ref())
    }

    /// 改名/移动（SPEC §4，O(1)——裁定 2）：只写本条目权威行 + 两侧投影迁移，
    /// **不重写任何后代**（后代 parent 指针是 entry id，与名字解耦）。
    ///
    /// `new_parent = None` 表示移到根（无投影）。中间崩溃的间隙由读时修复
    /// 收敛：旧槽残留 → [`Self::list_children`] 幽灵清理；新槽缺失 →
    /// [`Self::get_entry`] 补齐。目标条目不存在或已墓碑时返回
    /// [`HubError::EntryMissing`]。
    ///
    /// # Errors
    /// 目标缺失/墓碑，或任一平面操作失败。
    pub fn rename_entry(
        &self,
        entry_id: &[u8; 16],
        new_parent: Option<[u8; 16]>,
        new_name: &str,
    ) -> entry_plane::Result<()> {
        let row = self.entry.get(entry_id)?.ok_or(HubError::EntryMissing)?;
        if row.is_deleted() {
            return Err(HubError::EntryMissing);
        }
        rename_entry_impl(&self.entry, &self.tree, &row, new_parent, new_name)
    }

    /// keyset 分页列出目录 children，读时修复生效（SPEC §4）：
    /// 每个投影槽位对照权威行——幽灵项（权威行缺失/墓碑/已迁走）剔除并
    /// 清除槽位，陈项改写，返回项一律以权威行构造。修复补写受
    /// [`READ_REPAIR_CAP`] 限制，超出部分留待后续读继续。
    ///
    /// # Errors
    /// 引擎迭代失败。
    pub fn list_children(
        &self,
        dir_id: &[u8; 16],
        cursor: Option<&ChildCursor>,
        limit: u32,
    ) -> entry_plane::Result<ChildrenPage> {
        let mut items = Vec::with_capacity(limit as usize);
        let mut cursor = cursor.cloned();
        let mut budget = READ_REPAIR_CAP;
        loop {
            let raw = self.tree.list_children(dir_id, cursor.as_ref(), limit)?;
            let exhausted = raw.next_cursor.is_none();
            for item in raw.items {
                let slot = ChildRow {
                    entry_id: item.entry_id,
                    kind: item.kind,
                    deleted: item.deleted,
                };
                let Some(row) = self.validate_child(dir_id, &item.name, slot, &mut budget)? else {
                    continue; // 幽灵项：不进页
                };
                items.push(ChildItem {
                    name: row.name.clone(),
                    entry_id: row.entry_id,
                    kind: row.kind,
                    deleted: row.is_deleted(),
                });
                if items.len() == limit as usize {
                    break;
                }
            }
            if items.len() == limit as usize || exhausted {
                break;
            }
            cursor = raw.next_cursor; // 本页被幽灵耗尽：按原始游标续扫
        }
        // 契约（T04 起）：next_cursor Some ⟺ 满页——游标=末项名才严格续得上
        let next_cursor = if items.len() == limit as usize {
            items.last().map(|i| ChildCursor {
                last_name: i.name.clone(),
            })
        } else {
            None
        };
        Ok(ChildrenPage { items, next_cursor })
    }

    /// 核对单个投影槽位与权威行并按需修复（计预算）；返回 `None` 表示
    /// 幽灵槽位（权威行缺失/墓碑/已迁走）——预算内清除，不进返回页。
    fn validate_child(
        &self,
        dir_id: &[u8; 16],
        name: &str,
        slot: ChildRow,
        budget: &mut u32,
    ) -> entry_plane::Result<Option<EntryRow>> {
        let row = match self.entry.get(&slot.entry_id)? {
            Some(r) if !r.is_deleted() => r,
            _ => {
                if *budget > 0 {
                    *budget -= 1;
                    self.tree.remove_child(dir_id, name)?;
                }
                return Ok(None);
            }
        };
        if row.parent_id.as_ref() != Some(dir_id) || row.name != name {
            // 已迁走残留：现行槽位已由 put/rename 写就，此为崩溃间隙孤儿
            if *budget > 0 {
                *budget -= 1;
                self.tree.remove_child(dir_id, name)?;
            }
            return Ok(None);
        }
        let desired = ChildRow {
            entry_id: row.entry_id,
            kind: row.kind,
            deleted: false,
        };
        if slot != desired && *budget > 0 {
            *budget -= 1;
            self.tree.put_child(dir_id, name, &desired)?;
        }
        Ok(Some(row))
    }

    /// 子树遍历（SPEC §4/裁定 4）：children 递归迭代器，BFS（先根、层内按
    /// 名字典序）；途经 [`Self::list_children`] 分页，读时修复照常生效，
    /// 返回行一律为权威行。引擎错误使迭代终止（v0.1 契约——正确性由
    /// WP03 对账兜底）。防御性 visited 集合兜底调用方构造的 parent 环。
    pub fn subtree(&self, dir_id: &[u8; 16]) -> impl Iterator<Item = EntryRow> + '_ {
        SubtreeIter {
            hub: self,
            root: *dir_id,
            root_done: false,
            queue: VecDeque::new(),
            buf: VecDeque::new(),
            visited: HashSet::new(),
        }
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

/// [`Hub::subtree`] 的 BFS 迭代器（逐目录展开，根缺失/墓碑 = 空子树）。
struct SubtreeIter<'a> {
    hub: &'a Hub,
    root: [u8; 16],
    root_done: bool,
    queue: VecDeque<[u8; 16]>,
    buf: VecDeque<EntryRow>,
    visited: HashSet<[u8; 16]>,
}

impl Iterator for SubtreeIter<'_> {
    type Item = EntryRow;

    fn next(&mut self) -> Option<EntryRow> {
        loop {
            if let Some(row) = self.buf.pop_front() {
                return Some(row);
            }
            if !self.root_done {
                self.root_done = true;
                if self.visited.insert(self.root) {
                    self.queue.push_back(self.root);
                }
                return match self.hub.get_entry(&self.root) {
                    Ok(Some(row)) if !row.is_deleted() => Some(row),
                    Ok(_) => None,
                    Err(_) => None, // 引擎错误：迭代终止（见 subtree doc）
                };
            }
            let dir = self.queue.pop_front()?;
            let mut cursor = None;
            loop {
                let page = match self.hub.list_children(&dir, cursor.as_ref(), SUBTREE_PAGE) {
                    Ok(page) => page,
                    Err(_) => return None,
                };
                let exhausted = page.next_cursor.is_none();
                // 列表页已被门面按权威行修复过滤；此处仅回读权威行内容
                for item in &page.items {
                    let row = match self.hub.entry.get(&item.entry_id) {
                        Ok(Some(r)) if !r.is_deleted() => r,
                        Ok(_) => continue,
                        Err(_) => return None,
                    };
                    if row.kind == KIND_DIR && self.visited.insert(row.entry_id) {
                        self.queue.push_back(row.entry_id);
                    }
                    self.buf.push_back(row);
                }
                if exhausted {
                    break;
                }
                cursor = page.next_cursor;
            }
        }
    }
}

/// [`Hub::put_entry`] 内核（raft 状态机 apply 复用同一实现——语义逐字一致）。
///
/// # Errors
/// 任一平面写入失败。
pub(crate) fn put_entry_impl(
    entry: &HashPlane,
    tree: &TreePlane,
    row: &EntryRow,
) -> entry_plane::Result<()> {
    entry.put(row)?;
    if let Some(parent) = row.parent_id {
        tree.put_child(
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

/// [`Hub::remove_entry`] 内核（`prior` = 删除前的权威行，供投影定位）。
///
/// # Errors
/// 任一平面操作失败。
pub(crate) fn remove_entry_impl(
    entry: &HashPlane,
    tree: &TreePlane,
    entry_id: &[u8; 16],
    prior: Option<&EntryRow>,
) -> entry_plane::Result<()> {
    entry.remove(entry_id)?;
    if let Some(row) = prior {
        if let Some(parent) = row.parent_id {
            tree.remove_child(&parent, &row.name)?;
        }
    }
    Ok(())
}

/// [`Hub::rename_entry`] 内核（`row` = 改名前权威行，须存在且非墓碑）。
///
/// # Errors
/// 任一平面操作失败。
pub(crate) fn rename_entry_impl(
    entry: &HashPlane,
    tree: &TreePlane,
    row: &EntryRow,
    new_parent: Option<[u8; 16]>,
    new_name: &str,
) -> entry_plane::Result<()> {
    if let Some(old_parent) = row.parent_id {
        tree.remove_child(&old_parent, &row.name)?;
    }
    let mut new_row = row.clone();
    new_row.parent_id = new_parent;
    new_row.name = new_name.to_owned();
    entry.put(&new_row)?;
    if let Some(new_parent) = new_parent {
        tree.put_child(
            &new_parent,
            new_name,
            &ChildRow {
                entry_id: row.entry_id,
                kind: row.kind,
                deleted: false,
            },
        )?;
    }
    Ok(())
}
