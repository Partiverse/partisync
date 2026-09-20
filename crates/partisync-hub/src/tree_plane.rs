//! 目录树 children 聚簇平面（SPEC M3-WP01 §1/§3/裁定 5）。
//!
//! 键 = `dir_id 16B ␟ name`，字典序 range 分区 + 动态分裂；值 = 子项投影行
//! （entry_id 16B + kind 1B + deleted 1B）。分区表见 [`crate::router`]，
//! 分裂/恢复协议见 [`crate::split`]。
//!
//! 本平面只做投影的原始读写；**读时修复归 [`crate::Hub`] 门面**（WP01-T05，
//! SPEC §4：以权威行为准补齐/清理投影，单次补写上限 256 行）。
//! v0.1 单写者串行（进程内 `&self` 独占）；投影陈旧窗口由 WP03 对账收口。

use std::collections::HashMap;
use std::ops::Bound;
use std::path::Path;
use std::sync::RwLock;

use fjall::{Database, Keyspace, KeyspaceCreateOptions, OwnedWriteBatch, PersistMode};

use crate::encode::EncodeError;
use crate::entry_plane::HubError;
use crate::router::{
    Router, SharedRouter, META_KEYSPACE, META_NEXT_PID, META_PARTITION_PREFIX, STATUS_ACTIVE,
};

/// 默认分裂阈值：4M 行（SPEC §3 取舍：10⁹ children ≈ 250 组 Raft，可运营带）。
pub const DEFAULT_SPLIT_THRESHOLD: u64 = 4_000_000;

/// 分裂搬移的批大小（行）——原子批内 insert+remove，崩溃即回滚。
pub(crate) const MOVE_BATCH: usize = 4096;

/// 子项投影行（children 平面值）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRow {
    /// 子条目平面 ID。
    pub entry_id: [u8; 16],
    /// kind（`KIND_FILE` / `KIND_DIR`）。
    pub kind: u8,
    /// 墓碑标记。
    pub deleted: bool,
}

impl ChildRow {
    /// 编码（固定 18B）。
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(18);
        out.extend_from_slice(&self.entry_id);
        out.push(self.kind);
        out.push(u8::from(self.deleted));
        out
    }

    /// 解码。
    ///
    /// # Errors
    /// 长度非 18B 时返回 [`EncodeError::UnexpectedEof`]。
    pub fn decode(buf: &[u8]) -> std::result::Result<Self, EncodeError> {
        if buf.len() != 18 {
            return Err(EncodeError::UnexpectedEof);
        }
        let mut entry_id = [0u8; 16];
        entry_id.copy_from_slice(&buf[0..16]);
        Ok(Self {
            entry_id,
            kind: buf[16],
            deleted: buf[17] != 0,
        })
    }
}

/// 目录树平面：meta keyspace + 分区 keyspace 组 + 内存路由器。
pub struct TreePlane {
    pub(crate) db: Database,
    pub(crate) meta: Keyspace,
    pub(crate) router: SharedRouter,
    pub(crate) keyspaces: RwLock<HashMap<u64, Keyspace>>,
    pub(crate) split_threshold: u64,
}

/// 目录 children 游标（keyset 分页：从 `last_name` 之后继续）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildCursor {
    /// 上一页最后一个子项名。
    pub last_name: String,
}

/// 子项列表项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildItem {
    /// 子项名。
    pub name: String,
    /// 子条目平面 ID。
    pub entry_id: [u8; 16],
    /// kind。
    pub kind: u8,
    /// 墓碑标记（投影行删除前短暂可见；WP03 对账收口）。
    pub deleted: bool,
}

/// 一页 children。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildrenPage {
    /// 本页子项（按名字典序）。
    pub items: Vec<ChildItem>,
    /// 续页游标（`None` = 遍历完毕）。
    pub next_cursor: Option<ChildCursor>,
}

