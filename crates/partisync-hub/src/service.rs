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
use crate::registry::{RegistryError, RegistryService};
use crate::replica::{NodeConfig, Replica, ReplicaError};
use crate::{put_entry_impl, remove_entry_impl, rename_entry_impl};
use crate::{HashPlane, Hub, TreePlane};
use partisync_sync::reconcile::{LeafKind, LeafSource, StateLeaf};

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

/// 对账影子写命令（修复 sink 经 raft 入日志；载荷 = serde_json 本结构）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShadowUpsert {
    /// 本批影子行（幂等 upsert：按 key 覆盖）。
    pub rows: Vec<ShadowRow>,
}

/// Hub 业务状态机 = 通用节（applied 指针/membership，[`RaftStateMachineStore`]）
/// + 业务节（entry/children 平面 + 对账影子节，apply 调用与直连 [`Hub`] 相同的内核）。
pub struct HubStateMachine {
    inner: RaftStateMachineStore,
    entry: HashPlane,
    tree: TreePlane,
    shadow: fjall::Keyspace,
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
            tree: TreePlane::attach(db.clone(), split_threshold)?,
            shadow: db
                .keyspace(KS_SHADOW, fjall::KeyspaceCreateOptions::default)
                .map_err(HubError::Fjall)?,
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
                if let Ok(cmd) = decode_cmd(&d.0) {
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
                        Err(HubError::EntryMissing) => {
                            responses[i] = crate::HubResponse(vec![0x01]);
                        }
                        Err(e) => return Err(business_error(e)),
                    }
                } else if let Ok(cmd) = serde_json::from_slice::<ShadowUpsert>(&d.0) {
                    // 对账影子节：修复 sink 经 raft 应用（裁定 7——同 log 全副本收敛）
                    for row in &cmd.rows {
                        let json = serde_json::to_vec(row)
                            .map_err(|e| business_error_msg(format!("shadow encode: {e}")))?;
                        self.shadow
                            .insert(row.key.as_bytes(), json)
                            .map_err(|e| business_error_msg(format!("shadow write: {e}")))?;
                    }
                    responses[i] = crate::HubResponse(vec![0x00]);
                } else {
                    return Err(business_error_msg(format!(
                        "unrecognized payload at index {i}"
                    )));
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
fn business_error_msg(msg: impl std::fmt::Display) -> openraft::StorageError<u64> {
    let io = std::io::Error::other(msg.to_string());
    openraft::StorageError::IO {
        source: openraft::StorageIOError::new(
            openraft::ErrorSubject::StateMachine,
            openraft::ErrorVerb::Write,
            openraft::AnyError::new(&io),
        ),
    }
}

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

/// 对账影子节 keyspace（组 1 业务节第二段——裁定 7：同 log 各节按序应用）。
pub const KS_SHADOW: &str = "r-shadow";
/// 水位行键前缀（`\x00wm/{origin}` → hlc）。
pub const WM_PREFIX: u8 = 0x00;

/// 影子行（对账叶的持久形态；serde JSON 进 `r-shadow`）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShadowRow {
    /// 叶键（entry=path / `tag/{id}` / `link/{tag}␟{path}`）。
    pub key: String,
    /// 叶哈希（graph::merkle 口径）。
    pub hash: String,
    /// 完整字段集（修复 sink 需要全量字段）。
    pub kind: ShadowKind,
}

/// 影子行字段集（与 sync `LeafKind` 一一对应；serde 标签形态）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum ShadowKind {
    /// entry 叶。
    Entry {
        /// 全路径。
        path: String,
        /// 1=dir 2=file（graph 口径）。
        kind: i64,
        /// 内容 id（blake3 hex）。
        content: Option<String>,
        /// 属主设备。
        owner: Option<String>,
        /// 字节。
        size: i64,
        /// mtime。
        mtime_ns: i64,
    },
    /// tag 叶。
    Tag {
        /// tag id。
        id: String,
        /// 名称。
        name: String,
        /// 颜色。
        color: Option<String>,
        /// 删除标记。
        deleted: bool,
    },
    /// link 叶。
    Link {
        /// tag id。
        tag_id: String,
        /// entry 全路径。
        entry_path: String,
        /// 删除标记。
        deleted: bool,
    },
}

