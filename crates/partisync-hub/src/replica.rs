//! Raft 组生命周期（SPEC M3-WP02 T05，裁定 1/5）：bootstrap、静态成员配置、
//! 领导者转移、选举演练与 failover 演练。
//!
//! 拓扑（裁定 1）：每 range 分区一组 raft（`group_id` = 分区 pid），成员
//! 静态配置（`Members` = `{node_id → addr}`），v0.1 无动态成员。组注册表
//! （`shard_id → 组`）的持久化随分裂×复制耦合（裁定 6）在
//! WP03+ 接线时收口。
//!
//! 写路径（裁定 5）：leader 串行 `client_write`（append + quorum commit +
//! apply 后应答）。读路径：leader 侧线性一致读 =
//! [`Raft::ensure_linearizable`]（ReadIndex 语义）+ 本机状态机读；
//! follower 侧经 `wait_applied` 顺序化后读本机数据节（0.9.25 的
//! `ensure_linearizable` 仅 leader 可用——follower 读语义归 T06 钉子文档）。
//!
//! 领导者转移演练：openraft 0.9.25 无显式 transfer API（铁律 8 核实），
//! 演练形态 = 当前 leader `pause_election` + 目标节点 `trigger_elect`
//! （更高 term 迫使旧 leader 让位）；<5s KPI 由测试计 WallClock。
//!
//! failover 演练：leader 进程死亡形态 = `crash()`（raft core shutdown，
//! 连接断、无优雅交接）；新 leader 当选 + ACK 写入可读 <10s（关门 KPI）。
//! ACK 写入不丢由 raft 日志 fdatasync（T03 durability）独立保证。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use fjall::Database;
use openraft::error::{CheckIsLeaderError, ClientWriteError, Fatal};
use openraft::storage::RaftStateMachine;
use openraft::ConfigError;
use openraft::{BasicNode, Config, Raft, ServerState, SnapshotPolicy};
use tokio::net::TcpListener;

use crate::net::{serve, NetFactory, NetRequest, NetResponse};
use crate::raft_store::{open_raft_stores, HubData, HubTypeConfig, RaftStateMachineStore};
use openraft::StorageError;

/// 静态成员表：`{node_id → 监听地址}`（裁定 1）。
pub type Members = BTreeMap<u64, String>;

/// 节点配置（构造期注入，SPEC §2 契约）。
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// 本节点 id。
    pub node_id: u64,
    /// 本节点 RPC 监听地址（如 `127.0.0.1:9001`）。
    pub addr: String,
    /// 本节点 DB 根目录。
    pub db_root: std::path::PathBuf,
    /// raft 组 id（= 分区 pid）。
    pub group_id: u64,
    /// 静态成员表（含本节点）。
    pub members: Members,
    /// 选举超时区间 ms（min, max）——演练/测试注入小值。
    pub election_timeout_ms: (u64, u64),
    /// 心跳间隔 ms。
    pub heartbeat_interval_ms: u64,
    /// 禁用自动快照（业务状态机模式必开——快照/安装当前仅覆盖通用节，
    /// 业务节快照化归后续任务卡；T02 起门面模式恒为 true）。
    pub disable_auto_snapshot: bool,
}

/// 组生命周期错误集。
#[derive(Debug)]
pub enum ReplicaError {
    /// 监听/socket 错误。
    Io(std::io::Error),
    /// 存储引擎错误。
    Fjall(fjall::Error),
    /// raft 运行时配置错误。
    Config(ConfigError),
    /// raft core 致命错误（含 shutdown）。
    Fatal(Fatal<u64>),
    /// bootstrap（initialize）失败。
    Init(String),
    /// 客户端写失败。
    ClientWrite(ClientWriteError<u64, BasicNode>),
    /// 线性一致读的就绪确认失败。
    CheckIsLeader(CheckIsLeaderError<u64, BasicNode>),
    /// 状态机读取失败。
    Storage(StorageError<u64>),
    /// 演练等待超时。
    Timeout,
}