/// 平面结果别名（复用 hub 错误集）。
pub type Result<T> = std::result::Result<T, HubError>;

/// 与查询区间相交的单个分区流：`(keyspace, 起点, 终点)`。
pub(crate) type PartitionStream = (Keyspace, Bound<Vec<u8>>, Vec<u8>);

/// 子项键 = `dir_id ␟ name`。
#[must_use]
pub fn child_key(dir_id: &[u8; 16], name: &str) -> Vec<u8> {
    let mut k = Vec::with_capacity(16 + name.len());
    k.extend_from_slice(dir_id);
    k.extend_from_slice(name.as_bytes());
    k
}

impl TreePlane {
    /// 打开（或创建）目录树平面；完成分裂恢复后提供服务。
    ///
    /// # Errors
    /// 引擎打开、meta 重建或分裂恢复失败。
    pub fn open(root: &Path) -> Result<Self> {
        Self::open_with_threshold(root, DEFAULT_SPLIT_THRESHOLD)
    }

    /// 以指定分裂阈值打开（测试注入小阈值触发多轮分裂）。
    ///
    /// # Errors
    /// 同 [`Self::open`]。
    pub fn open_with_threshold(root: &Path, threshold: u64) -> Result<Self> {
        Self::attach(Database::open(fjall::Config::new(root))?, threshold)
    }

    /// 挂接到既有 Database（Hub 单库多平面共享，fjall 单路径排他锁）。
    ///
    /// # Errors
    /// meta 重建或分裂恢复失败。
    pub fn attach(db: Database, threshold: u64) -> Result<Self> {
        let meta = db.keyspace(META_KEYSPACE, KeyspaceCreateOptions::default)?;
        let mut plane = Self {
            db,
            meta,
            router: RwLock::new(Router::empty()),
            keyspaces: RwLock::new(HashMap::new()),
            split_threshold: threshold,
        };
        {
            let (router, keyspaces) =
                Router::load(&plane.meta, &plane.db).map_err(HubError::from)?;
            *plane.router.write().expect("router lock") = router;
            *plane.keyspaces.write().expect("keyspaces lock") = keyspaces;
        }
        if plane
            .router
            .read()
            .expect("router lock")
            .partitions()
            .is_empty()
        {
            plane.create_initial_partition()?;
        }
        plane.recover_splits()?;
        Ok(plane)
    }

    fn create_initial_partition(&mut self) -> Result<()> {
        // 首分区：start = 空，pid = 1；原子批写计数器（下一个可用 pid = 2）+ 分区行
        let mut batch = OwnedWriteBatch::with_capacity(self.db.clone(), 2);
        batch.insert(&self.meta, META_NEXT_PID, 2u64.to_be_bytes());
        let mut row = Vec::with_capacity(9);
        row.extend_from_slice(&1u64.to_be_bytes());
        row.push(STATUS_ACTIVE);
        batch.insert(&self.meta, [META_PARTITION_PREFIX], row);
        batch.commit().map_err(HubError::from)?;
        let ks = self
            .db
            .keyspace("t-p1", KeyspaceCreateOptions::default)
            .map_err(HubError::from)?;
        self.keyspaces
            .write()
            .expect("keyspaces lock")
            .insert(1, ks);
        self.router.write().expect("router lock").seed_first(1);
        Ok(())
    }

    /// 写入子项投影（路由到键所属分区；行数超阈值触发分裂）。
    ///
    /// # Errors
    /// 引擎写入或分裂失败。
    pub fn put_child(&self, dir_id: &[u8; 16], name: &str, child: &ChildRow) -> Result<()> {
        let key = child_key(dir_id, name);
        let (ks, idx) = self.route(&key)?;
        ks.insert(key, child.encode()).map_err(HubError::from)?;
        let trigger = {
            let mut router = self.router.write().expect("router lock");
            router.bump(idx);
            router.counts()[idx] > self.split_threshold && !router.partitions()[idx].splitting
        };
        if trigger {
            self.split_partition(idx)?;
        }
        Ok(())
    }

