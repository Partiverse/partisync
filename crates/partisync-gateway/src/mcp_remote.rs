//! Gateway 远程 MCP 传输面（M10-WP05-T03，SPEC docs/specs/M10-WP05.md §2.2，
//! ADR-0031 决策 2/4）：Streamable HTTP + Origin/host 校验（先于授权面）+
//! fail-closed 默认态（墙先于门）+ rustls TLS 线位与非回环启动守卫。端点沿
//! M9 骨架口径（`POST /mcp` + PRM）；远程入口 bin = `partisync-mcp-http`。
//!
//! 分派序（[`gate_decision`] 纯函数，探针钉死）：PRM 公开无墙；其余请求过
//! gate——Host 白名单（rmcp 语义镜像）→ Origin 白名单（名单空 = 关）→
//! 授权墙 [`AuthWall`]（T03 一切请求 401，含任意 Bearer——无数据通路，
//! T04 开门前面不可达）→（T04 起）rmcp 服务。运行时面：[`bind_server`]
//! （启动守卫 + TLS 装配 + bind）→ [`RemoteMcpServer::serve`]。探针见
//! `tests/wp05_mcp_remote.rs`；非回环前置条件见 [`check_startup`]。

use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::uri::Authority;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
use axum::Router;
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::tower::{
    StreamableHttpServerConfig, StreamableHttpService,
};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use serde_json::json;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;

use crate::mcp::McpServerState;

/// PRM well-known 根路径（RFC 9728；hub 骨架同形，M9-WP05 判例）。
pub const PRM_WELLKNOWN_PATH: &str = "/.well-known/oauth-protected-resource";

/// MCP endpoint 路径（2026-07-28 单端点，spec 例 `…/mcp`）。
pub const MCP_ENDPOINT_PATH: &str = "/mcp";

/// 入口 bin 缺省端口（评估件 §5：实施期定；避让 hub 演示面 8090/8091）。
pub const DEFAULT_BIND_PORT: u16 = 8424;

/// PRM 文档配置（ADR-0031 决策 1：四字段值全部配置入参；缺省 `.invalid`
/// 不可解析 mock 域——不冒充真实授权服务器，公网部署必须覆写）。
#[derive(Debug, Clone)]
pub struct PrmConfig {
    /// RFC 9728 `resource`（canonical URI）。
    pub resource: String,
    /// RFC 9728 `authorization_servers`（MCP MUST ≥1）。
    pub authorization_servers: Vec<String>,
    /// RFC 9728 `scopes_supported`（缺省 = ADR-0031 决策 5 扁平三域）。
    pub scopes_supported: Vec<String>,
}

impl Default for PrmConfig {
    fn default() -> Self {
        Self {
            resource: "https://mcp.partisync.invalid".to_string(),
            authorization_servers: vec!["https://auth.partisync.invalid".to_string()],
            scopes_supported: ["mcp:read", "mcp:write", "mcp:export"]
                .map(str::to_string)
                .to_vec(),
        }
    }
}

impl PrmConfig {
    /// RFC 9728 四字段 PRM 文档（`bearer_methods_supported` 固定 `["header"]`）。
    pub fn protected_resource_metadata(&self) -> serde_json::Value {
        json!({
            "resource": self.resource,
            "authorization_servers": self.authorization_servers,
            "scopes_supported": self.scopes_supported,
            "bearer_methods_supported": ["header"],
        })
    }

    /// 挑战 `resource_metadata` 值：绝对 URL resource → 完整 well-known
    /// URL；否则本进程路径（hub 骨架同形）。
    pub fn metadata_url(&self) -> String {
        let lower = self.resource.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") {
            format!(
                "{}{PRM_WELLKNOWN_PATH}",
                self.resource.trim_end_matches('/')
            )
        } else {
            PRM_WELLKNOWN_PATH.to_string()
        }
    }
}

