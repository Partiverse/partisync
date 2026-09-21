//! 空间注册表（SPEC M3-WP03 T03，裁定 3/4/5）：独立全局 raft 组
//! （group_id=0）承载 `space_id → SpaceRow`，含 D2 盐接线与角色 ACL。
//!
//! - **注册表行**：`SpaceRow{ root_entry_id, members, kdf_salt, created_at }`
//!   持久化于 `r-space` keyspace（serde_json，量级 ≤10⁴ 行）；
//! - **D2 盐接线**：`create_space` 门面侧以 CSPRNG（`sync::crypto::
//!   random_kdf_salt`，SEC-AUDIT P2-3 原语）生成盐并随命令入日志——SM
//!   apply 确定性（不得用 CSPRNG），盐必须由客户端供给；
//! - **角色**：owner（管成员）/editor（写）/viewer（只读）；SM 侧强制
//!   （成员管理须 actor=Owner；移除最后一名 Owner 拒绝——防锁定），
//!   业务语义拒绝 = apply 侧 no-op + 应答标志位（同 T02 口径）；
//! - **跨组无事务**（裁定 4）：注册表组与数据组两步非原子，「空间根
//!   存在性」单调不变量由测试显式覆盖（见 wp03 t03_*）。
//!
//! 设备身份 = M2 pairing 的 ed25519 验证密钥（[`DeviceId`]，hex 承载）；
//! 传输层签名信封归 T05。

// RegistryError 内含 ReplicaError（openraft API 错误，体积上游决定）——
// 同 raft_store/service 模块的豁免口径。
#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use fjall::Database;
use partisync_sync::crypto::random_kdf_salt;
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;

use crate::raft_store::{HubTypeConfig, RaftSnapshotBuilderStore, RaftStateMachineStore};
use crate::replica::{NodeConfig, Replica, ReplicaError};

/// 空间注册表 keyspace（组 0 业务节）。
pub const KS_SPACE: &str = "r-space";
/// 注册表组 id（pid=0；数据分区组自 1 起）。
pub const REGISTRY_GROUP: u64 = 0;

/// 应答标志：成功。
const FLAG_OK: u8 = 0x00;
/// 应答标志：空间已存在。
const FLAG_EXISTS: u8 = 0x01;
/// 应答标志：被角色规则拒绝。
const FLAG_FORBIDDEN: u8 = 0x02;
/// 应答标志：目标（空间/成员）不存在。
const FLAG_MISSING: u8 = 0x03;

/// 空间角色（裁定 5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// 管成员/删空间。
    Owner,
    /// 数据写。
    Editor,
    /// 只读。
    Viewer,
}

/// 设备身份 = ed25519 验证密钥（M2 pairing；hex 字符串承载于注册表行）。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceId(pub [u8; 32]);

impl DeviceId {
    /// hex 编码（64 字符小写）。
    #[must_use]
    pub fn to_hex(self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// hex 解码。
    ///
    /// # Errors
    /// 长度非 64 或含非 hex 字符。
    pub fn from_hex(s: &str) -> Result<Self, RegistryError> {
        let b = s.as_bytes();
        if b.len() != 64 {
            return Err(RegistryError::Io("device id hex length != 64".into()));
        }
        let mut out = [0u8; 32];
        for (i, chunk) in b.chunks(2).enumerate() {
            let hi = (chunk[0] as char)
                .to_digit(16)
                .ok_or_else(|| RegistryError::Io("device id hex digit invalid".into()))?;
            let lo = (chunk[1] as char)
                .to_digit(16)
                .ok_or_else(|| RegistryError::Io("device id hex digit invalid".into()))?;
            out[i] = (hi * 16 + lo) as u8;
        }
        Ok(Self(out))
    }
}

impl Serialize for DeviceId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for DeviceId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        DeviceId::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl fmt::Debug for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DeviceId({})", self.to_hex())
    }
}

/// 空间注册表行（裁定 4）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceRow {
    /// 空间根 entry（裁定 3：归属由 parent 链自根界定）。
    pub root_entry_id: [u8; 16],
    /// 成员表（hex(device vk) → 角色）。
    pub members: BTreeMap<String, Role>,
    /// D2：CSPRNG 持久盐（创建时生成，master_key 派生域分离）。
    pub kdf_salt: [u8; 16],
    /// 创建时间（unix ns）。
    pub created_at_ns: i64,
}

