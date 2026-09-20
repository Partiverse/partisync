//! 网络层（SPEC M3-WP02 T04，裁定 4）：tokio `TcpStream` 长连接 + 手写紧凑
//! 帧 + openraft 信封（serde_json）。
//!
//! 帧格式（版本字节为未来 iroh 替换留位）：
//! `[len u32 BE][version u8][payload]`，`len` 含 version 字节本身，
//! `payload` = serde_json([`NetRequest`]/[`NetResponse`])。Entry 载荷
//! （`HubData`）在 `AppendEntriesRequest` 内随信封走 JSON（裁定 4 允许：
//! 若 T07 实测吞吐受限，信封换紧凑编码属本地优化、协议不动）。
//!
//! 连接模型（长连接）：每个 [`TcpNetwork`] 实例对应一条到目标节点的 TCP
//! 连接，懒建立、串行复用（openraft 每实例 RPC 串行——`&mut self`）；
//! 传输错误即重置连接，重连由 openraft 重试驱动。单方向请求-应答，无
//! 多路复用；快照走 `install_snapshot` 分块帧（openraft 默认 `full_snapshot`
//! 切块逻辑）。
//!
//! 错误映射：`hard_ttl` 超时 → [`RPCError::Timeout`]；连接/IO 错误 →
//! [`RPCError::Network`]；远端业务错误（信封 `Err(RaftError)`）→
//! [`RPCError::RemoteError`]。

use std::future::Future;
use std::io;
use std::time::Duration;

use crate::raft_store::HubTypeConfig;
use openraft::error::{
    Infallible, InstallSnapshotError, NetworkError, RPCError, RaftError, RemoteError, Timeout,
};
use openraft::network::RPCOption;
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use openraft::{BasicNode, RPCTypes, RaftNetwork, RaftNetworkFactory};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// 当前帧协议版本（iroh 替换或其他传输演进时递增）。
pub const FRAME_VERSION: u8 = 0x01;

/// 单帧上限（64 MiB）——超限拒绝，防畸形长度击穿内存。
pub const MAX_FRAME_BYTES: u32 = 64 * 1024 * 1024;

/// 请求信封（裁定 4：openraft 消息 serde_json 序列化）。
#[derive(Debug, Serialize, Deserialize)]
pub enum NetRequest {
    /// 日志复制 / 心跳。
    AppendEntries(AppendEntriesRequest<HubTypeConfig>),
    /// 选举投票。
    Vote(VoteRequest<u64>),
    /// 快照分块安装。
    InstallSnapshot(InstallSnapshotRequest<HubTypeConfig>),
}

/// 应答信封：远端业务错误以 `Err(RaftError)` 过网（传输层错误由客户端
/// 本地构造，不过网）。
#[derive(Debug, Serialize, Deserialize)]
pub enum NetResponse {
    /// [`NetRequest::AppendEntries`] 的应答。
    AppendEntries(Result<AppendEntriesResponse<u64>, RaftError<u64>>),
    /// [`NetRequest::Vote`] 的应答。
    Vote(Result<VoteResponse<u64>, RaftError<u64>>),
    /// [`NetRequest::InstallSnapshot`] 的应答。
    InstallSnapshot(Result<InstallSnapshotResponse<u64>, RaftError<u64, InstallSnapshotError>>),
}

/// 写一帧：`[len u32 BE][version u8][payload]`。
///
/// # Errors
/// IO 失败或载荷超限。
pub async fn write_frame(stream: &mut TcpStream, payload: &[u8]) -> io::Result<()> {
    let len = u32::try_from(payload.len() + 1)
        .map_err(|_| io::Error::other("frame payload exceeds u32"))?;
    if len > MAX_FRAME_BYTES {
        return Err(io::Error::other("frame too large"));
    }
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(&[FRAME_VERSION]).await?;
    stream.write_all(payload).await?;
    stream.flush().await
}