/// TLS 证书文件对（PEM）；由 TLS 线位在 bind 期（`bind_server`）加载。
#[derive(Debug, Clone)]
pub struct TlsFiles {
    /// 证书链 PEM（leaf 在前）。
    pub cert_path: PathBuf,
    /// 私钥 PEM（PKCS8/PKCS1/SEC1 任一，`PemObject` 自动识别）。
    pub key_path: PathBuf,
}

/// 远程 MCP 服务配置（bin CLI → 本结构的映射见 `partisync-mcp-http`）。
#[derive(Debug, Clone)]
pub struct McpRemoteConfig {
    /// 监听地址；缺省 127.0.0.1:[`DEFAULT_BIND_PORT`]。
    pub bind_addr: SocketAddr,
    /// TLS 证书对；缺省 None（回环可明文，非回环被 [`check_startup`] 拒）。
    pub tls: Option<TlsFiles>,
    /// Host 白名单覆写；None = rmcp 回环默认名单（[`default_allowed_hosts`]）。
    pub allowed_hosts: Option<Vec<String>>,
    /// Origin 白名单；空 = Origin 校验关（rmcp 默认语义，公网部署须显式配）。
    pub allowed_origins: Vec<String>,
    /// PRM 四字段配置。
    pub prm: PrmConfig,
}

impl Default for McpRemoteConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], DEFAULT_BIND_PORT)),
            tls: None,
            allowed_hosts: None,
            allowed_origins: Vec::new(),
            prm: PrmConfig::default(),
        }
    }
}

impl McpRemoteConfig {
    /// Host 白名单生效值：未覆写 = rmcp 回环默认（与其 `Default` 同名单）；
    /// gate 与 rmcp 内层共用同一名单，无双源漂移。
    pub fn effective_allowed_hosts(&self) -> Vec<String> {
        self.allowed_hosts.clone().unwrap_or_else(|| {
            vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "::1".to_string(),
            ]
        })
    }
}

/// gate 的 HTTP 层拒绝结果（`None` = 放行进下一层）。
#[derive(Debug, PartialEq, Eq)]
pub struct GateRejection {
    pub status: StatusCode,
    /// text/plain 响应体（401 挑战无体，hub 骨架同形）。
    pub message: &'static str,
    /// Some = `WWW-Authenticate` 头值（401 挑战）。
    pub www_authenticate: Option<String>,
}

impl GateRejection {
    pub fn into_response(self) -> Response {
        let mut resp = (self.status, self.message).into_response();
        if let Some(www) = self.www_authenticate {
            if let Ok(value) = HeaderValue::from_str(&www) {
                resp.headers_mut().insert(header::WWW_AUTHENTICATE, value);
            }
        }
        resp
    }
}

/// 启动守卫（SPEC §2.2：非回环绑定必须 TLS，不静默降级明文）。
///
/// 非回环前置条件（评估件 §5 + ADR-0031 决策 4）：①未配置 TLS → 拒绝
/// 启动；②`allowed_hosts` 未显式覆写（仍为回环默认名单，远程请求将被
/// host 校验 403 全拒）或显式**空名单**（host 校验整体关闭 = DNS
/// rebinding 防线失效，rmcp 空名单语义为放行）→ 拒绝启动。授权配置
/// T03 期由 fail-closed 墙兜底（一切请求 401），T04 起随授权面接线
/// 检查（SPEC §6-R9 窗口期设计）。
pub fn check_startup(
    bind_addr: SocketAddr,
    tls: Option<&TlsFiles>,
    allowed_hosts: Option<&[String]>,
) -> Result<(), String> {
    if bind_addr.ip().is_loopback() {
        return Ok(());
    }
    if tls.is_none() {
        return Err(format!(
            "启动守卫拒绝：bind {bind_addr} 为非回环地址且未配置 TLS（不静默降级明文）\
             ——配置 --tls-cert/--tls-key，或绑定回环 127.0.0.1"
        ));
    }
    match allowed_hosts {
        None => {
            return Err(format!(
                "启动守卫拒绝：bind {bind_addr} 为非回环地址且未显式配置 allowed_hosts\
                 （内建回环默认名单会 403 全拒远程请求）——用 --allow-host 显式配置真实主机名"
            ));
        }
        Some([]) => {
            return Err(format!(
                "启动守卫拒绝：bind {bind_addr} 为非回环地址且 allowed_hosts 为空名单\
                 （host 校验整体关闭，DNS rebinding 防线失效）——用 --allow-host 显式配置真实主机名"
            ));
        }
        Some(_) => {}
    }
    Ok(())
}