impl std::fmt::Display for ReplicaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "replica io: {e}"),
            Self::Fjall(e) => write!(f, "replica storage: {e}"),
            Self::Config(e) => write!(f, "replica config: {e}"),
            Self::Fatal(e) => write!(f, "replica fatal: {e}"),
            Self::Init(e) => write!(f, "replica bootstrap: {e}"),
            Self::ClientWrite(e) => write!(f, "replica client write: {e}"),
            Self::CheckIsLeader(e) => write!(f, "replica read: {e}"),
            Self::Storage(e) => write!(f, "replica state machine: {e}"),
            Self::Timeout => write!(f, "replica wait timeout"),
        }
    }
}

impl std::error::Error for ReplicaError {}

impl From<std::io::Error> for ReplicaError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<fjall::Error> for ReplicaError {
    fn from(e: fjall::Error) -> Self {
        Self::Fjall(e)
    }
}
impl From<ConfigError> for ReplicaError {
    fn from(e: ConfigError) -> Self {
        Self::Config(e)
    }
}
impl From<Fatal<u64>> for ReplicaError {
    fn from(e: Fatal<u64>) -> Self {
        Self::Fatal(e)
    }
}
impl From<StorageError<u64>> for ReplicaError {
    fn from(e: StorageError<u64>) -> Self {
        Self::Storage(e)
    }
}

/// 已提交写句柄：[`Replica::submit`] 的应答，await 即等待该条目
/// commit 并 apply（返回 log index）——流水线导入（有界在途窗口）用。
pub struct WriteHandle {
    rx: tokio::sync::oneshot::Receiver<openraft::raft::ClientWriteResult<HubTypeConfig>>,
}

impl WriteHandle {
    /// 等待提交完成。
    ///
    /// # Errors
    /// raft core 停止或写被拒绝。
    pub async fn ack(self) -> Result<u64, ReplicaError> {
        self.ack_with_response().await.map(|(idx, _)| idx)
    }

    /// 等待提交完成并取回状态机应答（业务节以应答字节承载语义标志）。
    ///
    /// # Errors
    /// raft core 停止或写被拒绝。
    pub async fn ack_with_response(self) -> Result<(u64, Vec<u8>), ReplicaError> {
        let res = self
            .rx
            .await
            .map_err(|e| ReplicaError::Init(format!("raft core stopped: {e}")))?;
        match res {
            Ok(resp) => Ok((resp.log_id.index, resp.data.0)),
            Err(e) => Err(ReplicaError::ClientWrite(e)),
        }
    }
}

/// 一个 raft 组成员节点（v0.1：一节点一 DB、承载一组）。
pub struct Replica {
    node_id: u64,
    group_id: u64,
    members: Members,
    raft: Raft<HubTypeConfig>,
    sm_reader: RaftStateMachineStore,
    _db: Database,
}

impl Replica {
    /// 打开节点：绑定 RPC 监听 + 打开组存储 + 启动 raft core。
    ///
    /// 打开后处于未初始化态——需调用 [`Self::bootstrap`] 形成集群。
    ///
    /// # Errors
    /// 监听绑定、存储打开、配置构建或 raft core 启动失败。
    pub async fn open(cfg: &NodeConfig) -> Result<Self, ReplicaError> {
        let db = Database::open(fjall::Config::new(&cfg.db_root))?;
        let (_, sm) = open_raft_stores(&db, cfg.group_id)?;
        Self::open_with_sm(cfg, db, sm).await
    }

