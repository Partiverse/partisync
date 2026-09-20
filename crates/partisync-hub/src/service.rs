//! Hub 业务门面（SPEC M3-WP03 T02，裁定 1）：raft 复制语义之上的
//! WP01 同构 API。
//!
//! 架构（M3-WP02 裁定 7 兑现）：entry 权威行与 children 投影是同一 raft
//! 组状态机的**业务节**——写路径 = `HubCmd` 编码进日志，`HubStateMachine`
//! apply 时调用与直连 [`Hub`] 完全相同的内核函数（`put_entry_impl` 等，
//! 语义逐字一致）；读路径 = `ensure_linearizable`（ReadIndex）+ 本机
//! 平面读（含读时修复，修复写为本地收敛，多节点路由归 T06）。
//!
//! v0.1 限定：单节点组（members={self}，bootstrap 幂等）；业务节快照未
//! 实现——`disable_auto_snapshot` 恒开（快照/安装仅覆盖通用节，业务节
//! 快照化归后续任务卡，T03+）。
//!
//! 分裂与 failpoint：SM apply 内的 children 分裂沿用 WP01 元数据-only
//! 协议（组内各副本按同序 apply 得出确定性同split）；failpoint 注入
//! 的崩溃测试语义归 T07 矩阵（raft 形态），直连形态由 WP01 存档回归。

// ReplicaError 内含 openraft API 错误（体积由上游类型决定）——同 raft_store
// 模块的豁免口径。
#![allow(clippy::result_large_err)]

use std::path::Path;
use std::time::Duration;

use tokio::runtime::Runtime;

use crate::encode::EntryRow;
use crate::entry_plane::HubError;
use crate::raft_store::{HubTypeConfig, RaftSnapshotBuilderStore, RaftStateMachineStore};
use crate::replica::{NodeConfig, Replica, ReplicaError};
use crate::{put_entry_impl, remove_entry_impl, rename_entry_impl};
use crate::{HashPlane, Hub, TreePlane};

/// 业务命令（raft 日志载荷；`HubData` = newtype-over-bytes）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubCmd {
    /// 写入权威行 + 投影（[`Hub::put_entry`] 同构）。
    Put(EntryRow),
    /// 删除（墓碑 + 投影清除；[`Hub::remove_entry`] 同构）。
    Remove([u8; 16]),
    /// 改名/移动（[`Hub::rename_entry`] 同构）。
    Rename([u8; 16], Option<[u8; 16]>, String),
}

/// 命令 tag。
const CMD_PUT: u8 = 0x01;
/// 命令 tag。
const CMD_REMOVE: u8 = 0x02;
/// 命令 tag。
const CMD_RENAME: u8 = 0x03;

/// 编码 [`HubCmd`]（紧凑二进制；EntryRow 用 WP01 行编码）。
///
/// # Errors
/// 行编码失败。
pub fn encode_cmd(cmd: &HubCmd) -> Result<Vec<u8>, HubError> {
    let mut v = Vec::with_capacity(64);
    match cmd {
        HubCmd::Put(row) => {
            v.push(CMD_PUT);
            // 行编码不含 entry_id（WP01 口径：id 在键上）——命令须显式携带
            v.extend_from_slice(&row.entry_id);
            v.extend_from_slice(&crate::encode_entry_row(row).map_err(HubError::Encode)?);
        }
        HubCmd::Remove(id) => {
            v.push(CMD_REMOVE);
            v.extend_from_slice(id);
        }
        HubCmd::Rename(id, parent, name) => {
            v.push(CMD_RENAME);
            v.extend_from_slice(id);
            v.push(u8::from(parent.is_some()));
            if let Some(p) = parent {
                v.extend_from_slice(p);
            }
            v.extend_from_slice(name.as_bytes());
        }
    }
    Ok(v)
}