/// 授权墙（fail-closed 默认态，SPEC §2.2「墙先于门」）。T03 期仅
/// `FailClosedAll` 一态；T04 以 token 校验矩阵替换 `decide` 语义（探针
/// 断言面 = 未授权必 401，届时演化不删除断言）。
#[derive(Debug, Clone)]
pub enum AuthWall {
    /// 全量 401：任何请求都未过授权校验（T03 默认态）。
    FailClosedAll {
        /// PRM 文档 URL（挑战 `resource_metadata` 指向，评估件 A-2 形状）。
        prm_metadata_url: String,
        /// 挑战 `scope` 参数（spec Scope Selection Strategy：挑战中优先给出）。
        scopes: String,
    },
}

impl AuthWall {
    /// 裁决：`authorization` 为原始 `Authorization` 头值。
    pub fn decide(&self, _authorization: Option<&HeaderValue>) -> Option<GateRejection> {
        match self {
            AuthWall::FailClosedAll {
                prm_metadata_url,
                scopes,
            } => Some(GateRejection {
                status: StatusCode::UNAUTHORIZED,
                message: "",
                www_authenticate: Some(format!(
                    "Bearer resource_metadata=\"{prm_metadata_url}\", scope=\"{scopes}\""
                )),
            }),
        }
    }
}

/// gate 总裁决：host → origin（先于授权面，SPEC §2.2）→ 授权墙。
pub fn gate_decision(
    allowed_hosts: &[String],
    allowed_origins: &[String],
    uri: &Uri,
    headers: &HeaderMap,
    wall: &AuthWall,
) -> Option<GateRejection> {
    if let Err(rejection) = host_check(uri, headers, allowed_hosts) {
        return Some(rejection);
    }
    if let Err(rejection) = origin_check(headers, allowed_origins) {
        return Some(rejection);
    }
    wall.decide(headers.get(header::AUTHORIZATION))
}

/// Host 校验（rmcp tower.rs `validate_dns_rebinding_headers` 语义镜像）：
/// 缺失/畸形 → 400；非白名单 → 403；空名单放行（rmcp 同）。
fn host_check(
    uri: &Uri,
    headers: &HeaderMap,
    allowed_hosts: &[String],
) -> Result<(), GateRejection> {
    if allowed_hosts.is_empty() {
        return Ok(());
    }
    let host: NormalizedAuthority = if let Some(host) = headers.get(header::HOST) {
        let host_str = host
            .to_str()
            .map_err(|_| bad_request("Invalid Host header encoding"))?;
        parse_allowed_authority(host_str).ok_or_else(|| bad_request("Invalid Host header"))?
    } else {
        let authority = uri
            .authority()
            .ok_or_else(|| bad_request("missing Host header"))?;
        NormalizedAuthority {
            host: normalize_host(authority.host()),
            port: authority.port_u16(),
        }
    };
    if !host_is_allowed(&host, allowed_hosts) {
        return Err(GateRejection {
            status: StatusCode::FORBIDDEN,
            message: "Forbidden: Host header is not allowed",
            www_authenticate: None,
        });
    }
    Ok(())
}