    /// 以既有 Database 与外部状态机打开（业务状态机模式——
    /// [`crate::service::HubService`]：SM = 通用节 + 业务节包装）。
    ///
    /// # Errors
    /// 同 [`Self::open`]。
    pub async fn open_with_sm(
        cfg: &NodeConfig,
        db: Database,
        sm: impl RaftStateMachine<HubTypeConfig>,
    ) -> Result<Self, ReplicaError> {
        let listener = TcpListener::bind(&cfg.addr).await?;
        let (log, _generic) = open_raft_stores(&db, cfg.group_id)?;
        drop(_generic); // 业务模式下通用 SM 由调用方包装传入

        let config = Arc::new(Config {
            cluster_name: "partisync-hub".to_owned(),
            heartbeat_interval: cfg.heartbeat_interval_ms,
            election_timeout_min: cfg.election_timeout_ms.0,
            election_timeout_max: cfg.election_timeout_ms.1,
            snapshot_policy: if cfg.disable_auto_snapshot {
                SnapshotPolicy::Never
            } else {
                Config::default().snapshot_policy
            },
            ..Config::default()
        });

        let raft = Raft::new(cfg.node_id, config, NetFactory::new(cfg.node_id), log, sm).await?;

        // RPC 分发：帧信封 → 本节点 Raft API（T04 serve 框架）
        let raft_serve = raft.clone();
        let handler = move |req: NetRequest| {
            let raft = raft_serve.clone();
            async move {
                match req {
                    NetRequest::AppendEntries(r) => {
                        NetResponse::AppendEntries(raft.append_entries(r).await)
                    }
                    NetRequest::Vote(r) => NetResponse::Vote(raft.vote(r).await),
                    NetRequest::InstallSnapshot(r) => {
                        NetResponse::InstallSnapshot(raft.install_snapshot(r).await)
                    }
                }
            }
        };
        tokio::spawn(serve(listener, handler));

        let sm_reader = {
            let (_, sm2) = open_raft_stores(&db, cfg.group_id)?;
            sm2
        };
        Ok(Self {
            node_id: cfg.node_id,
            group_id: cfg.group_id,
            members: cfg.members.clone(),
            raft,
            sm_reader,
            _db: db,
        })
    }

    /// 节点 id。
    #[must_use]
    pub fn node_id(&self) -> u64 {
        self.node_id
    }

    /// 节点 Database 句柄（注册表等业务节复用同一库）。
    #[must_use]
    pub fn database(&self) -> &Database {
        &self._db
    }

    /// 组 id（= 分区 pid）。
    #[must_use]
    pub fn group_id(&self) -> u64 {
        self.group_id
    }

    /// 静态成员表。
    #[must_use]
    pub fn members(&self) -> &Members {
        &self.members
    }

    /// bootstrap：以静态成员表初始化集群（幂等——已初始化节点安全忽略）。
    ///
    /// # Errors
    /// 非 NotAllowed 的 initialize 失败。
    pub async fn bootstrap(&self) -> Result<(), ReplicaError> {
        let nodes: BTreeMap<u64, BasicNode> = self
            .members
            .iter()
            .map(|(id, addr)| (*id, BasicNode::new(addr)))
            .collect();
        match self.raft.initialize(nodes).await {
            Ok(()) => Ok(()),
            // 幂等语义：并发 bootstrap 下先胜者的 membership 已复制到本
            // 节点——「已初始化」按 openraft 文档安全忽略
            Err(openraft::error::RaftError::APIError(
                openraft::error::InitializeError::NotAllowed(_),
            )) => Ok(()),
            Err(openraft::error::RaftError::Fatal(f)) => Err(ReplicaError::Fatal(f)),
            Err(other) => Err(ReplicaError::Init(other.to_string())),
        }
    }