/// 解码 [`HubCmd`]（[`encode_cmd`] 逆变换）。
///
/// # Errors
/// 长度/编码不合法。
pub fn decode_cmd(mut buf: &[u8]) -> Result<HubCmd, HubError> {
    let Some(&tag) = buf.first() else {
        return Err(HubError::Encode(crate::EncodeError::UnexpectedEof));
    };
    buf = &buf[1..];
    let short = HubError::Encode(crate::EncodeError::UnexpectedEof);
    match tag {
        CMD_PUT => {
            if buf.len() < 16 {
                return Err(short);
            }
            let mut id = [0u8; 16];
            id.copy_from_slice(&buf[..16]);
            let mut row = crate::decode_entry_row(&buf[16..]).map_err(HubError::Encode)?;
            row.entry_id = id;
            Ok(HubCmd::Put(row))
        }
        CMD_REMOVE => {
            if buf.len() < 16 {
                return Err(short);
            }
            let mut id = [0u8; 16];
            id.copy_from_slice(&buf[..16]);
            Ok(HubCmd::Remove(id))
        }
        CMD_RENAME => {
            if buf.len() < 17 {
                return Err(short);
            }
            let mut id = [0u8; 16];
            id.copy_from_slice(&buf[..16]);
            let has_parent = buf[16] != 0;
            buf = &buf[17..];
            let parent = if has_parent {
                if buf.len() < 16 {
                    return Err(short);
                }
                let mut p = [0u8; 16];
                p.copy_from_slice(&buf[..16]);
                buf = &buf[16..];
                Some(p)
            } else {
                None
            };
            let name = String::from_utf8(buf.to_vec())
                .map_err(|_| HubError::Encode(crate::EncodeError::InvalidNameLength))?;
            Ok(HubCmd::Rename(id, parent, name))
        }
        _ => Err(short),
    }
}

/// Hub 业务状态机 = 通用节（applied 指针/membership，[`RaftStateMachineStore`]）
/// + 业务节（entry/children 平面，apply 调用与直连 [`Hub`] 相同的内核）。
pub struct HubStateMachine {
    inner: RaftStateMachineStore,
    entry: HashPlane,
    tree: TreePlane,
}

impl HubStateMachine {
    /// 挂接到既有 Database（业务节平面与门面读共享同一 keyspace）。
    ///
    /// # Errors
    /// 平面打开失败。
    pub fn attach(
        db: fjall::Database,
        inner: RaftStateMachineStore,
        split_threshold: u64,
    ) -> Result<Self, HubError> {
        Ok(Self {
            inner,
            entry: HashPlane::attach(db.clone())?,
            tree: TreePlane::attach(db, split_threshold)?,
        })
    }
}

impl openraft::storage::RaftStateMachine<HubTypeConfig> for HubStateMachine {
    type SnapshotBuilder = RaftSnapshotBuilderStore;

    async fn applied_state(
        &mut self,
    ) -> Result<
        (
            Option<openraft::LogId<u64>>,
            openraft::StoredMembership<u64, openraft::BasicNode>,
        ),
        openraft::StorageError<u64>,
    > {
        self.inner.applied_state().await
    }