impl From<&LeafKind> for ShadowKind {
    fn from(k: &LeafKind) -> Self {
        match k {
            LeafKind::Entry {
                path,
                kind,
                content,
                owner,
                size,
                mtime_ns,
            } => Self::Entry {
                path: path.clone(),
                kind: *kind,
                content: content.clone(),
                owner: owner.clone(),
                size: *size,
                mtime_ns: *mtime_ns,
            },
            LeafKind::Tag {
                id,
                name,
                color,
                deleted,
            } => Self::Tag {
                id: id.clone(),
                name: name.clone(),
                color: color.clone(),
                deleted: *deleted,
            },
            LeafKind::Link {
                tag_id,
                entry_path,
                deleted,
            } => Self::Link {
                tag_id: tag_id.clone(),
                entry_path: entry_path.clone(),
                deleted: *deleted,
            },
        }
    }
}

impl From<&ShadowKind> for LeafKind {
    fn from(k: &ShadowKind) -> Self {
        match k {
            ShadowKind::Entry {
                path,
                kind,
                content,
                owner,
                size,
                mtime_ns,
            } => Self::Entry {
                path: path.clone(),
                kind: *kind,
                content: content.clone(),
                owner: owner.clone(),
                size: *size,
                mtime_ns: *mtime_ns,
            },
            ShadowKind::Tag {
                id,
                name,
                color,
                deleted,
            } => Self::Tag {
                id: id.clone(),
                name: name.clone(),
                color: color.clone(),
                deleted: *deleted,
            },
            ShadowKind::Link {
                tag_id,
                entry_path,
                deleted,
            } => Self::Link {
                tag_id: tag_id.clone(),
                entry_path: entry_path.clone(),
                deleted: *deleted,
            },
        }
    }
}

