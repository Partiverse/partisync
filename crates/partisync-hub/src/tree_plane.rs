//! 目录树 children 聚簇平面（SPEC M3-WP01 §1/§3/裁定 5）。
//!
//! 键 = `0x01 ␟ dir_id 16B ␟ name`（1B 前缀 + dir_id + name），
//! 字典序 range 分区 + 动态分裂；值 = 子项投影行（entry_id 16B + kind 1B
//! + deleted 1B）。分区表见 [`crate::router`]，分裂/恢复协议见 [`crate::split`]。
//!
//! keyspace 收敛（SPEC M3-WP02 裁定 3）：所有分区共享 `m-child` keyspace，
//! 分区逻辑边界（start_key 序）由 router 驱动，不再依赖物理 keyspace 边界。
//! 读时修复归 [`crate::Hub`] 门面（WP01-T05）；投影陈旧窗口由 WP03 对账收口。

use std::collections::HashMap;
use std::ops::Bound;
use std::path::Path;
use std::sync::RwLock;

use fjall::{Database, Keyspace, KeyspaceCreateOptions, OwnedWriteBatch, PersistMode};

use crate::encode::EncodeError;
use crate::entry_plane::HubError;
use crate::ksconv::{
    child_key as conv_child_key, legacy_layout_exists, meta_key, migrate_legacy, CHILD_PREFIX,
    KS_CHILD, KS_META,
};
use crate::router::{Router, SharedRouter, META_NEXT_PID, META_PARTITION_PREFIX, STATUS_ACTIVE};

/// 默认分裂阈值：4M 行（SPEC §3 取舍：10⁹ children ≈ 250 组 Raft，可运营带）。
pub const DEFAULT_SPLIT_THRESHOLD: u64 = 4_000_000;