    async fn apply<I>(
        &mut self,
        entries: I,
    ) -> Result<Vec<crate::HubResponse>, openraft::StorageError<u64>>
    where
        I: IntoIterator<Item = openraft::Entry<HubTypeConfig>> + openraft::OptionalSend,
        I::IntoIter: openraft::OptionalSend,
    {
        let cloned: Vec<openraft::Entry<HubTypeConfig>> = entries.into_iter().collect();
        let mut responses = self.inner.apply(cloned.clone()).await?;
        // 业务节：与直连 Hub 同内核逐条 apply（单写者 log 序 = WP01 串行语义）。
        // 业务语义拒绝（rename 目标缺失/墓碑）= apply 侧 no-op + 应答标志位
        // （apply 错误在 openraft 中是 fatal——不可承载业务错误）
        for (i, e) in cloned.iter().enumerate() {
            if let openraft::EntryPayload::Normal(d) = &e.payload {
                let cmd = decode_cmd(&d.0).map_err(business_error)?;
                let outcome: Result<(), HubError> = match cmd {
                    HubCmd::Put(row) => put_entry_impl(&self.entry, &self.tree, &row),
                    HubCmd::Remove(id) => {
                        let prior = self.entry.get(&id).map_err(business_error)?;
                        remove_entry_impl(&self.entry, &self.tree, &id, prior.as_ref())
                    }
                    HubCmd::Rename(id, parent, name) => {
                        match self.entry.get(&id).map_err(business_error)? {
                            Some(row) if !row.is_deleted() => {
                                rename_entry_impl(&self.entry, &self.tree, &row, parent, &name)
                            }
                            _ => Err(HubError::EntryMissing), // no-op + 标志位
                        }
                    }
                };
                match outcome {
                    Ok(()) => responses[i] = crate::HubResponse(vec![0x00]),
                    Err(HubError::EntryMissing) => responses[i] = crate::HubResponse(vec![0x01]),
                    Err(e) => return Err(business_error(e)),
                }
            }
        }
        Ok(responses)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        self.inner.get_snapshot_builder().await
    }

    async fn begin_receiving_snapshot(
        &mut self,
    ) -> Result<
        Box<<HubTypeConfig as openraft::RaftTypeConfig>::SnapshotData>,
        openraft::StorageError<u64>,
    > {
        self.inner.begin_receiving_snapshot().await
    }

    async fn install_snapshot(
        &mut self,
        meta: &openraft::SnapshotMeta<u64, openraft::BasicNode>,
        snapshot: Box<<HubTypeConfig as openraft::RaftTypeConfig>::SnapshotData>,
    ) -> Result<(), openraft::StorageError<u64>> {
        // v0.1：快照仅覆盖通用节（disable_auto_snapshot 恒开——见模块文档）
        self.inner.install_snapshot(meta, snapshot).await
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<openraft::Snapshot<HubTypeConfig>>, openraft::StorageError<u64>> {
        self.inner.get_current_snapshot().await
    }
}

/// 业务平面错误 → openraft SM IO 错误（openraft SM 无业务错误通道；
/// 调用方经日志与 ACK 结果感知）。
fn business_error(e: HubError) -> openraft::StorageError<u64> {
    let io = std::io::Error::other(e.to_string());
    openraft::StorageError::IO {
        source: openraft::StorageIOError::new(
            openraft::ErrorSubject::StateMachine,
            openraft::ErrorVerb::Write,
            openraft::AnyError::new(&io),
        ),
    }
}

/// raft 复制语义之上的 Hub 门面（WP01 API 同构；v0.1 单节点组）。
pub struct HubService {
    rt: Runtime,
    replica: Replica,
    hub: Hub,
}

/// 门面配置。
#[derive(Debug, Clone)]
pub struct HubServiceConfig {
    /// 数据根目录（fjall DB + raft 锁同库）。
    pub root: std::path::PathBuf,
    /// children 分裂阈值。
    pub split_threshold: u64,
    /// 选举超时区间 ms（演练/测试注入）。
    pub election_timeout_ms: (u64, u64),
    /// 心跳间隔 ms。
    pub heartbeat_interval_ms: u64,
}

impl HubService {
    /// 打开（默认阈值 4M 行；选举/心跳默认 300-600/50ms）。
    ///
    /// # Errors
    /// 存储打开、raft 启动或 bootstrap/选举失败。
    pub fn open(root: &Path) -> Result<Self, ReplicaError> {
        Self::open_with_config(HubServiceConfig {
            root: root.to_path_buf(),
            split_threshold: crate::DEFAULT_SPLIT_THRESHOLD,
            election_timeout_ms: (300, 600),
            heartbeat_interval_ms: 50,
        })
    }

