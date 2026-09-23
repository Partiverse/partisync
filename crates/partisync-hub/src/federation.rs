//! Hub ↔ Hub 联邦路由协议（SPEC M5-WP02 裁定 3-6）：紧凑帧 + serde_json
//! [`FedRequest`]/[`FedResponse`] 信封。
//!
//! 帧层直接复用 [`crate::net`] 的 `[len u32 BE][version u8][payload]` 原语
//! （零新依赖；iroh 替换留位 = version 字节，M3-WP02 裁定 4 同款）。信封与
//! openraft `NetRequest` 族互不相交——同一连接只跑一种协议，变体不匹配 =
//! 连接重置（客户端重连自愈，[`TcpNetwork`](crate::net::TcpNetwork) 同口径）。
//!
//! 连接模型：[`FedClient`] 每目标一条懒建长连接、串行复用、错误重置；
//! 单次往返带 [`DEFAULT_FED_TIMEOUT`] 硬超时。协议语义（合并/协商）归
//! [`FedHandle`]（T04/T05）；本模块只管线格式与传输。
//!
//! 安全姿态：联邦口 v0.1 无认证，与 raft RPC 口一致（局域网/专线假设，
//! SPEC 裁定 3）；mTLS/iroh 传输归后续卡。

use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;

use crate::net::{read_frame, write_frame};
use crate::registry::{merge_route, RouteRow};

/// 单次联邦往返硬超时（客户端侧；loopback 实测 P99 ≪ 1ms，SPEC T07 登记）。
pub const DEFAULT_FED_TIMEOUT: Duration = Duration::from_secs(5);

/// 联邦协议错误集（传输错误不重试——重连/重试策略归调用方）。
#[derive(Debug)]
pub enum FedError {
    /// 连接/帧 IO 错误（对端不可达、重置、超时取消）。
    Io(std::io::Error),
    /// 信封解码失败或应答变体不匹配（协议不变量破坏）。
    Protocol(String),
    /// 落盘/读视图的 raft 错误（服务端语义层）。
    Registry(crate::registry::RegistryError),
}

impl fmt::Display for FedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "federation io: {e}"),
            Self::Protocol(e) => write!(f, "federation protocol: {e}"),
            Self::Registry(e) => write!(f, "federation registry: {e}"),
        }
    }
}

impl std::error::Error for FedError {}

impl From<std::io::Error> for FedError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<crate::registry::RegistryError> for FedError {
    fn from(e: crate::registry::RegistryError) -> Self {
        Self::Registry(e)
    }
}

/// 联邦请求信封（hub → hub；SPEC 契约 2）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FedRequest {
    /// 握手（反熵发起方自报家门；应答携带应答方全量路由视图）。
    Hello {
        /// 发起方 hub id。
        hub_id: u64,
        /// 发起方联邦口对外地址。
        addr: String,
    },
    /// 路由查询（只答本地视图，不代查——防环，裁定 9）。
    RouteQuery {
        /// 空间 id。
        space_id: String,
    },
    /// 路由认领（定向单行合并，裁定 5）。
    RouteClaim {
        /// 提案行（epoch = 提案方本地该空间已知最大 epoch + 1）。
        row: RouteRow,
    },
}

/// 联邦应答信封（hub → hub；与 [`FedRequest`] 一一对应）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FedResponse {
    /// [`FedRequest::Hello`] 应答：应答方身份 + 全量路由视图。
    HelloAck {
        /// 应答方 hub id。
        hub_id: u64,
        /// 应答方联邦口对外地址。
        addr: String,
        /// 应答方本地全量视图（发起方按合并规则吸收）。
        routes: Vec<RouteRow>,
    },
    /// [`FedRequest::RouteQuery`] 应答：本地视图命中与否。
    RouteAnswer {
        /// 命中的路由行（本地视图无此空间 = None）。
        route: Option<RouteRow>,
    },
    /// [`FedRequest::RouteClaim`] 裁决：提案是否胜出 + 收件方视图的胜者
    /// （败者据 winner 即时改写，不等周期握手——裁定 5）。
    ClaimVerdict {
        /// 提案胜出并被接受。
        accepted: bool,
        /// 裁决后该空间的胜者行（接受时 = 提案行）。
        winner: Option<RouteRow>,
    },
}

/// 联邦长连接客户端（懒建、串行复用、错误重置——[`TcpNetwork`](crate::net::TcpNetwork)
/// 同构但非 openraft trait）。类型化 helper（[`Self::hello`] 等）在应答
/// 变体不符时报 [`FedError::Protocol`]；原始往返见 [`Self::call`]。
#[derive(Debug)]
pub struct FedClient {
    addr: String,
    conn: Option<TcpStream>,
}