/// raft 复制语义之上的 Hub 门面（WP01 API 同构；v0.1 单节点组）。
///
/// 同库承载两个 raft 组：数据组（pid=1，[`Self::hub`] 平面）与
/// 空间注册表组（pid=0，[`Self::registry`]，裁定 4）。
pub struct HubService {
    rt: Runtime,
    replica: Replica,
    hub: Hub,
    registry: RegistryService,
    shadow: fjall::Keyspace,
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

/// 空间路由决策（SPEC M5-WP02 T06/契约 4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteDecision {
    /// 本 hub 即持有方：操作就地继续。
    Local,
    /// 非 本 hub 空间：重定向到持有 hub。
    Redirect {
        /// 持有 hub id。
        hub_id: u64,
        /// 持有 hub 可达地址（路由行 `addr`）。
        addr: String,
    },
    /// 联邦视图未知——升级为 Redirect/Local 由联邦层 `resolve_space` /
    /// `claim_space` 完成（service 层纯本地视图，不持网络）。
    Unknown,
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
        let (replica, hub, registry, shadow) = rt.block_on(open_async(&cfg))?;
        Ok(Self {
            rt,
            replica,
            hub,
            registry,
            shadow,
        })
    }

    /// 空间注册表服务（全局组 pid=0；裁定 4）。
    #[must_use]
    pub fn registry(&self) -> &RegistryService {
        &self.registry
    }

    /// 当前 leader 提示（演示面用；None = 选举中）。
    #[must_use]
    pub fn registry_leader_hint(&self) -> Option<u64> {
        self.replica.current_leader()
    }

    /// 空间路由门（SPEC M5-WP02 T06/契约 4）：读本地 raft 视图（线性一致）
    /// 三分类——本 hub 持有 → [`RouteDecision::Local`]；他 hub 持有 →
    /// [`RouteDecision::Redirect`]；未知 → [`RouteDecision::Unknown`]。
    ///
    /// # Errors
    /// raft 读失败。
    pub fn route_for(
        &self,
        space_id: &str,
        self_hub_id: u64,
    ) -> Result<RouteDecision, ReplicaError> {
        self.rt
            .block_on(self.route_for_async(space_id, self_hub_id))
    }

    /// [`Self::route_for`] 的异步形态。
    ///
    /// # Errors
    /// raft 读失败。
    pub async fn route_for_async(
        &self,
        space_id: &str,
        self_hub_id: u64,
    ) -> Result<RouteDecision, ReplicaError> {
        match self.registry.route_async(space_id).await {
            Ok(Some(row)) if row.hub_id == self_hub_id => Ok(RouteDecision::Local),
            Ok(Some(row)) => Ok(RouteDecision::Redirect {
                hub_id: row.hub_id,
                addr: row.addr,
            }),
            Ok(None) => Ok(RouteDecision::Unknown),
            Err(RegistryError::Raft(re)) => Err(*re),
            Err(other) => Err(ReplicaError::Io(std::io::Error::other(other.to_string()))),
        }
    }

    /// 写入 entry（raft 线性一致：commit+apply 后应答）。
    ///
    /// # Errors
    /// raft 错误。
    pub fn put_entry(&self, row: &EntryRow) -> Result<(), ReplicaError> {
        self.rt.block_on(self.put_entry_async(row))
    }

    /// [`Self::put_entry`] 的异步形态（运行在调用方 runtime 上）。
    ///
    /// # Errors
    /// raft 错误。
    pub async fn put_entry_async(&self, row: &EntryRow) -> Result<(), ReplicaError> {
        let cmd = encode_cmd(&HubCmd::Put(row.clone()))
            .map_err(|e| ReplicaError::Io(std::io::Error::other(e.to_string())))?;
        let h = self.replica.submit(cmd).await?;
        h.ack().await?;
        Ok(())
    }

    /// 读取 entry（线性一致 + 读时修复，WP01 §4 同构）。
    ///
    /// # Errors
    /// raft 或引擎错误。
    pub fn get_entry(&self, entry_id: &[u8; 16]) -> Result<Option<EntryRow>, ReplicaError> {
        self.rt.block_on(self.get_entry_async(entry_id))
    }

    /// [`Self::get_entry`] 的异步形态。
    ///
    /// # Errors
    /// raft 或引擎错误。
    pub async fn get_entry_async(
        &self,
        entry_id: &[u8; 16],
    ) -> Result<Option<EntryRow>, ReplicaError> {
        self.replica.ensure_linearizable().await?;
        self.hub.get_entry(entry_id).map_err(into_replica)
    }

    /// 删除 entry（raft 线性一致）。
    ///
    /// # Errors
    /// raft 错误。
    pub fn remove_entry(&self, entry_id: &[u8; 16]) -> Result<(), ReplicaError> {
        self.rt.block_on(self.remove_entry_async(entry_id))
    }

    /// [`Self::remove_entry`] 的异步形态。
    ///
    /// # Errors
    /// raft 错误。
    pub async fn remove_entry_async(&self, entry_id: &[u8; 16]) -> Result<(), ReplicaError> {
        let cmd = encode_cmd(&HubCmd::Remove(*entry_id))
            .map_err(|e| ReplicaError::Io(std::io::Error::other(e.to_string())))?;
        let h = self.replica.submit(cmd).await?;
        h.ack().await?;
        Ok(())
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
        self.rt
            .block_on(self.rename_entry_async(entry_id, new_parent, new_name))
    }

    /// [`Self::rename_entry`] 的异步形态。
    ///
    /// # Errors
    /// raft 或引擎错误。
    pub async fn rename_entry_async(
        &self,
        entry_id: &[u8; 16],
        new_parent: Option<[u8; 16]>,
        new_name: &str,
    ) -> Result<(), ReplicaError> {
        let cmd = encode_cmd(&HubCmd::Rename(*entry_id, new_parent, new_name.to_owned()))
            .map_err(|e| ReplicaError::Io(std::io::Error::other(e.to_string())))?;
        let h = self.replica.submit(cmd).await?;
        let (_, resp) = h.ack_with_response().await?;
        if resp.first() == Some(&0x01) {
            return Err(ReplicaError::Io(std::io::Error::other(
                "entry missing or tombstoned",
            )));
        }
        Ok(())
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
        self.rt
            .block_on(self.list_children_async(dir_id, cursor, limit))
    }

    /// [`Self::list_children`] 的异步形态。
    ///
    /// # Errors
    /// raft 或引擎错误。
    pub async fn list_children_async(
        &self,
        dir_id: &[u8; 16],
        cursor: Option<&crate::ChildCursor>,
        limit: u32,
    ) -> Result<crate::ChildrenPage, ReplicaError> {
        self.replica.ensure_linearizable().await?;
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
        self.rt.block_on(self.crash_async());
    }

    /// [`Self::crash`] 的异步形态（可从外部 runtime 直接 await）。
    pub async fn crash_async(&self) {
        self.replica.crash().await;
    }

    /// 设备↔hub Merkle 对账（裁定 2 + 裁定 7）：hub 侧以
    /// [`RaftLeafSource`]（影子节）参战——设备叶集为真相源之一，
    /// 修复写经 raft 全副本收敛。
    ///
    /// # Errors
    /// 协议轮次内未收敛（max_rounds 耗尽）或引擎错误。
    pub async fn reconcile_with_device<S: LeafSource>(
        &self,
        device: &S,
        opts: partisync_sync::reconcile::ReconcileOpts,
    ) -> Result<partisync_sync::reconcile::ReconcileStats, ReplicaError> {
        let hub_src = RaftLeafSource::new(self);
        partisync_sync::reconcile::reconcile(device, &hub_src, opts)
            .await
            .map_err(|e| {
                eprintln!("reconcile failed: {e}");
                into_replica(HubError::Encode(crate::EncodeError::UnexpectedEof))
            })
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

/// 异步打开：建库/平面 → 注册表组（pid=0）→ 业务状态机 → 数据组
/// （pid=1）→ bootstrap → 等 leader。
async fn open_async(
    cfg: &HubServiceConfig,
) -> Result<(Replica, Hub, RegistryService, fjall::Keyspace), ReplicaError> {
    let db = fjall::Database::open(fjall::Config::new(&cfg.root))?;
    let hub = Hub::attach(db.clone(), cfg.split_threshold).map_err(into_replica)?;
    let registry = RegistryService::open_on(
        &db,
        &cfg.root,
        cfg.election_timeout_ms,
        cfg.heartbeat_interval_ms,
    )
    .await
    .map_err(|e| ReplicaError::Io(std::io::Error::other(e.to_string())))?;
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
    let shadow = replica
        .database()
        .keyspace(KS_SHADOW, fjall::KeyspaceCreateOptions::default)
        .map_err(|e| into_replica(HubError::Fjall(e)))?;
    Ok((replica, hub, registry, shadow))
}

impl HubService {
    /// 对账影子叶扫描（线性一致确认后直读 `r-shadow`，键序 = Merkle 序）。
    ///
    /// # Errors
    /// raft 或引擎错误。
    pub async fn shadow_scan_async(&self) -> Result<Vec<StateLeaf>, ReplicaError> {
        self.replica.ensure_linearizable().await?;
        let mut out = Vec::new();
        for guard in self.shadow.iter() {
            let (k, v) = guard
                .into_inner()
                .map_err(|e| into_replica(HubError::Fjall(e)))?;
            if k.first() == Some(&WM_PREFIX) {
                continue; // 水位行不进叶集
            }
            let row: ShadowRow = serde_json::from_slice(&v).map_err(|e| {
                eprintln!("shadow row decode failed (key={k:02x?}): {e}");
                into_replica(HubError::Encode(crate::EncodeError::UnexpectedEof))
            })?;
            out.push(StateLeaf {
                key: row.key,
                hash: row.hash,
                kind: LeafKind::from(&row.kind),
            });
        }
        Ok(out)
    }

    /// 影子行 upsert（经 raft；`LeafSource` 修复 sink 的写通道）。
    ///
    /// # Errors
    /// raft 错误。
    pub async fn shadow_upsert_async(&self, rows: Vec<ShadowRow>) -> Result<(), ReplicaError> {
        let cmd = serde_json::to_vec(&ShadowUpsert { rows })
            .map_err(|_e| into_replica(HubError::Encode(crate::EncodeError::UnexpectedEof)))?;
        let h = self.replica.submit(cmd).await?;
        h.ack().await?;
        Ok(())
    }

    /// 水位表读取（` wm/{origin}` 特殊行）。
    async fn watermarks_async(&self) -> Result<Vec<(String, String)>, ReplicaError> {
        self.replica.ensure_linearizable().await?;
        let mut out = Vec::new();
        for guard in self.shadow.prefix([WM_PREFIX]) {
            let (k, v) = guard
                .into_inner()
                .map_err(|e| into_replica(HubError::Fjall(e)))?;
            let origin = String::from_utf8(k[1..].to_vec())
                .map_err(|_e| into_replica(HubError::Encode(crate::EncodeError::UnexpectedEof)))?;
            let hlc = String::from_utf8(v.to_vec())
                .map_err(|_e| into_replica(HubError::Encode(crate::EncodeError::UnexpectedEof)))?;
            out.push((origin, hlc));
        }
        Ok(out)
    }

    /// 水位写入（经 raft；finalize_watermarks 的 hub 侧通道）。
    async fn note_applied_async(&self, origin: &str, hlc: &str) -> Result<(), ReplicaError> {
        let mut key = vec![WM_PREFIX];
        key.extend_from_slice(origin.as_bytes());
        self.shadow
            .insert(key.as_slice(), hlc.as_bytes())
            .map_err(|e| into_replica(HubError::Fjall(e)))
    }
}

/// hub 侧对账叶源适配器（裁定 2）：把 [`LeafSource`] 的叶扫描/修复 sink
/// 映射到 HubService 的影子节（读 = 线性一致；写 = 经 raft 的影子 upsert）。
///
/// 语义注记（v0.1）：hub 影子 = **镜像**（last-writer-wins 覆盖，不产生
/// P11 冲突）——属主决胜发生在设备侧，hub 侧冲突行以普通叶收录。
pub struct RaftLeafSource<'a> {
    svc: &'a HubService,
}