/// 注册表命令（serde_json 信封进 raft 日志）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegistryCmd {
    /// 创建空间（存在 → EXISTS no-op）。
    CreateSpace {
        /// 操作者（首任 Owner）。
        owner: DeviceId,
        /// 空间 id。
        space_id: String,
        /// 空间根 entry。
        root: [u8; 16],
        /// D2 盐（门面 CSPRNG 生成——SM apply 确定性）。
        salt: [u8; 16],
        /// 创建时间。
        created_at_ns: i64,
    },
    /// 设置成员角色（须 Owner；最后一名 Owner 不可降/移）。
    SetMember {
        /// 操作者。
        actor: DeviceId,
        /// 空间 id。
        space_id: String,
        /// 目标设备。
        target: DeviceId,
        /// 目标角色。
        role: Role,
    },
    /// 移除成员（须 Owner；最后一名 Owner 不可移）。
    RemoveMember {
        /// 操作者。
        actor: DeviceId,
        /// 空间 id。
        space_id: String,
        /// 目标设备。
        target: DeviceId,
    },
}

/// 注册表操作（读侧权限判定用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// 读（成员全员）。
    Read,
    /// 数据写（editor+）。
    Write,
    /// 成员管理（owner）。
    Admin,
}

/// 注册表错误集。
#[derive(Debug)]
pub enum RegistryError {
    /// 空间已存在。
    Exists,
    /// 空间/成员不存在。
    Missing,
    /// 角色规则拒绝。
    Forbidden,
    /// raft/引擎错误。
    Raft(Box<ReplicaError>),
    /// 编码/身份格式错误。
    Io(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exists => write!(f, "registry: space exists"),
            Self::Missing => write!(f, "registry: missing"),
            Self::Forbidden => write!(f, "registry: forbidden"),
            Self::Raft(e) => write!(f, "registry raft: {e}"),
            Self::Io(e) => write!(f, "registry io: {e}"),
        }
    }
}

impl std::error::Error for RegistryError {}

impl From<ReplicaError> for RegistryError {
    fn from(e: ReplicaError) -> Self {
        Self::Raft(Box::new(e))
    }
}

/// 应答标志 → 业务错误（ok 除外）。
fn flag_to_error(flag: u8) -> Result<(), RegistryError> {
    match flag {
        FLAG_OK => Ok(()),
        FLAG_EXISTS => Err(RegistryError::Exists),
        FLAG_FORBIDDEN => Err(RegistryError::Forbidden),
        FLAG_MISSING => Err(RegistryError::Missing),
        other => Err(RegistryError::Io(format!("unknown flag {other}"))),
    }
}

/// 注册表状态机 = 通用节 + `r-space` 业务节。
pub struct RegistryStateMachine {
    inner: RaftStateMachineStore,
    spaces: fjall::Keyspace,
}

/// 业务拒绝/成功 → 应答字节。
fn business_error(e: impl fmt::Display) -> openraft::StorageError<u64> {
    let io = std::io::Error::other(e.to_string());
    openraft::StorageError::IO {
        source: openraft::StorageIOError::new(
            openraft::ErrorSubject::StateMachine,
            openraft::ErrorVerb::Write,
            openraft::AnyError::new(&io),
        ),
    }
}