/// Origin 校验（rmcp tower.rs `validate_origin_header` 语义镜像）：名单空 =
/// 校验关；缺失放行（spec 只约束「存在且非法」）；非 UTF-8/畸形/非白名单 → 403。
fn origin_check(headers: &HeaderMap, allowed_origins: &[String]) -> Result<(), GateRejection> {
    if allowed_origins.is_empty() {
        return Ok(());
    }
    let Some(origin) = headers.get(header::ORIGIN) else {
        return Ok(());
    };
    let origin_str = origin
        .to_str()
        .map_err(|_| forbidden("Forbidden: Invalid Origin header encoding"))?;
    let parsed = parse_origin_value(origin_str)
        .ok_or_else(|| forbidden("Forbidden: Invalid Origin header"))?;
    if !origin_is_allowed(&parsed, allowed_origins) {
        return Err(forbidden("Forbidden: Origin header is not allowed"));
    }
    Ok(())
}

fn bad_request(message: &'static str) -> GateRejection {
    GateRejection {
        status: StatusCode::BAD_REQUEST,
        message,
        www_authenticate: None,
    }
}

fn forbidden(message: &'static str) -> GateRejection {
    GateRejection {
        status: StatusCode::FORBIDDEN,
        message,
        www_authenticate: None,
    }
}

// host/origin 解析与匹配——rmcp tower.rs :781-:913 五个解析/匹配函数的
// 语义镜像（大小写不敏感、IPv6 方括号剥离、host-only 条目匹配任意端口、
// Origin 按 RFC 6454 三元组等值、"null" origin 显式匹配）。

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedAuthority {
    host: String,
    port: Option<u16>,
}

fn normalize_host(host: &str) -> String {
    host.trim_matches('[')
        .trim_matches(']')
        .to_ascii_lowercase()
}

fn parse_allowed_authority(allowed: &str) -> Option<NormalizedAuthority> {
    let allowed = allowed.trim();
    if allowed.is_empty() {
        return None;
    }
    Some(
        Authority::try_from(allowed)
            .map(|a| NormalizedAuthority {
                host: normalize_host(a.host()),
                port: a.port_u16(),
            })
            .unwrap_or(NormalizedAuthority {
                host: normalize_host(allowed),
                port: None,
            }),
    )
}