impl<'a> RaftLeafSource<'a> {
    /// 包装门面。
    #[must_use]
    pub fn new(svc: &'a HubService) -> Self {
        Self { svc }
    }
}

impl LeafSource for RaftLeafSource<'_> {
    async fn device_id(&self) -> Result<String, partisync_core::error::PartisyError> {
        Ok(format!("hub-{}", self.svc.replica.node_id()))
    }

    async fn watermarks(
        &self,
    ) -> Result<Vec<(String, String)>, partisync_core::error::PartisyError> {
        self.svc.watermarks_async().await.map_err(partisy)
    }

    async fn clock_top(&self) -> Result<Option<String>, partisync_core::error::PartisyError> {
        // hub 影子自身不产生写（镜像）——时钟顶恒 None
        Ok(None)
    }

    async fn note_applied(
        &self,
        origin: &str,
        hlc: &str,
    ) -> Result<(), partisync_core::error::PartisyError> {
        self.svc
            .note_applied_async(origin, hlc)
            .await
            .map_err(partisy)
    }

    async fn entry_state_leaves(
        &self,
    ) -> Result<
        Vec<(String, i64, Option<String>, Option<String>, i64, i64)>,
        partisync_core::error::PartisyError,
    > {
        self.svc
            .replica
            .ensure_linearizable()
            .await
            .map_err(partisy)?;
        let mut out = Vec::new();
        for leaf in self.svc.shadow_scan_async().await.map_err(partisy)? {
            if let LeafKind::Entry {
                path,
                kind,
                content,
                owner,
                size,
                mtime_ns,
            } = leaf.kind
            {
                out.push((path, kind, content, owner, size, mtime_ns));
            }
        }
        Ok(out)
    }

    async fn tag_state_leaves(
        &self,
    ) -> Result<Vec<(String, String, Option<String>, i64)>, partisync_core::error::PartisyError>
    {
        self.svc
            .replica
            .ensure_linearizable()
            .await
            .map_err(partisy)?;
        let mut out = Vec::new();
        for leaf in self.svc.shadow_scan_async().await.map_err(partisy)? {
            if let LeafKind::Tag {
                id,
                name,
                color,
                deleted,
            } = leaf.kind
            {
                out.push((id, name, color, i64::from(deleted)));
            }
        }
        Ok(out)
    }

    async fn link_state_leaves(
        &self,
    ) -> Result<Vec<(String, String, i64)>, partisync_core::error::PartisyError> {
        self.svc
            .replica
            .ensure_linearizable()
            .await
            .map_err(partisy)?;
        let mut out = Vec::new();
        for leaf in self.svc.shadow_scan_async().await.map_err(partisy)? {
            if let LeafKind::Link {
                tag_id,
                entry_path,
                deleted,
            } = leaf.kind
            {
                out.push((tag_id, entry_path, i64::from(deleted)));
            }
        }
        Ok(out)
    }

    async fn apply_remote_entry(
        &self,
        path: &str,
        name: &str,
        kind: partisync_graph::store::EntryKind,
        size: u64,
        mtime_ns: u64,
        content: Option<(&str, u64)>,
        chunk_root: Option<&str>,
        owner_device: &str,
    ) -> Result<partisync_graph::store::ApplyOutcome, partisync_core::error::PartisyError> {
        let _ = (name, chunk_root); // 影子叶只承载状态全量字段（与叶哈希口径一致）
        let kind_i = match kind {
            partisync_graph::store::EntryKind::Dir => 1,
            partisync_graph::store::EntryKind::File => 0,
        };
        let leaf = partisync_graph::merkle::entry_leaf(
            path,
            kind_i,
            content.map(|(h, _)| h),
            Some(owner_device),
            size,
            mtime_ns,
        );
        let row = ShadowRow {
            key: leaf.key,
            hash: leaf.hash,
            kind: ShadowKind::from(&LeafKind::Entry {
                path: path.to_owned(),
                kind: kind_i,
                content: content.map(|(h, _)| h.to_owned()),
                owner: Some(owner_device.to_owned()),
                size: size as i64,
                mtime_ns: mtime_ns as i64,
            }),
        };
        self.svc
            .shadow_upsert_async(vec![row])
            .await
            .map_err(partisy)?;
        Ok(partisync_graph::store::ApplyOutcome {
            path: path.to_owned(),
            conflict: None, // hub 影子 = 镜像覆盖（见 impl 块文档）
        })
    }

    async fn apply_remote_tag(
        &self,
        id: &str,
        name: &str,
        color: Option<&str>,
        deleted: bool,
        hlc_key: &str,
    ) -> Result<bool, partisync_core::error::PartisyError> {
        let _ = hlc_key;
        let leaf = partisync_graph::merkle::tag_leaf(id, name, color, deleted);
        let row = ShadowRow {
            key: leaf.key,
            hash: leaf.hash,
            kind: ShadowKind::from(&LeafKind::Tag {
                id: id.to_owned(),
                name: name.to_owned(),
                color: color.map(str::to_owned),
                deleted,
            }),
        };
        self.svc
            .shadow_upsert_async(vec![row])
            .await
            .map_err(partisy)?;
        Ok(true)
    }

    async fn apply_remote_tag_link(
        &self,
        tag_id: &str,
        entry_path: &str,
        deleted: bool,
        hlc_key: &str,
    ) -> Result<bool, partisync_core::error::PartisyError> {
        let _ = hlc_key;
        let leaf = partisync_graph::merkle::link_leaf(tag_id, entry_path, deleted);
        let row = ShadowRow {
            key: leaf.key,
            hash: leaf.hash,
            kind: ShadowKind::from(&LeafKind::Link {
                tag_id: tag_id.to_owned(),
                entry_path: entry_path.to_owned(),
                deleted,
            }),
        };
        self.svc
            .shadow_upsert_async(vec![row])
            .await
            .map_err(partisy)?;
        Ok(true)
    }

    async fn record_conflict(
        &self,
        space_id: &str,
        base_path: &str,
        local_path: &str,
        incoming_path: &str,
        origin_device: &str,
        detected_hlc: &str,
    ) -> Result<String, partisync_core::error::PartisyError> {
        // hub 影子不产生冲突（镜像语义）——血缘留痕以影子行记录（P11 审计面）
        let key = format!(
            "\x00conflict/{space_id}/{}",
            uuid_like(base_path, incoming_path, detected_hlc)
        );
        let row = ShadowRow {
            key: base_path.to_owned(),
            hash: String::new(),
            kind: ShadowKind::Entry {
                path: local_path.to_owned(),
                kind: 2,
                content: Some(incoming_path.to_owned()),
                owner: Some(origin_device.to_owned()),
                size: 0,
                mtime_ns: 0,
            },
        };
        let _ = key;
        self.svc
            .shadow_upsert_async(vec![row])
            .await
            .map_err(partisy)?;
        Ok(String::new())
    }
}

fn uuid_like(a: &str, b: &str, c: &str) -> String {
    use std::fmt::Write;
    let mut h = blake3::Hasher::new();
    h.update(a.as_bytes());
    h.update(b.as_bytes());
    h.update(c.as_bytes());
    let d = h.finalize();
    let mut s = String::new();
    for b in &d.as_bytes()[..8] {
        write!(s, "{b:02x}").unwrap();
    }
    s
}

/// ReplicaError → PartisyError（Fatal + source 链）。
fn partisy(e: ReplicaError) -> partisync_core::error::PartisyError {
    partisync_core::error::PartisyError {
        severity: partisync_core::error::Severity::Fatal,
        source: Some(Box::new(e)),
    }
}

fn into_replica(e: HubError) -> ReplicaError {
    ReplicaError::Io(std::io::Error::other(e.to_string()))
}