impl FedClient {
    /// 构造客户端。
    #[must_use]
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            addr: addr.into(),
            conn: None,
        }
    }

    /// 对端地址。
    #[must_use]
    pub fn addr(&self) -> &str {
        &self.addr
    }

    /// 取（必要时建立）长连接。
    ///
    /// # Errors
    /// 连接失败。
    async fn conn(&mut self) -> std::io::Result<&mut TcpStream> {
        if self.conn.is_none() {
            self.conn = Some(TcpStream::connect(&self.addr).await?);
        }
        Ok(self.conn.as_mut().expect("connection established above"))
    }

    /// 单次请求-应答（[`DEFAULT_FED_TIMEOUT`] 硬超时；任何错误重置连接）。
    ///
    /// # Errors
    /// 超时、IO 失败或信封/变体解码失败。
    pub async fn call(&mut self, req: FedRequest) -> Result<FedResponse, FedError> {
        match tokio::time::timeout(DEFAULT_FED_TIMEOUT, self.do_call(req)).await {
            Ok(r) => r,
            Err(_elapsed) => {
                self.conn = None;
                Err(FedError::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "federation round trip timed out",
                )))
            }
        }
    }

    async fn do_call(&mut self, req: FedRequest) -> Result<FedResponse, FedError> {
        let body = serde_json::to_vec(&req)
            .map_err(|e| FedError::Protocol(format!("request encode: {e}")))?;
        let stream = self.conn().await?;
        write_frame(stream, &body).await?;
        let payload = read_frame(stream).await?;
        serde_json::from_slice(&payload)
            .map_err(|e| FedError::Protocol(format!("response decode: {e}")))
    }

    /// 应答变体守卫：不匹配即 [`FedError::Protocol`]（协议不变量破坏）。
    fn expect_variant(resp: FedResponse, want: &'static str) -> Result<FedResponse, FedError> {
        let ok = match &resp {
            FedResponse::HelloAck { .. } => want == "HelloAck",
            FedResponse::RouteAnswer { .. } => want == "RouteAnswer",
            FedResponse::ClaimVerdict { .. } => want == "ClaimVerdict",
        };
        if ok {
            Ok(resp)
        } else {
            Err(FedError::Protocol(format!(
                "response variant mismatch: want {want}"
            )))
        }
    }

    /// 握手（反熵发起方）：返回 `(对方 hub_id, 对方地址, 对方全量视图)`。
    ///
    /// # Errors
    /// 传输失败或变体不匹配。
    pub async fn hello(
        &mut self,
        self_hub_id: u64,
        self_addr: &str,
    ) -> Result<(u64, String, Vec<RouteRow>), FedError> {
        let resp = Self::expect_variant(
            self.call(FedRequest::Hello {
                hub_id: self_hub_id,
                addr: self_addr.to_owned(),
            })
            .await?,
            "HelloAck",
        )?;
        match resp {
            FedResponse::HelloAck {
                hub_id,
                addr,
                routes,
            } => Ok((hub_id, addr, routes)),
            _ => unreachable!("expect_variant guards"),
        }
    }

    /// 路由查询：本地视图命中与否（对端不代查——防环）。
    ///
    /// # Errors
    /// 传输失败或变体不匹配。
    pub async fn route_query(&mut self, space_id: &str) -> Result<Option<RouteRow>, FedError> {
        let resp = Self::expect_variant(
            self.call(FedRequest::RouteQuery {
                space_id: space_id.to_owned(),
            })
            .await?,
            "RouteAnswer",
        )?;
        match resp {
            FedResponse::RouteAnswer { route } => Ok(route),
            _ => unreachable!("expect_variant guards"),
        }
    }

    /// 路由认领：返回 `(是否接受, 裁决胜者)`——败者据胜者即时改写。
    ///
    /// # Errors
    /// 传输失败或变体不匹配。
    pub async fn route_claim(
        &mut self,
        row: RouteRow,
    ) -> Result<(bool, Option<RouteRow>), FedError> {
        let resp = Self::expect_variant(
            self.call(FedRequest::RouteClaim { row }).await?,
            "ClaimVerdict",
        )?;
        match resp {
            FedResponse::ClaimVerdict { accepted, winner } => Ok((accepted, winner)),
            _ => unreachable!("expect_variant guards"),
        }
    }
}

/// 读侧/写侧公用：吸收远端视图进本地 raft（T04 反熵语义内核；T03 先导
/// 纯函数化便于测试）。
///
/// 逐行以 [`crate::registry::merge_route`] 判定，胜出且与本地不同才落盘；
/// 返回改写行数（幂等重放 = 0）。
///
/// # Errors
/// 视图读取或 raft 落盘失败。
pub async fn absorb_routes(
    view: &crate::registry::FederationView,
    remote: &[RouteRow],
) -> Result<usize, FedError> {
    let local = view.rows_async().await?;
    let index: std::collections::HashMap<&str, &RouteRow> =
        local.iter().map(|r| (r.space_id.as_str(), r)).collect();
    let mut applied = 0usize;
    for row in remote {
        if let Some(winner) = merge_route(index.get(row.space_id.as_str()).copied(), row) {
            view.set_route_async(winner).await?;
            applied += 1;
        }
    }
    Ok(applied)
}