fn host_is_allowed(host: &NormalizedAuthority, allowed_hosts: &[String]) -> bool {
    allowed_hosts
        .iter()
        .filter_map(|a| parse_allowed_authority(a))
        .any(|allowed| {
            allowed.host == host.host
                && match allowed.port {
                    Some(port) => host.port == Some(port),
                    None => true,
                }
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NormalizedOrigin {
    Null,
    Tuple {
        scheme: String,
        host: String,
        port: Option<u16>,
    },
}

fn parse_origin_value(value: &str) -> Option<NormalizedOrigin> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value.eq_ignore_ascii_case("null") {
        return Some(NormalizedOrigin::Null);
    }
    let uri = Uri::try_from(value).ok()?;
    let scheme = uri.scheme_str()?.to_ascii_lowercase();
    let authority = uri.authority()?;
    Some(NormalizedOrigin::Tuple {
        scheme,
        host: normalize_host(authority.host()),
        port: authority.port_u16(),
    })
}

fn origin_is_allowed(origin: &NormalizedOrigin, allowed_origins: &[String]) -> bool {
    allowed_origins
        .iter()
        .filter_map(|raw| parse_origin_value(raw))
        .any(|allowed| match (&allowed, origin) {
            (NormalizedOrigin::Null, NormalizedOrigin::Null) => true,
            (
                NormalizedOrigin::Tuple {
                    scheme: a_s,
                    host: a_h,
                    port: a_p,
                },
                NormalizedOrigin::Tuple {
                    scheme: o_s,
                    host: o_h,
                    port: o_p,
                },
            ) => a_s == o_s && a_h == o_h && (a_p.is_none() || a_p == o_p),
            _ => false,
        })
}

#[derive(Debug, Clone)]
struct GateState {
    allowed_hosts: Vec<String>,
    allowed_origins: Vec<String>,
    wall: AuthWall,
    prm: PrmConfig,
}

/// 组装远程 MCP router：PRM 公开路由 + gate 中间件包住的 rmcp 服务（`POST
/// /mcp` 单端点）。T03 期 gate 对一切非 PRM 请求 401（fail-closed 默认态）。
pub fn build_router(config: &McpRemoteConfig, mcp_state: Arc<McpServerState>) -> Router {
    let allowed_hosts = config.effective_allowed_hosts();
    let gate_state = Arc::new(GateState {
        allowed_hosts: allowed_hosts.clone(),
        allowed_origins: config.allowed_origins.clone(),
        wall: AuthWall::FailClosedAll {
            prm_metadata_url: config.prm.metadata_url(),
            scopes: config.prm.scopes_supported.join(" "),
        },
        prm: config.prm.clone(),
    });
    let rmcp_service = StreamableHttpService::new(
        {
            let state = Arc::clone(&mcp_state);
            move || -> std::io::Result<Arc<McpServerState>> { Ok(Arc::clone(&state)) }
        },
        Arc::new(NeverSessionManager::default()),
        // fail-closed 四件套（ADR-0031 决策 2）：stateless + 单 JSON +
        // 协议头强制（rmcp 默认 false，缺头放行是关键坑位）+ 同源白名单。
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true)
            .with_stateless_protocol_metadata_required(true)
            .with_allowed_hosts(allowed_hosts)
            .with_allowed_origins(config.allowed_origins.clone()),
    );
    let mcp_router: Router<()> = Router::new()
        .route_service(MCP_ENDPOINT_PATH, rmcp_service)
        .layer(middleware::from_fn_with_state(
            Arc::clone(&gate_state),
            gate_middleware,
        ));
    Router::new()
        .route(PRM_WELLKNOWN_PATH, get(prm_handler))
        .fallback_service(mcp_router)
        .with_state(gate_state)
}

async fn gate_middleware(
    State(gate): State<Arc<GateState>>,
    request: Request,
    next: Next,
) -> Response {
    match gate_decision(
        &gate.allowed_hosts,
        &gate.allowed_origins,
        request.uri(),
        request.headers(),
        &gate.wall,
    ) {
        Some(rejection) => rejection.into_response(),
        None => next.run(request).await,
    }
}

async fn prm_handler(State(gate): State<Arc<GateState>>) -> Json<serde_json::Value> {
    Json(gate.prm.protected_resource_metadata())
}

// ─────────────────────────────────────────────────────────────────────────────
// TLS 线位 + 运行时面（M10-WP05-T03 PR-3：ADR-0031 决策 4 + SPEC §2.2 入口）
// ─────────────────────────────────────────────────────────────────────────────

/// PEM 证书对 → rustls 服务端配置（`PemObject` 文件加载；证书链 leaf 在前，
/// 私钥 PKCS8/PKCS1/SEC1 任一自动识别）。任何失败显式报错，不 panic。
///
/// provider 显式指定 aws-lc-rs（ADR-0031 决策 4 线位）：workspace 统一
/// feature 后 rustls 可能同时启用 `aws-lc-rs` 与 `ring`（iroh 线拉入），
/// `ServerConfig::builder()` 在双 provider 下会 panic 拒绝猜测——测试全量
/// 并行实发（crypto/mod.rs:249），显式指定消除该不确定性。
fn load_tls_server_config(tls: &TlsFiles) -> Result<rustls::ServerConfig, String> {
    let certs: Vec<CertificateDer> = CertificateDer::pem_file_iter(&tls.cert_path)
        .map_err(|e| format!("TLS 证书 {} 读取失败: {e}", tls.cert_path.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("TLS 证书 {} 解析失败: {e}", tls.cert_path.display()))?;
    if certs.is_empty() {
        return Err(format!(
            "TLS 证书 {} 为空（需 PEM 证书链，leaf 在前）",
            tls.cert_path.display()
        ));
    }
    let key = PrivateKeyDer::from_pem_file(&tls.key_path)
        .map_err(|e| format!("TLS 私钥 {} 读取/解析失败: {e}", tls.key_path.display()))?;
    rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| format!("TLS provider 协议版本装配失败: {e}"))?
    .with_no_client_auth()
    .with_single_cert(certs, key)
    .map_err(|e| format!("TLS 证书/私钥装配失败（不匹配？）: {e}"))
}

/// TLS 监听器：TcpListener + TLS acceptor 的 [`axum::serve::Listener`] 实现
/// （axum 0.8 内建 Listener 仅明文 TCP；该 trait 未 sealed，允许外部落位）。
/// 单条连接的 TLS 握手在 accept 线位内串行完成——握手失败丢弃该连接继续
/// 服务。**已知债务（T03 评审登记）**：串行握手可被慢握手客户端放大
/// （slowloris 型）——SPEC §4 明列速率限制/反代为非目标且 T03 墙后无
/// 数据通路，当前不可利用；公网暴露前（T-R7 处置）须改握手 spawn 化
/// （每连接任务内完成握手）。
pub struct TlsListener {
    tcp: TcpListener,
    acceptor: TlsAcceptor,
}

impl axum::serve::Listener for TlsListener {
    type Io = tokio_rustls::server::TlsStream<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let (stream, addr) = match self.tcp.accept().await {
                Ok(pair) => pair,
                Err(e) => {
                    // 与 axum 内建 TcpListener::accept 同口径：accept 瞬时
                    // 错误不终止服务（持续错误打满日志是部署面监控项）。
                    eprintln!("mcp_remote: tcp accept error: {e}");
                    continue;
                }
            };
            match self.acceptor.accept(stream).await {
                Ok(tls) => return (tls, addr),
                Err(e) => eprintln!("mcp_remote: TLS handshake failed from {addr}: {e}"),
            }
        }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.tcp.local_addr()
    }
}