    /// 以指定分裂阈值打开（测试注入小阈值触发多轮分裂）。
    ///
    /// # Errors
    /// 同 [`Self::open`]。
    pub fn open_with_threshold(root: &Path, threshold: u64) -> Result<Self, ReplicaError> {
        Self::open_with_config(HubServiceConfig {
            root: root.to_path_buf(),
            split_threshold: threshold,
            election_timeout_ms: (300, 600),
            heartbeat_interval_ms: 50,
        })
    }

    /// 全量配置打开：单节点组（node_id=1，members={self}），bootstrap 幂等，
    /// 等待本组选出 leader 后返回。
    ///
    /// # Errors
    /// 存储打开、raft 启动、bootstrap 或选举等待失败。
    pub fn open_with_config(cfg: HubServiceConfig) -> Result<Self, ReplicaError> {
        let rt = Runtime::new().map_err(|e| ReplicaError::Io(std::io::Error::other(e)))?;
        let (replica, hub) = rt.block_on(open_async(&cfg))?;
        Ok(Self { rt, replica, hub })
    }

    /// 写入 entry（raft 线性一致：commit+apply 后应答）。
    ///
    /// # Errors
    /// raft 错误。
    pub fn put_entry(&self, row: &EntryRow) -> Result<(), ReplicaError> {
        let cmd = encode_cmd(&HubCmd::Put(row.clone()))
            .map_err(|e| ReplicaError::Io(std::io::Error::other(e.to_string())))?;
        self.rt.block_on(async {
            let h = self.replica.submit(cmd).await?;
            h.ack().await?;
            Ok(())
        })
    }

    /// 读取 entry（线性一致 + 读时修复，WP01 §4 同构）。
    ///
    /// # Errors
    /// raft 或引擎错误。
    pub fn get_entry(&self, entry_id: &[u8; 16]) -> Result<Option<EntryRow>, ReplicaError> {
        self.rt.block_on(self.replica.ensure_linearizable())?;
        self.hub.get_entry(entry_id).map_err(into_replica)
    }

    /// 删除 entry（raft 线性一致）。
    ///
    /// # Errors
    /// raft 错误。
    pub fn remove_entry(&self, entry_id: &[u8; 16]) -> Result<(), ReplicaError> {
        let cmd = encode_cmd(&HubCmd::Remove(*entry_id))
            .map_err(|e| ReplicaError::Io(std::io::Error::other(e.to_string())))?;
        self.rt.block_on(async {
            let h = self.replica.submit(cmd).await?;
            h.ack().await?;
            Ok(())
        })
    }

    /// 改名/移动（raft 线性一致；O(1)——不重写后代）。
    ///
    /// # Errors
    /// raft 或引擎错误。
    pub fn rename_entry(
        &self,
        entry_id: &[u8; 16],
        new_parent: Option<[u8; 16]>,
        new_name: &str,
    ) -> Result<(), ReplicaError> {
        let cmd = encode_cmd(&HubCmd::Rename(*entry_id, new_parent, new_name.to_owned()))
            .map_err(|e| ReplicaError::Io(std::io::Error::other(e.to_string())))?;
        self.rt.block_on(async {
            let h = self.replica.submit(cmd).await?;
            let (_, resp) = h.ack_with_response().await?;
            if resp.first() == Some(&0x01) {
                return Err(ReplicaError::Io(std::io::Error::other(
                    "entry missing or tombstoned",
                )));
            }
            Ok(())
        })
    }

    /// keyset 分页列出 children（线性一致 + 读时修复，WP01 §4 同构）。
    ///
    /// # Errors
    /// raft 或引擎错误。
    pub fn list_children(
        &self,
        dir_id: &[u8; 16],
        cursor: Option<&crate::ChildCursor>,
        limit: u32,
    ) -> Result<crate::ChildrenPage, ReplicaError> {
        self.rt.block_on(self.replica.ensure_linearizable())?;
        self.hub
            .list_children(dir_id, cursor, limit)
            .map_err(into_replica)
    }