/// 读一帧并校验版本字节（返回剥去版本头的 payload）。
///
/// # Errors
/// IO 失败、对端关闭、版本不符或长度超限。
pub async fn read_frame(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf);
    if len == 0 {
        return Err(io::Error::other("empty frame"));
    }
    if len > MAX_FRAME_BYTES {
        return Err(io::Error::other("frame too large"));
    }
    let mut version_buf = [0u8; 1];
    stream.read_exact(&mut version_buf).await?;
    if version_buf[0] != FRAME_VERSION {
        return Err(io::Error::other(format!(
            "frame version mismatch: {} != {FRAME_VERSION}",
            version_buf[0]
        )));
    }
    let mut payload = vec![0u8; len as usize - 1];
    stream.read_exact(&mut payload).await?;
    Ok(payload)
}

/// 框架服务端：accept 循环 + 每连接一任务，逐帧分发到 `handler`。
///
/// `handler` 由 replica 层（T05）提供——把 [`NetRequest`] 路由到本节点
/// `Raft` API；本模块只管帧与信封。
///
/// # Errors
/// accept 失败（连接级错误不计——单连接任务自行消化）。
pub async fn serve<F, Fut>(listener: TcpListener, handler: F) -> io::Result<()>
where
    F: Fn(NetRequest) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = NetResponse> + Send,
{
    loop {
        let (stream, _peer) = listener.accept().await?;
        let h = handler.clone();
        tokio::spawn(async move {
            let _ = handle_conn(stream, h).await;
        });
    }
}

/// 单连接服务循环：读帧 → 分发 → 写应答；任何错误即断开（客户端重置重连）。
async fn handle_conn<F, Fut>(mut stream: TcpStream, handler: F) -> io::Result<()>
where
    F: Fn(NetRequest) -> Fut,
    Fut: Future<Output = NetResponse>,
{
    loop {
        let payload = read_frame(&mut stream).await?;
        let req: NetRequest = serde_json::from_slice(&payload)
            .map_err(|e| io::Error::other(format!("request decode: {e}")))?;
        let resp = handler(req).await;
        let body = serde_json::to_vec(&resp)
            .map_err(|e| io::Error::other(format!("response encode: {e}")))?;
        write_frame(&mut stream, &body).await?;
    }
}

/// 客户端工厂：每个目标节点一个 [`TcpNetwork`]（地址取自 membership 的
/// `BasicNode.addr`）。
#[derive(Debug, Clone)]
pub struct NetFactory {
    self_id: u64,
}

impl NetFactory {
    /// 构造工厂（`self_id` 用于超时错误上报）。
    #[must_use]
    pub fn new(self_id: u64) -> Self {
        Self { self_id }
    }
}

impl RaftNetworkFactory<HubTypeConfig> for NetFactory {
    type Network = TcpNetwork;

    async fn new_client(&mut self, target: u64, node: &BasicNode) -> Self::Network {
        TcpNetwork {
            self_id: self.self_id,
            target,
            addr: node.addr.clone(),
            conn: None,
        }
    }
}

/// 长连接 RPC 客户端（懒连接、串行复用、错误重置）。
#[derive(Debug)]
pub struct TcpNetwork {
    self_id: u64,
    target: u64,
    addr: String,
    conn: Option<TcpStream>,
}

/// 信封变体不匹配（协议不变量破坏）→ Network 错误。
fn variant_mismatch() -> NetworkError {
    NetworkError::new(&io::Error::other("response variant mismatch"))
}

impl TcpNetwork {
    /// 取（必要时建立）长连接。
    async fn conn(&mut self) -> io::Result<&mut TcpStream> {
        if self.conn.is_none() {
            self.conn = Some(TcpStream::connect(&self.addr).await?);
        }
        Ok(self.conn.as_mut().expect("connection established above"))
    }