impl openraft::storage::RaftStateMachine<HubTypeConfig> for RegistryStateMachine {
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
        for (i, e) in cloned.iter().enumerate() {
            if let openraft::EntryPayload::Normal(d) = &e.payload {
                let cmd: RegistryCmd = serde_json::from_slice(&d.0)
                    .map_err(|e| business_error(format!("registry cmd decode: {e}")))?;
                match cmd {
                    RegistryCmd::CreateSpace {
                        owner,
                        space_id,
                        root,
                        salt,
                        created_at_ns,
                    } => {
                        if self
                            .spaces
                            .get(space_id.as_bytes())
                            .map_err(business_error)?
                            .is_some()
                        {
                            responses[i] = crate::HubResponse(vec![FLAG_EXISTS]);
                        } else {
                            let mut members = BTreeMap::new();
                            members.insert(owner.to_hex(), Role::Owner);
                            let row = SpaceRow {
                                root_entry_id: root,
                                members,
                                kdf_salt: salt,
                                created_at_ns,
                            };
                            let json = serde_json::to_vec(&row)
                                .map_err(|e| business_error(format!("row encode: {e}")))?;
                            self.spaces
                                .insert(space_id.as_bytes(), json)
                                .map_err(business_error)?;
                            responses[i] = crate::HubResponse(vec![FLAG_OK]);
                        }
                    }
                    RegistryCmd::SetMember {
                        actor,
                        space_id,
                        target,
                        role,
                    } => {
                        let raw = self
                            .spaces
                            .get(space_id.as_bytes())
                            .map_err(business_error)?;
                        let Some(raw) = raw else {
                            responses[i] = crate::HubResponse(vec![FLAG_MISSING]);
                            continue;
                        };
                        let mut row: SpaceRow = serde_json::from_slice(&raw)
                            .map_err(|e| business_error(format!("row decode: {e}")))?;
                        if row.members.get(&actor.to_hex()) != Some(&Role::Owner) {
                            responses[i] = crate::HubResponse(vec![FLAG_FORBIDDEN]);
                            continue;
                        }
                        if row.members.get(&target.to_hex()) == Some(&Role::Owner)
                            && role != Role::Owner
                            && row.members.values().filter(|r| **r == Role::Owner).count() == 1
                        {
                            // 最后一名 Owner 不可降
                            responses[i] = crate::HubResponse(vec![FLAG_FORBIDDEN]);
                            continue;
                        }
                        row.members.insert(target.to_hex(), role);
                        put_row(&self.spaces, &space_id, &row)?;
                        responses[i] = crate::HubResponse(vec![FLAG_OK]);
                    }
                    RegistryCmd::RemoveMember {
                        actor,
                        space_id,
                        target,
                    } => {
                        let raw = self
                            .spaces
                            .get(space_id.as_bytes())
                            .map_err(business_error)?;
                        let Some(raw) = raw else {
                            responses[i] = crate::HubResponse(vec![FLAG_MISSING]);
                            continue;
                        };
                        let mut row: SpaceRow = serde_json::from_slice(&raw)
                            .map_err(|e| business_error(format!("row decode: {e}")))?;
                        if row.members.get(&actor.to_hex()) != Some(&Role::Owner) {
                            responses[i] = crate::HubResponse(vec![FLAG_FORBIDDEN]);
                            continue;
                        }
                        if row.members.get(&target.to_hex()) != Some(&Role::Owner)
                            || row.members.values().filter(|r| **r == Role::Owner).count() > 1
                        {
                            row.members.remove(&target.to_hex());
                            put_row(&self.spaces, &space_id, &row)?;
                            responses[i] = crate::HubResponse(vec![FLAG_OK]);
                        } else {
                            // 最后一名 Owner 不可移
                            responses[i] = crate::HubResponse(vec![FLAG_FORBIDDEN]);
                        }
                    }
                };
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
        // v0.1：快照仅覆盖通用节（注册表组恒禁自动快照——业务节快照化归后续卡）
        self.inner.install_snapshot(meta, snapshot).await
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<openraft::Snapshot<HubTypeConfig>>, openraft::StorageError<u64>> {
        self.inner.get_current_snapshot().await
    }
}

fn put_row(
    spaces: &fjall::Keyspace,
    space_id: &str,
    row: &SpaceRow,
) -> Result<(), openraft::StorageError<u64>> {
    let json = serde_json::to_vec(row).map_err(|e| business_error(format!("row encode: {e}")))?;
    spaces
        .insert(space_id.as_bytes(), json)
        .map_err(business_error)
}

/// 空间注册表服务（全局组 pid=0）。
pub struct RegistryService {
    handle: Handle,
    replica: Replica,
}

impl RegistryService {
    /// 在既有 Database 上打开注册表组（pid=0；bootstrap 幂等；等 leader）。
    ///
    /// # Errors
    /// raft 启动/bootstrap/选举失败。
    pub async fn open_on(
        db: &Database,
        root: &std::path::Path,
        election_timeout_ms: (u64, u64),
        heartbeat_interval_ms: u64,
    ) -> Result<Self, RegistryError> {
        let (_, sm_generic) = crate::open_raft_stores(db, REGISTRY_GROUP)
            .map_err(|e| RegistryError::Io(e.to_string()))?;
        let spaces = db
            .keyspace(KS_SPACE, fjall::KeyspaceCreateOptions::default)
            .map_err(|e| RegistryError::Io(e.to_string()))?;
        let sm = RegistryStateMachine {
            inner: sm_generic,
            spaces,
        };
        let node = NodeConfig {
            node_id: 1,
            addr: "127.0.0.1:0".to_owned(),
            db_root: root.to_path_buf(),
            group_id: REGISTRY_GROUP,
            members: [(1u64, "127.0.0.1:0".to_owned())].into_iter().collect(),
            election_timeout_ms,
            heartbeat_interval_ms,
            disable_auto_snapshot: true,
        };
        let replica = Replica::open_with_sm(&node, db.clone(), sm).await?;
        replica.bootstrap().await?;
        replica.wait_leader(Duration::from_secs(10)).await?;
        Ok(Self {
            handle: Handle::current(),
            replica,
        })
    }

    /// 创建空间（D2：盐由门面 CSPRNG 生成；创建者 = 首任 Owner）。
    ///
    /// # Errors
    /// 已存在（`Exists`）或 raft 错误。
    pub fn create_space(
        &self,
        space_id: &str,
        root_entry_id: [u8; 16],
        owner: DeviceId,
    ) -> Result<SpaceRow, RegistryError> {
        let salt = random_kdf_salt();
        let created_at_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as i64;
        let cmd = RegistryCmd::CreateSpace {
            owner,
            space_id: space_id.to_owned(),
            root: root_entry_id,
            salt,
            created_at_ns,
        };
        let flag = self.submit(cmd)?;
        flag_to_error(flag)?;
        Ok(SpaceRow {
            root_entry_id,
            members: BTreeMap::from([(owner.to_hex(), Role::Owner)]),
            kdf_salt: salt,
            created_at_ns,
        })
    }

    /// 读空间行（线性一致确认后直读 `r-space`）。
    ///
    /// # Errors
    /// raft 错误。
    pub fn space(&self, space_id: &str) -> Result<Option<SpaceRow>, RegistryError> {
        self.handle.block_on(self.replica.ensure_linearizable())?;
        self.read_row(space_id)
    }

    /// 设置成员角色（actor 须为 Owner；SM 侧二次强制）。
    ///
    /// # Errors
    /// `Forbidden`/`Missing` 或 raft 错误。
    pub fn set_member(
        &self,
        actor: DeviceId,
        space_id: &str,
        target: DeviceId,
        role: Role,
    ) -> Result<(), RegistryError> {
        let flag = self.submit(RegistryCmd::SetMember {
            actor,
            space_id: space_id.to_owned(),
            target,
            role,
        })?;
        flag_to_error(flag)
    }

    /// 移除成员（actor 须为 Owner；最后一名 Owner 不可移）。
    ///
    /// # Errors
    /// `Forbidden`/`Missing` 或 raft 错误。
    pub fn remove_member(
        &self,
        actor: DeviceId,
        space_id: &str,
        target: DeviceId,
    ) -> Result<(), RegistryError> {
        let flag = self.submit(RegistryCmd::RemoveMember {
            actor,
            space_id: space_id.to_owned(),
            target,
        })?;
        flag_to_error(flag)
    }

    /// 角色检查（读侧纯判定；viewer 写/非成员/缺空间一律拒绝）。
    ///
    /// # Errors
    /// `Forbidden`/`Missing`。
    pub fn check(
        &self,
        space_id: &str,
        device: DeviceId,
        action: Action,
    ) -> Result<(), RegistryError> {
        let Some(row) = self.space(space_id)? else {
            return Err(RegistryError::Missing);
        };
        let Some(role) = row.members.get(&device.to_hex()) else {
            return Err(RegistryError::Forbidden);
        };
        let ok = match action {
            Action::Read => true,
            Action::Write => *role != Role::Viewer,
            Action::Admin => *role == Role::Owner,
        };
        if ok {
            Ok(())
        } else {
            Err(RegistryError::Forbidden)
        }
    }

    /// 停止注册表 raft core（演练/测试用）。
    pub fn crash(&self) {
        self.handle.block_on(self.replica.crash());
    }

    fn read_row(&self, space_id: &str) -> Result<Option<SpaceRow>, RegistryError> {
        // 读通道：直接经 SM 所在 keyspace（线性一致已由调用点确认）
        let db = self.replica.database();
        let spaces = db
            .keyspace(KS_SPACE, fjall::KeyspaceCreateOptions::default)
            .map_err(|e| RegistryError::Io(e.to_string()))?;
        Ok(spaces
            .get(space_id.as_bytes())
            .map_err(|e| RegistryError::Io(e.to_string()))?
            .and_then(|g| serde_json::from_slice(&g).ok()))
    }

    fn submit(&self, cmd: RegistryCmd) -> Result<u8, RegistryError> {
        let payload =
            serde_json::to_vec(&cmd).map_err(|e| RegistryError::Io(format!("cmd encode: {e}")))?;
        self.handle.block_on(async {
            let h = self.replica.submit(payload).await?;
            let (_, resp) = h.ack_with_response().await?;
            Ok(resp.first().copied().unwrap_or(FLAG_MISSING))
        })
    }
}