    /// 删除子项投影（幂等；分裂期对源/目标两侧 keyspace 同删）。
    ///
    /// # Errors
    /// 引擎删除失败。
    pub fn remove_child(&self, dir_id: &[u8; 16], name: &str) -> Result<()> {
        let key = child_key(dir_id, name);
        let candidates: Vec<Keyspace> = {
            let router = self.router.read().expect("router lock");
            let keyspaces = self.keyspaces.read().expect("keyspaces lock");
            let idx = router.lookup(&key);
            let mut v = vec![keyspaces[&router.partitions()[idx].id].clone()];
            // 分裂期：逻辑属主与物理残留可能不同——两侧同删（幂等）
            for p in router.partitions() {
                if p.splitting && p.id != router.partitions()[idx].id {
                    if let Some(ks) = keyspaces.get(&p.id) {
                        v.push(ks.clone());
                    }
                }
            }
            v
        };
        for ks in candidates {
            ks.remove(key.as_slice()).map_err(HubError::from)?;
        }
        Ok(())
    }

    /// keyset 分页列出目录 children（按键序跨分区顺序迭代）。
    ///
    /// **原始投影读**：不校验权威行、不修复——Hub 门面在其上做读时修复与
    /// 幽灵项剔除（WP01-T05）。分裂中分区由 open 恢复路径先行收尾——
    /// 单写者串行下列表不会观察半分裂状态；`deleted` 投影行短暂可见，
    /// WP03 对账收口。
    ///
    /// # Errors
    /// 引擎迭代失败。
    pub fn list_children(
        &self,
        dir_id: &[u8; 16],
        cursor: Option<&ChildCursor>,
        limit: u32,
    ) -> Result<ChildrenPage> {
        let lo: Bound<Vec<u8>> = match cursor {
            None => Bound::Included(dir_id.to_vec()),
            // keyset 游标：严格排在上一页末项之后
            Some(c) => Bound::Excluded(child_key(dir_id, &c.last_name)),
        };
        let mut hi = dir_id.to_vec();
        hi.push(0xFF); // UTF-8 首字节 ≤ 0xF4、续字节 ≤ 0xBF——0xFF 为安全排他上界
        let mut items = Vec::with_capacity(limit as usize);
        let mut next_cursor = None;
        'outer: for (ks, s, e) in self.overlapping_streams(&lo, &hi)? {
            for guard in ks.range((s, Bound::Excluded(e.clone()))) {
                let (k, v) = guard.into_inner().map_err(HubError::from)?;
                let name = String::from_utf8(k[16..].to_vec())
                    .map_err(|_| HubError::Encode(EncodeError::InvalidNameLength))?;
                let row = ChildRow::decode(&v)?;
                items.push(ChildItem {
                    name,
                    entry_id: row.entry_id,
                    kind: row.kind,
                    deleted: row.deleted,
                });
                if items.len() == limit as usize {
                    next_cursor = items.last().map(|i| ChildCursor {
                        last_name: i.name.clone(),
                    });
                    break 'outer;
                }
            }
        }
        Ok(ChildrenPage { items, next_cursor })
    }

    /// 与 `[lo, hi)` 相交的分区键空间流（按键序；边界裁剪到分区逻辑区间）。
    fn overlapping_streams(&self, lo: &Bound<Vec<u8>>, hi: &[u8]) -> Result<Vec<PartitionStream>> {
        let router = self.router.read().expect("router lock");
        let keyspaces = self.keyspaces.read().expect("keyspaces lock");
        let parts = router.partitions();
        let mut out: Vec<PartitionStream> = Vec::new();
        for (i, p) in parts.iter().enumerate() {
            let part_end: Vec<u8> = parts
                .get(i + 1)
                .map_or_else(|| hi.to_vec(), |n| n.start.clone());
            let s: Bound<Vec<u8>> = match lo {
                Bound::Included(v) if v.as_slice() <= p.start.as_slice() => {
                    Bound::Included(p.start.clone())
                }
                Bound::Included(v) => Bound::Included(v.clone()),
                Bound::Excluded(v) if v.as_slice() < p.start.as_slice() => {
                    Bound::Included(p.start.clone())
                }
                Bound::Excluded(v) => Bound::Excluded(v.clone()),
                Bound::Unbounded => Bound::Included(p.start.clone()),
            };
            let s_ok: bool = match &s {
                Bound::Included(v) => v.as_slice() < part_end.as_slice(),
                Bound::Excluded(v) => v.as_slice() < part_end.as_slice(),
                Bound::Unbounded => true,
            };
            let e = std::cmp::min(part_end.as_slice(), hi).to_vec();
            if s_ok && !e.is_empty() {
                if let Some(ks) = keyspaces.get(&p.id) {
                    out.push((ks.clone(), s, e));
                }
            }
        }
        Ok(out)
    }

    pub(crate) fn route(&self, key: &[u8]) -> Result<(Keyspace, usize)> {
        let (pid, idx) = {
            let router = self.router.read().expect("router lock");
            let idx = router.lookup(key);
            (router.partitions()[idx].id, idx)
        };
        let ks = self
            .keyspaces
            .read()
            .expect("keyspaces lock")
            .get(&pid)
            .cloned()
            .ok_or(HubError::PartitionMissing(pid))?;
        Ok((ks, idx))
    }

    /// 读取单个投影槽位（原始读，不修复——修复归 [`crate::Hub`] 门面）。
    ///
    /// # Errors
    /// 引擎读取失败或值损坏。
    pub fn get_child(&self, dir_id: &[u8; 16], name: &str) -> Result<Option<ChildRow>> {
        let key = child_key(dir_id, name);
        let (ks, _) = self.route(&key)?;
        match ks.get(key.as_slice()).map_err(HubError::from)? {
            None => Ok(None),
            Some(guard) => Ok(Some(ChildRow::decode(&guard)?)),
        }
    }

    /// 全平面投影行内省（测试/运维）：`(dir_id, name, row)` 按分区序。
    ///
    /// # Errors
    /// 引擎迭代失败或值损坏。
    pub fn iter_child_rows(&self) -> Result<Vec<([u8; 16], String, ChildRow)>> {
        let keyspaces: Vec<Keyspace> = {
            let router = self.router.read().expect("router lock");
            let keyspaces = self.keyspaces.read().expect("keyspaces lock");
            router
                .partitions()
                .iter()
                .filter_map(|p| keyspaces.get(&p.id).cloned())
                .collect()
        };
        let mut out = Vec::new();
        for ks in keyspaces {
            for guard in ks.iter() {
                let (k, v) = guard.into_inner().map_err(HubError::from)?;
                let mut dir_id = [0u8; 16];
                dir_id.copy_from_slice(&k[0..16]);
                let name = String::from_utf8(k[16..].to_vec())
                    .map_err(|_| HubError::Encode(EncodeError::InvalidNameLength))?;
                out.push((dir_id, name, ChildRow::decode(&v)?));
            }
        }
        Ok(out)
    }

    /// 落盘。
    ///
    /// # Errors
    /// 引擎刷盘失败。
    pub fn persist(&self) -> Result<()> {
        self.db.persist(PersistMode::SyncData)?;
        Ok(())
    }

    /// 分区表内省（测试/运维）：`(pid, start_hex, splitting, count)` 按键序。
    #[must_use]
    pub fn partition_info(&self) -> Vec<(u64, String, bool, u64)> {
        let router = self.router.read().expect("router lock");
        router
            .partitions()
            .iter()
            .enumerate()
            .map(|(i, p)| {
                (
                    p.id,
                    p.start
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>(),
                    p.splitting,
                    router.counts()[i],
                )
            })
            .collect()
    }
}