    /// 等待本组出现任一 leader（返回其 id）。
    ///
    /// # Errors
    /// 超时。
    pub async fn wait_leader(&self, timeout: Duration) -> Result<u64, ReplicaError> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(l) = self.raft.metrics().borrow().current_leader {
                return Ok(l);
            }
            if Instant::now() >= deadline {
                return Err(ReplicaError::Timeout);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// 当前已知 leader（不等待）。
    #[must_use]
    pub fn current_leader(&self) -> Option<u64> {
        self.raft.metrics().borrow().current_leader
    }

    /// 本节点是否为 leader。
    #[must_use]
    pub fn is_leader(&self) -> bool {
        self.raft.metrics().borrow().state == ServerState::Leader
    }

    /// 等待本节点状态机应用到 `index`（follower 顺序读的就绪条件）。
    ///
    /// # Errors
    /// 超时。
    pub async fn wait_applied(&self, index: u64, timeout: Duration) -> Result<(), ReplicaError> {
        self.raft
            .wait(Some(timeout))
            .applied_index_at_least(Some(index), "wait applied")
            .await
            .map_err(|_| ReplicaError::Timeout)?;
        Ok(())
    }

    /// 客户端写（leader 串行；返回提交并应用后的 log index）。
    ///
    /// # Errors
    /// 非 leader（`ForwardToLeader` 指路）或 raft 错误。
    pub async fn write(&self, payload: Vec<u8>) -> Result<u64, ReplicaError> {
        let resp = self
            .raft
            .client_write(HubData(payload))
            .await
            .map_err(|e| match e {
                openraft::error::RaftError::APIError(ce) => ReplicaError::ClientWrite(ce),
                other => ReplicaError::Fatal(other.into_fatal().unwrap_or(Fatal::Stopped)),
            })?;
        Ok(resp.log_id.index)
    }

    /// leader 线性一致读（裁定 5：ReadIndex 语义）——确认领导权后读本机
    /// 状态机数据节。
    ///
    /// # Errors
    /// 本节点非 leader 或引擎读取失败。
    /// 线性一致读就绪确认（裁定 5：ReadIndex 语义）——leader 向 quorum
    /// 确认领导权并等待本机 apply 追平读点。业务门面读侧确认后即可读本机
    /// 状态机（Replica 数据节或业务平面）。
    ///
    /// # Errors
    /// 本节点非 leader（`CheckIsLeader`）或 raft 错误。
    pub async fn ensure_linearizable(&self) -> Result<(), ReplicaError> {
        self.raft
            .ensure_linearizable()
            .await
            .map(|_| ())
            .map_err(|e| match e {
                openraft::error::RaftError::APIError(ce) => ReplicaError::CheckIsLeader(ce),
                other => ReplicaError::Fatal(other.into_fatal().unwrap_or(Fatal::Stopped)),
            })
    }

    /// leader 线性一致读（裁定 5）：ReadIndex 就绪确认 + 本机数据节读。
    ///
    /// # Errors
    /// 本节点非 leader 或引擎读取失败。
    pub async fn linearizable_read(&self, index: u64) -> Result<Option<Vec<u8>>, ReplicaError> {
        self.ensure_linearizable().await?;
        self.sm_reader
            .read_data_row(index)
            .await
            .map_err(Into::into)
    }

    /// follower 顺序读：已由 [`Self::wait_applied`] 确认应用进度后读本机
    /// 数据节（0.9.25 `ensure_linearizable` 仅 leader 可用，见模块文档）。
    ///
    /// # Errors
    /// 引擎读取失败。
    pub async fn read_applied(&self, index: u64) -> Result<Option<Vec<u8>>, ReplicaError> {
        self.sm_reader
            .read_data_row(index)
            .await
            .map_err(Into::into)
    }

    /// 提交写（fire-and-forget）：立即返回 [`WriteHandle`]，由调用方
    /// 决定在途窗口并逐个 `ack` 等待——openraft 对在途写做日志合批刷盘，
    /// 吞吐远高于逐条 `write` 的串行 fsync 形态。
    ///
    /// # Errors
    /// raft core 停止。
    pub async fn submit(&self, payload: Vec<u8>) -> Result<WriteHandle, ReplicaError> {
        let rx = self.raft.client_write_ff(HubData(payload)).await?;
        Ok(WriteHandle { rx })
    }

    /// 领导者转移演练第一步：暂停本节点选举（openraft 0.9 无显式
    /// transfer API，见模块文档）。
    pub fn pause_election(&self) {
        self.raft.runtime_config().elect(false);
    }

    /// 恢复本节点选举。
    pub fn resume_election(&self) {
        self.raft.runtime_config().elect(true);
    }

    /// 立即触发本节点选举（转移演练第二步 / 选举演练用）。
    ///
    /// # Errors
    /// raft core 致命错误。
    pub async fn trigger_elect(&self) -> Result<(), ReplicaError> {
        self.raft.trigger().elect().await?;
        Ok(())
    }

    /// 进程死亡形态（failover 演练）：停掉 raft core（无优雅交接，
    /// 连接断开）。ACK 写入不丢由日志 fdatasync 独立保证（T03）。
    pub async fn crash(&self) {
        let _ = self.raft.shutdown().await;
    }
}