    /// 子树遍历（线性一致起点；BFS + 环防护，WP01 §4 同构）。
    pub fn subtree(&self, dir_id: &[u8; 16]) -> Vec<EntryRow> {
        self.rt.block_on(self.replica.ensure_linearizable()).ok();
        self.hub.subtree(dir_id).collect()
    }

    /// 落盘。
    ///
    /// # Errors
    /// 引擎刷盘失败。
    pub fn persist(&self) -> Result<(), ReplicaError> {
        self.hub.persist().map_err(into_replica)
    }

    /// 停止 raft core（演练/测试用：进程死亡形态，无优雅交接）。
    pub fn crash(&self) {
        self.rt.block_on(self.replica.crash());
    }

    /// 分区表内省（直连读；测试/运维）。
    ///
    /// 门面 TreePlane 的内存路由是打开时刻的快照——SM apply 侧的分裂只落
    /// m-meta（跨实例不广播），故此处**从 m-meta 重载**而非读内存态。
    pub fn hub_partition_info(&self) -> Vec<(u64, String, bool, u64)> {
        let router = crate::router::Router::load(&self.hub.tree.meta, &self.hub.tree.data);
        match router {
            Ok(r) => r
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
                        r.counts()[i],
                    )
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// 测试故障注入：直连写投影槽位（绕过 raft——制造幽灵/陈旧投影，
    /// 供读时修复面验证；生产路径不经过此方法）。
    ///
    /// # Errors
    /// 引擎写入失败。
    pub fn hub_direct_tree_put_ghost(
        &self,
        dir_id: &[u8; 16],
        name: &str,
        entry_id: [u8; 16],
    ) -> Result<(), ReplicaError> {
        self.hub
            .tree
            .put_child(
                dir_id,
                name,
                &crate::ChildRow {
                    entry_id,
                    kind: crate::KIND_FILE,
                    deleted: false,
                },
            )
            .map_err(into_replica)
    }

    /// 测试故障注入：直连删投影槽位（绕过 raft——制造缺失投影）。
    ///
    /// # Errors
    /// 引擎删除失败。
    pub fn hub_direct_tree_remove(
        &self,
        dir_id: &[u8; 16],
        name: &str,
    ) -> Result<(), ReplicaError> {
        self.hub
            .tree
            .remove_child(dir_id, name)
            .map_err(into_replica)
    }
}

/// 异步打开：建库/平面 → 业务状态机 → raft → bootstrap → 等leader。
async fn open_async(cfg: &HubServiceConfig) -> Result<(Replica, Hub), ReplicaError> {
    let db = fjall::Database::open(fjall::Config::new(&cfg.root))?;
    let hub = Hub::attach(db.clone(), cfg.split_threshold).map_err(into_replica)?;
    let (_, sm_generic) = crate::open_raft_stores(&db, 1)?;
    let sm = HubStateMachine::attach(db.clone(), sm_generic, cfg.split_threshold)
        .map_err(into_replica)?;
    let node = NodeConfig {
        node_id: 1,
        addr: "127.0.0.1:0".to_owned(),
        db_root: cfg.root.clone(),
        group_id: 1,
        members: [(1u64, "127.0.0.1:0".to_owned())].into_iter().collect(),
        election_timeout_ms: cfg.election_timeout_ms,
        heartbeat_interval_ms: cfg.heartbeat_interval_ms,
        disable_auto_snapshot: true,
    };
    let replica = Replica::open_with_sm(&node, db, sm).await?;
    replica.bootstrap().await?;
    replica.wait_leader(Duration::from_secs(10)).await?;
    Ok((replica, hub))
}

fn into_replica(e: HubError) -> ReplicaError {
    ReplicaError::Io(std::io::Error::other(e.to_string()))
}