/// 分裂搬移的批大小（行）——WP01 物理搬移遗留；收敛后分裂为纯元数据，
/// 本常数保留以兼容 WP01 测试/基准签名（不再参与协议路径）。
#[allow(dead_code)]
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
///
/// 收敛后：`m-child` 共享 keyspace + 前缀隔离；`m-meta` 存分区表/组注册表。
/// 路由器按 start_key 逻辑分区，不再映射物理 keyspace。
pub struct TreePlane {
    pub(crate) db: Database,
    /// 分区表/组注册表（`m-meta`）。
    pub(crate) meta: Keyspace,
    /// children 数据共享 keyspace（`m-child`）。
    pub(crate) data: Keyspace,
    pub(crate) router: SharedRouter,
    /// keyspace 收敛后只缓存 `m-child`（路由按前缀逻辑分区）。
    /// 保留字段以兼容 WP01 测试签名；分裂/恢复逻辑统一走 `data`。
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
/// 收敛后所有分区共享 `data` keyspace；此类型保留以兼容 WP01 调用方。
pub(crate) type PartitionStream = (Keyspace, Bound<Vec<u8>>, Vec<u8>);

/// 子项键 = `0x01 ␟ dir_id ␟ name`（前缀 + dir_id + name）。
#[must_use]
pub fn child_key(dir_id: &[u8; 16], name: &str) -> Vec<u8> {
    conv_child_key(CHILD_PREFIX, dir_id, name)
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
    /// 收敛后首先执行旧布局探测——`e-*`/`t-p*`/`t-meta` 存在则原地迁移
    /// 进前缀化共享 keyspace（一次性，WP01 已有库零重建）。
    ///
    /// # Errors
    /// meta 重建、旧布局迁移或分裂恢复失败。
    pub fn attach(db: Database, threshold: u64) -> Result<Self> {
        let meta = db.keyspace(KS_META, KeyspaceCreateOptions::default)?;
        let data = db.keyspace(KS_CHILD, KeyspaceCreateOptions::default)?;
        if legacy_layout_exists(&db) {
            migrate_legacy(&db, &meta).map_err(HubError::from)?;
        }
        let mut plane = Self {
            db,
            meta,
            data,
            router: RwLock::new(Router::empty()),
            keyspaces: RwLock::new(HashMap::new()),
            split_threshold: threshold,
        };
        {
            let router = Router::load(&plane.meta, &plane.data).map_err(HubError::from)?;
            *plane.router.write().expect("router lock") = router;
            // 收敛后统一填同一个 data keyspace（兼容 WP01 结构）
            for p in plane.router.read().expect("router lock").partitions() {
                plane
                    .keyspaces
                    .write()
                    .expect("keyspaces lock")
                    .insert(p.id, plane.data.clone());
            }
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
        batch.insert(&self.meta, meta_key(META_NEXT_PID), 2u64.to_be_bytes());
        let mut row = Vec::with_capacity(9);
        row.extend_from_slice(&1u64.to_be_bytes());
        row.push(STATUS_ACTIVE);
        batch.insert(&self.meta, meta_key(&[META_PARTITION_PREFIX]), row);
        batch.commit().map_err(HubError::from)?;
        self.keyspaces
            .write()
            .expect("keyspaces lock")
            .insert(1, self.data.clone());
        self.router.write().expect("router lock").seed_first(1);
        Ok(())
    }

    /// 写入子项投影（路由到键所属分区；行数超阈值触发分裂）。
    ///
    /// # Errors
    /// 引擎写入或分裂失败。
    pub fn put_child(&self, dir_id: &[u8; 16], name: &str, child: &ChildRow) -> Result<()> {
        let key = child_key(dir_id, name);
        let (_, idx) = self.route(&key)?;
        self.data
            .insert(key, child.encode())
            .map_err(HubError::from)?;
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
    /// 收敛后同一 `m-child` keyspace——源/目标为同一路由槽位，删一次即生效。
    /// 保留循环以兼容 WP01 调用方语义（幂等无副作用）。
    ///
    /// # Errors
    /// 引擎删除失败。
    pub fn remove_child(&self, dir_id: &[u8; 16], name: &str) -> Result<()> {
        let key = child_key(dir_id, name);
        self.data.remove(key.as_slice()).map_err(HubError::from)?;
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
            None => Bound::Included(child_key(dir_id, "")),
            // keyset 游标：严格排在上一页末项之后
            Some(c) => Bound::Excluded(child_key(dir_id, &c.last_name)),
        };
        let mut hi = child_key(dir_id, "");
        hi.push(0xFF); // UTF-8 首字节 ≤ 0xF4、续字节 ≤ 0xBF——0xFF 为安全排他上界
        let mut items = Vec::with_capacity(limit as usize);
        let mut next_cursor = None;
        'outer: for (ks, s, e) in self.overlapping_streams(&lo, &hi)? {
            for guard in ks.range((s, Bound::Excluded(e.clone()))) {
                let (k, v) = guard.into_inner().map_err(HubError::from)?;
                let name = String::from_utf8(k[17..].to_vec())
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
    ///
    /// 收敛后所有分区共享 `self.data`——`keyspaces` 映射保留以兼容
    /// WP01 测试/路由层签名，实际取数据统一走 `self.data`。
    fn overlapping_streams(&self, lo: &Bound<Vec<u8>>, hi: &[u8]) -> Result<Vec<PartitionStream>> {
        let router = self.router.read().expect("router lock");
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
                out.push((self.data.clone(), s, e));
            }
        }
        Ok(out)
    }

    /// 路由键所属分区下标；收敛后返回 `(data_keyspace, idx)`。
    pub(crate) fn route(&self, key: &[u8]) -> Result<(Keyspace, usize)> {
        let (idx, _) = {
            let router = self.router.read().expect("router lock");
            let idx = router.lookup(key);
            (idx, router.partitions()[idx].id)
        };
        Ok((self.data.clone(), idx))
    }

    /// 读取单个投影槽位（原始读，不修复——修复归 [`crate::Hub`] 门面）。
    ///
    /// # Errors
    /// 引擎读取失败或值损坏。
    pub fn get_child(&self, dir_id: &[u8; 16], name: &str) -> Result<Option<ChildRow>> {
        let key = child_key(dir_id, name);
        match self.data.get(key.as_slice()).map_err(HubError::from)? {
            None => Ok(None),
            Some(guard) => Ok(Some(ChildRow::decode(&guard)?)),
        }
    }

    /// 全平面投影行内省（测试/运维）：`(dir_id, name, row)` 按键序。
    ///
    /// # Errors
    /// 引擎迭代失败或值损坏。
    pub fn iter_child_rows(&self) -> Result<Vec<([u8; 16], String, ChildRow)>> {
        let mut out = Vec::new();
        for guard in self.data.iter() {
            let (k, v) = guard.into_inner().map_err(HubError::from)?;
            if k.len() > 1 && k[0] == CHILD_PREFIX {
                let mut dir_id = [0u8; 16];
                dir_id.copy_from_slice(&k[1..17]);
                let name = String::from_utf8(k[17..].to_vec())
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