/// 已绑定的远程 MCP 服务（SPEC §2.2 远程入口运行时面）。bind 期完成启动
/// 守卫与 TLS 装配（显式错误，不静默降级）；[`serve`](Self::serve) 消费后
/// 长驻至进程退出。
pub struct RemoteMcpServer {
    tcp: TcpListener,
    tls: Option<TlsAcceptor>,
    router: Router,
}

impl RemoteMcpServer {
    /// 实际绑定地址（bin 日志/探针用；bind 端口 0 时为内核分配端口）。
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.tcp.local_addr()
    }

    /// 服务至出错或进程终止。TLS 配置存在时走 TLS 线位（自签 fixture 亦然）。
    pub async fn serve(self) -> io::Result<()> {
        match self.tls {
            Some(acceptor) => {
                axum::serve(
                    TlsListener {
                        tcp: self.tcp,
                        acceptor,
                    },
                    self.router,
                )
                .await
            }
            None => axum::serve(self.tcp, self.router).await,
        }
    }
}

/// 装配并绑定远程 MCP 服务（bin `partisync-mcp-http` 与 TLS 探针共用）：
/// ①启动守卫 [`check_startup`]（非回环必须 TLS + 显式 allowed_hosts，SPEC
/// §2.2，显式错误不静默降级明文）；②TLS 装配（文件加载失败显式报错）；
/// ③TcpListener bind；④router（gate + rmcp fail-closed，PRM 公开）。
pub async fn bind_server(
    config: &McpRemoteConfig,
    state: Arc<McpServerState>,
) -> Result<RemoteMcpServer, String> {
    check_startup(
        config.bind_addr,
        config.tls.as_ref(),
        config.allowed_hosts.as_deref(),
    )?;
    let tls = match &config.tls {
        Some(files) => Some(TlsAcceptor::from(Arc::new(load_tls_server_config(files)?))),
        None => None,
    };
    let tcp = TcpListener::bind(config.bind_addr)
        .await
        .map_err(|e| format!("bind {} 失败: {e}", config.bind_addr))?;
    let router = build_router(config, state);
    Ok(RemoteMcpServer { tcp, tls, router })
}