    /// 单次请求-应答（含 hard_ttl 超时与错误重置）。
    async fn round_trip<E>(
        &mut self,
        req: NetRequest,
        option: &RPCOption,
        action: RPCTypes,
    ) -> Result<NetResponse, RPCError<u64, BasicNode, RaftError<u64, E>>>
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        let hard_ttl: Duration = option.hard_ttl();
        let fut = self.do_round_trip(req);
        match tokio::time::timeout(hard_ttl, fut).await {
            Ok(Ok(resp)) => Ok(resp),
            Ok(Err(e)) => {
                self.conn = None;
                Err(RPCError::Network(NetworkError::new(&e)))
            }
            Err(_elapsed) => {
                self.conn = None;
                Err(RPCError::Timeout(Timeout {
                    action,
                    id: self.self_id,
                    target: self.target,
                    timeout: hard_ttl,
                }))
            }
        }
    }

    async fn do_round_trip(&mut self, req: NetRequest) -> io::Result<NetResponse> {
        let stream = self.conn().await?;
        let body = serde_json::to_vec(&req)
            .map_err(|e| io::Error::other(format!("request encode: {e}")))?;
        write_frame(stream, &body).await?;
        let payload = read_frame(stream).await?;
        serde_json::from_slice(&payload)
            .map_err(|e| io::Error::other(format!("response decode: {e}")))
    }
}

impl RaftNetwork<HubTypeConfig> for TcpNetwork {
    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<HubTypeConfig>,
        option: RPCOption,
    ) -> Result<AppendEntriesResponse<u64>, RPCError<u64, BasicNode, RaftError<u64>>> {
        let target = self.target;
        let resp = self
            .round_trip::<Infallible>(
                NetRequest::AppendEntries(rpc),
                &option,
                RPCTypes::AppendEntries,
            )
            .await?;
        match resp {
            NetResponse::AppendEntries(r) => {
                r.map_err(|e| RPCError::RemoteError(RemoteError::new(target, e)))
            }
            _ => Err(RPCError::Network(variant_mismatch())),
        }
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<u64>,
        option: RPCOption,
    ) -> Result<VoteResponse<u64>, RPCError<u64, BasicNode, RaftError<u64>>> {
        let target = self.target;
        let resp = self
            .round_trip::<Infallible>(NetRequest::Vote(rpc), &option, RPCTypes::Vote)
            .await?;
        match resp {
            NetResponse::Vote(r) => {
                r.map_err(|e| RPCError::RemoteError(RemoteError::new(target, e)))
            }
            _ => Err(RPCError::Network(variant_mismatch())),
        }
    }

    async fn install_snapshot(
        &mut self,
        rpc: InstallSnapshotRequest<HubTypeConfig>,
        option: RPCOption,
    ) -> Result<
        InstallSnapshotResponse<u64>,
        RPCError<u64, BasicNode, RaftError<u64, InstallSnapshotError>>,
    > {
        let target = self.target;
        let resp = self
            .round_trip(
                NetRequest::InstallSnapshot(rpc),
                &option,
                RPCTypes::InstallSnapshot,
            )
            .await?;
        match resp {
            NetResponse::InstallSnapshot(r) => {
                r.map_err(|e| RPCError::RemoteError(RemoteError::new(target, e)))
            }
            _ => Err(RPCError::Network(variant_mismatch())),
        }
    }
}

/// 测试与演示复用：从 `addr` 建一条一次性 RPC（不经长连接缓存）。
///
/// # Errors
/// 连接、超时或远端错误。
pub async fn one_shot_vote(
    addr: &str,
    req: VoteRequest<u64>,
    timeout: Duration,
) -> Result<VoteResponse<u64>, RPCError<u64, BasicNode, RaftError<u64>>> {
    let mut net = TcpNetwork {
        self_id: 0,
        target: 0,
        addr: addr.to_owned(),
        conn: None,
    };
    net.vote(req, RPCOption::new(timeout)).await
}
