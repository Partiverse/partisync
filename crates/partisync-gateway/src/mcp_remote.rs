//! Gateway 远程 MCP 传输面 + 授权面（M10-WP05-T03/T04，SPEC
//! docs/specs/M10-WP05.md §2.2/§2.3，ADR-0031 决策 2/3/4/5）：Streamable
//! HTTP + Origin/host 校验（先于授权面）+ 真实 OAuth 2.1 RS 验签
//! （jsonwebtoken 11：iss/aud(RFC 8707)/exp/scope fail-closed，401/403
//! 语义）+ rustls TLS 线位与非回环启动守卫。端点沿 M9 骨架口径
//! （`POST /mcp` + PRM）；远程入口 bin = `partisync-mcp-http`。
//!
//! 分派序（探针钉死，P23）：PRM 公开无墙；其余请求过 gate——Host 白名单
//! （rmcp 语义镜像）→ Origin 白名单（名单空 = 关）→ 授权墙
//! [`AuthWall`]（配置缺省 = fail-closed 全 401 墙态；配置授权后
//! Bearer 验签）→ scope gate（仅 `tools/call`，不足 403
//! `insufficient_scope`；协议元面随合法 token）→ rmcp 服务。运行时面：
//! [`bind_server`]（启动守卫 + JWKS 预拉 fail-fast + TLS 装配 + bind）→
//! [`RemoteMcpServer::serve`]。探针见 `tests/wp05_mcp_remote.rs`；
//! 非回环前置条件见 [`check_startup`]。

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::uri::Authority;
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
use axum::Router;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::tower::{
    StreamableHttpServerConfig, StreamableHttpService,
};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use serde::Deserialize;
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
    /// 授权面配置（ADR-0031 决策 1/3）。None = fail-closed 墙态
    /// （[`AuthWall::FailClosedAll`]，除 PRM 外一切 401——回环开发默认态）；
    /// Some = Bearer 验签面。RFC 8707 受众绑定取 `prm.resource`。
    pub auth: Option<AuthConfig>,
}

impl Default for McpRemoteConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], DEFAULT_BIND_PORT)),
            tls: None,
            allowed_hosts: None,
            allowed_origins: Vec::new(),
            prm: PrmConfig::default(),
            auth: None,
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
/// rebinding 防线失效，rmcp 空名单语义为放行）→ 拒绝启动；③授权配置
/// （T04 接线，SPEC §6-R9 窗口期设计收口）：缺省 → 拒绝启动（公网面
/// 必须显式配置 issuer/JWKS，fail-closed 不给「忘了配授权」留口）。
pub fn check_startup(
    bind_addr: SocketAddr,
    tls: Option<&TlsFiles>,
    allowed_hosts: Option<&[String]>,
    auth: Option<&AuthConfig>,
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
    if auth.is_none() {
        return Err(format!(
            "启动守卫拒绝：bind {bind_addr} 为非回环地址且未配置授权面 \
             （缺省为 fail-closed 全 401 墙态，服务不可用）\
             ——用 --issuer/--jwks-uri 显式配置受信授权服务器"
        ));
    }
    Ok(())
}

/// 受信 issuer 及其 JWKS 端点（ADR-0031 决策 1：AS 为外部配置，RS 只校验；
/// `jwks_uri` 为部署面受信配置而非用户输入，不做 SSRF 环回过滤——本地
/// mock AS/内网 IdP 是合法部署形态）。
#[derive(Debug, Clone)]
pub struct TrustedIssuer {
    /// token `iss` 必须精确等于此值（P23-a）。
    pub issuer: String,
    /// 该 issuer 的 JWKS 端点（`GET` 返回 RFC 7517 JwkSet）。
    pub jwks_uri: String,
}

/// 授权面配置（P23 载体，SPEC §2.3）。受众绑定目标 = [`PrmConfig::resource`]
/// （RFC 8707：token `aud` MUST 含本 resource），不单列配置防两处漂移。
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// 受信 issuer 集（空 = 配置错误，`TokenValidator` 拒绝构建）。
    pub issuers: Vec<TrustedIssuer>,
    /// 算法 allowlist（ADR-0031 决策 3：显式钉定，不接受 token header 任指
    /// 算法；header `alg` ∉ 列表即 401）。
    pub algorithms: Vec<Algorithm>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            issuers: Vec::new(),
            algorithms: vec![Algorithm::RS256, Algorithm::ES256, Algorithm::EdDSA],
        }
    }
}

/// 已认证身份（P23 放行面）。`subject` 供 T05 跨请求状态键控（T-R5：
/// `<subject>:<handle>`，不以 handle 替代鉴权）。
#[derive(Debug, Clone)]
pub struct Authorized {
    pub subject: Option<String>,
    /// token `scope` 集（OAuth 2.0 空格分隔；数组形态宽容解析）。
    pub scopes: Vec<String>,
}

impl Authorized {
    fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}

/// access token claims（仅取消费面字段；`iss`/`aud`/`exp` 由
/// [`Validation`] 在 required-claims + 专字段校验，不在此重复声明）。
#[derive(Debug, Deserialize)]
struct AccessTokenClaims {
    #[serde(default)]
    sub: Option<String>,
    #[serde(default)]
    scope: ScopeSet,
}

/// OAuth 2.0 `scope` claim：标准为空格分隔字符串；宽容接受字符串数组
/// （部分 IdP 变体），其余形态视为空集（scope 面 fail-closed，不挡身份）。
#[derive(Debug, Default, Clone)]
struct ScopeSet(Vec<String>);

impl<'de> Deserialize<'de> for ScopeSet {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        let scopes = match value {
            serde_json::Value::String(s) => s.split_whitespace().map(str::to_string).collect(),
            serde_json::Value::Array(items) => items
                .into_iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            _ => Vec::new(),
        };
        Ok(ScopeSet(scopes))
    }
}

/// JWKS 缓存行（60s TTL；`kid` miss 强刷一次应对 key 轮换）。
#[derive(Clone)]
struct CachedJwks {
    jwks: Arc<JwkSet>,
    fetched_at: Instant,
}

/// RS 侧 access token 校验器（ADR-0031 决策 3：jsonwebtoken 11 验签 +
/// reqwest JWKS 拉取手写缓存）。校验序（P23-a，fail-closed 全行 401）：
/// header `alg` ∈ allowlist → `iss` 精确命中受信 issuer（未验签 peek，
/// 由此绑定「issuer → 其 JWKS」1:1，杜绝跨 issuer key 混淆）→ JWKS 按
/// `kid` 取钥（miss 强刷一次）→ `decode`（签名 + iss/aud(RFC 8707)/exp +
/// required claims `exp/iss/aud`）。
pub struct TokenValidator {
    client: reqwest::Client,
    /// RFC 8707 受众 = 本 resource（`PrmConfig::resource`）。
    audience: String,
    issuers: Vec<TrustedIssuer>,
    algorithms: Vec<Algorithm>,
    jwks_cache: RwLock<HashMap<String, CachedJwks>>,
}

/// JWKS 缓存 TTL（轮换容忍窗口；`kid` miss 不等 TTL 直接强刷）。
const JWKS_CACHE_TTL: Duration = Duration::from_secs(60);

impl TokenValidator {
    /// 构建校验器（不拉取；`bind_server` 启动期调 [`Self::load_all`]
    /// fail-fast）。空 issuer 集为配置错误，显式报错。
    pub fn new(auth: &AuthConfig, audience: String) -> Result<Self, String> {
        if auth.issuers.is_empty() {
            return Err(
                "授权配置错误：受信 issuer 集为空（--issuer/--jwks-uri 必须成对给出）".into(),
            );
        }
        if auth.algorithms.is_empty() {
            return Err(
                "授权配置错误：algorithm allowlist 为空（fail-closed 拒绝放行任一算法）".into(),
            );
        }
        Ok(Self {
            client: reqwest::Client::new(),
            audience,
            issuers: auth.issuers.clone(),
            algorithms: auth.algorithms.clone(),
            jwks_cache: RwLock::new(HashMap::new()),
        })
    }

    /// 启动期全量预拉 JWKS（bind 期 fail-fast：配置的 JWKS 端点不可达 =
    /// 显式拒绝启动，不留「跑到第一个请求才发现」的窗口）。
    pub async fn load_all(&self) -> Result<(), String> {
        for issuer in &self.issuers {
            self.fetch_jwks(&issuer.jwks_uri)
                .await
                .map_err(|e| format!("JWKS 预拉失败（{}）: {e}", issuer.jwks_uri))?;
        }
        Ok(())
    }

    async fn fetch_jwks(&self, uri: &str) -> Result<Arc<JwkSet>, String> {
        let resp = self
            .client
            .get(uri)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("请求失败: {e}"))?
            .error_for_status()
            .map_err(|e| format!("HTTP 状态异常: {e}"))?;
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("响应体读取失败: {e}"))?;
        let jwks: JwkSet =
            serde_json::from_slice(&bytes).map_err(|e| format!("JwkSet 解析失败: {e}"))?;
        Ok(Arc::new(jwks))
    }

    /// 带缓存的 JWKS 获取（60s TTL；锁不跨 await，拉取在锁外完成）。
    async fn jwks_for(&self, uri: &str) -> Result<Arc<JwkSet>, String> {
        if let Some(cached) = self
            .jwks_cache
            .read()
            .ok()
            .and_then(|cache| cache.get(uri).cloned())
            .filter(|c| c.fetched_at.elapsed() < JWKS_CACHE_TTL)
        {
            return Ok(cached.jwks);
        }
        let jwks = self.fetch_jwks(uri).await?;
        if let Ok(mut cache) = self.jwks_cache.write() {
            cache.insert(
                uri.to_string(),
                CachedJwks {
                    jwks: Arc::clone(&jwks),
                    fetched_at: Instant::now(),
                },
            );
        }
        Ok(jwks)
    }

    /// 授权裁决：`token` 为剥去 `Bearer ` 前缀的裸 JWT。`Err` = 401 挑战
    /// 静态 reason（不含 token 内容，不泄露载荷）。
    pub async fn authorize(&self, token: &str) -> Result<Authorized, &'static str> {
        // ① header 算法 allowlist（ADR-0031 决策 3：不接受 header 任指算法）。
        let header = decode_header(token).map_err(|_| "malformed token")?;
        if !self.algorithms.contains(&header.alg) {
            return Err("algorithm not allowed");
        }
        // ② 未验签 peek `iss` → 绑定受信 issuer（未知 iss 直接拒，不做
        //    JWKS 查找——key 与 issuer 1:1，跨 issuer key 混淆不可达）。
        let peek: serde_json::Value = jsonwebtoken::dangerous::insecure_decode(token)
            .map_err(|_| "malformed token")?
            .claims;
        let issuer = peek
            .get("iss")
            .and_then(|v| v.as_str())
            .ok_or("missing iss claim")?;
        let trusted = self
            .issuers
            .iter()
            .find(|t| t.issuer == issuer)
            .ok_or("issuer not trusted")?;
        // ③ JWKS 按 `kid` 取钥；miss 强刷一次（key 轮换窗口），仍无即拒。
        let Some(kid) = header.kid.as_deref() else {
            return Err("missing kid");
        };
        let jwks = self
            .jwks_for(&trusted.jwks_uri)
            .await
            .map_err(|_| "jwks unavailable")?;
        let jwk = jwks.find(kid).ok_or("unknown kid")?;
        let key = DecodingKey::from_jwk(jwk).map_err(|_| "invalid jwk")?;
        // ④ 验签 + iss/aud(RFC 8707)/exp + required claims 全 fail-closed
        //    （`aud`/`iss` 仅在 claim 存在时才校验，故 required 钉死三者，
        //    防「缺 claim 绕过受众绑定」）。
        let mut validation = Validation::new(header.alg);
        validation.algorithms = self.algorithms.clone();
        validation.set_issuer(&[trusted.issuer.as_str()]);
        validation.set_audience(&[self.audience.as_str()]);
        validation.set_required_spec_claims(&["exp", "iss", "aud"]);
        let data =
            decode::<AccessTokenClaims>(token, &key, &validation).map_err(|e| reject_reason(&e))?;
        Ok(Authorized {
            subject: data.claims.sub,
            scopes: data.claims.scope.0,
        })
    }
}

/// 手工 Debug（reqwest Client / JWKS 缓存内件非 Debug；无敏感内容）。
impl std::fmt::Debug for TokenValidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenValidator")
            .field("audience", &self.audience)
            .field("issuers", &self.issuers)
            .field("algorithms", &self.algorithms)
            .finish_non_exhaustive()
    }
}

/// JWT 校验失败 → 挑战静态 reason（RFC 6750 `invalid_token`）。
fn reject_reason(err: &jsonwebtoken::errors::Error) -> &'static str {
    use jsonwebtoken::errors::ErrorKind as K;
    match err.kind() {
        K::InvalidSignature => "bad signature",
        K::ExpiredSignature => "token expired",
        K::InvalidIssuer => "issuer mismatch",
        K::InvalidAudience => "audience mismatch",
        K::InvalidAlgorithm => "algorithm mismatch",
        K::MissingRequiredClaim(_) => "missing required claim",
        _ => "invalid token",
    }
}

/// 工具 → scope 映射（ADR-0031 决策 5 / 评估件 §3 全量表：11 内建工具 +
/// `ext_*` 动态面保守归 `mcp:write`；未知工具名同构归高域 fail-closed）。
pub fn required_scope_for_tool(tool: &str) -> &'static str {
    const READ: &str = "mcp:read";
    const WRITE: &str = "mcp:write";
    const EXPORT: &str = "mcp:export";
    match tool {
        "asset_search" | "asset_read" | "job_status" | "memory_search" | "memory_verify"
        | "ext_list" => READ,
        "asset_organize" | "memory_write" | "memory_update" | "memory_delete" => WRITE,
        "dataset_export" => EXPORT,
        _ => WRITE,
    }
}

/// 授权墙裁决（P23 分派序第③层）。
#[derive(Debug)]
pub enum WallDecision {
    /// 已认证 → 进入 scope gate（`tools/call`）后放行 rmcp。
    Authenticated(Authorized),
    /// 拒绝（401 挑战，`WWW-Authenticate` 已带 `resource_metadata`）。
    Reject(GateRejection),
}

/// 授权墙（SPEC §2.2「墙先于门」→ §2.3 真实 OAuth 2.1 RS）。
/// `FailClosedAll` = 授权配置缺省的默认态（除 PRM 外一切 401，无数据通路）；
/// `Bearer` = jsonwebtoken 验签面（[`TokenValidator::authorize`]）。
#[derive(Debug, Clone)]
pub enum AuthWall {
    /// 全量 401：任何请求都未过授权校验（T03 默认态；T04 起仅当授权
    /// 配置缺省时保留——回环开发态）。
    FailClosedAll {
        /// PRM 文档 URL（挑战 `resource_metadata` 指向，评估件 A-2 形状）。
        prm_metadata_url: String,
        /// 挑战 `scope` 参数（spec Scope Selection Strategy：挑战中优先给出）。
        scopes: String,
    },
    /// Bearer 验签面（T04）：无/畸形 token 或验签失败 → 401 invalid_token。
    Bearer {
        validator: Arc<TokenValidator>,
        prm_metadata_url: String,
        scopes: String,
    },
}

/// `Authorization: Bearer <token>` 剥前缀（scheme 大小写不敏感，RFC 7235）。
fn extract_bearer_token(value: &str) -> Option<&str> {
    let (scheme, rest) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = rest.trim();
    (!token.is_empty()).then_some(token)
}

impl AuthWall {
    /// 裁决：`authorization` 为原始 `Authorization` 头值（async——JWKS
    /// 按 TTL 刷新）。
    pub async fn decide(&self, authorization: Option<&HeaderValue>) -> WallDecision {
        match self {
            AuthWall::FailClosedAll {
                prm_metadata_url,
                scopes,
            } => WallDecision::Reject(unauthorized_challenge(prm_metadata_url, scopes, None)),
            AuthWall::Bearer {
                validator,
                prm_metadata_url,
                scopes,
            } => {
                let challenge = || unauthorized_challenge(prm_metadata_url, scopes, None);
                let Some(value) = authorization.and_then(|v| v.to_str().ok()) else {
                    return WallDecision::Reject(challenge());
                };
                let Some(token) = extract_bearer_token(value) else {
                    return WallDecision::Reject(challenge());
                };
                match validator.authorize(token).await {
                    Ok(authorized) => WallDecision::Authenticated(authorized),
                    Err(reason) => WallDecision::Reject(unauthorized_challenge(
                        prm_metadata_url,
                        scopes,
                        Some(reason),
                    )),
                }
            }
        }
    }
}

/// 401 挑战（A-2 形状）：`Bearer [error=…, error_description=…,]
/// resource_metadata="<PRM>", scope="<supported>"`。`reason` 为静态串，
/// 不回显 token 内容。
fn unauthorized_challenge(
    prm_metadata_url: &str,
    scopes: &str,
    reason: Option<&'static str>,
) -> GateRejection {
    let mut www = format!("Bearer resource_metadata=\"{prm_metadata_url}\", scope=\"{scopes}\"");
    if let Some(reason) = reason {
        www = format!("Bearer error=\"invalid_token\", error_description=\"{reason}\", {www}");
    }
    GateRejection {
        status: StatusCode::UNAUTHORIZED,
        message: "",
        www_authenticate: Some(www),
    }
}

/// 403 step-up 挑战（P23-b）：`Bearer error="insufficient_scope",
/// scope="<所需>", resource_metadata="<PRM>"`（单挑战只含所需 scope，
/// ADR-0031 决策 5）。
fn insufficient_scope_challenge(prm_metadata_url: &str, required: &str) -> GateRejection {
    GateRejection {
        status: StatusCode::FORBIDDEN,
        message: "",
        www_authenticate: Some(format!(
            "Bearer error=\"insufficient_scope\", scope=\"{required}\", \
             resource_metadata=\"{prm_metadata_url}\""
        )),
    }
}

/// gate 总裁决（同步段）：host → origin（先于授权面，SPEC §2.2）。授权墙
/// （async）由 [`gate_middleware`] 在本裁决通过后调用 [`AuthWall::decide`]。
pub fn gate_decision(
    allowed_hosts: &[String],
    allowed_origins: &[String],
    uri: &Uri,
    headers: &HeaderMap,
) -> Option<GateRejection> {
    if let Err(rejection) = host_check(uri, headers, allowed_hosts) {
        return Some(rejection);
    }
    if let Err(rejection) = origin_check(headers, allowed_origins) {
        return Some(rejection);
    }
    None
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

/// 授权墙构建（配置 → 墙态）：`auth` 缺省 = fail-closed 全 401；Some =
/// Bearer 验签面（受众 = `prm.resource`，RFC 8707）。
fn wall_from_config(config: &McpRemoteConfig) -> Result<AuthWall, String> {
    let prm_metadata_url = config.prm.metadata_url();
    let scopes = config.prm.scopes_supported.join(" ");
    match &config.auth {
        None => Ok(AuthWall::FailClosedAll {
            prm_metadata_url,
            scopes,
        }),
        Some(auth) => {
            let validator = TokenValidator::new(auth, config.prm.resource.clone())?;
            Ok(AuthWall::Bearer {
                validator: Arc::new(validator),
                prm_metadata_url,
                scopes,
            })
        }
    }
}

/// 组装远程 MCP router：PRM 公开路由 + gate 中间件包住的 rmcp 服务（`POST
/// /mcp` 单端点）。授权配置缺省时 gate 对一切非 PRM 请求 401（fail-closed
/// 默认态）；配置授权后走 Bearer 验签（T04）。
pub fn build_router(config: &McpRemoteConfig, mcp_state: Arc<McpServerState>) -> Router {
    build_router_with_wall(
        config,
        wall_from_config(config).expect("auth config"),
        mcp_state,
    )
}

/// [`build_router`] 的墙注入变体：`bind_server` 在启动期完成 JWKS 预拉
/// fail-fast 后注入同一 validator（探针/运行时共用，无双源漂移）。
pub fn build_router_with_wall(
    config: &McpRemoteConfig,
    wall: AuthWall,
    mcp_state: Arc<McpServerState>,
) -> Router {
    let allowed_hosts = config.effective_allowed_hosts();
    let gate_state = Arc::new(GateState {
        allowed_hosts: allowed_hosts.clone(),
        allowed_origins: config.allowed_origins.clone(),
        wall,
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

/// scope gate 的体预读上限（2 MiB，与 axum 默认请求体限额同量级）；
/// 超限请求 fail-closed 显式 400，绝不未验 scope 抵达工具执行。
const SCOPE_PEEK_BODY_LIMIT: usize = 2 * 1024 * 1024;

/// gate 中间件（P23 分派序）：host/origin（同步，先于授权面）→ 授权墙
/// （async 验签）→ scope gate（仅 `tools/call`：所需 scope ∉ token scope
/// 集 → 403 insufficient_scope；协议元面随合法 token）→ rmcp。
async fn gate_middleware(
    State(gate): State<Arc<GateState>>,
    request: Request,
    next: Next,
) -> Response {
    // ① host/origin 白名单（先于授权面：拒绝不产生 401 挑战头）。
    let (parts, body) = request.into_parts();
    if let Some(rejection) = gate_decision(
        &gate.allowed_hosts,
        &gate.allowed_origins,
        &parts.uri,
        &parts.headers,
    ) {
        return rejection.into_response();
    }
    // ② 授权墙：FailClosedAll 一切 401；Bearer 验签失败 401 invalid_token。
    let authorized = match gate
        .wall
        .decide(parts.headers.get(header::AUTHORIZATION))
        .await
    {
        WallDecision::Reject(rejection) => return rejection.into_response(),
        WallDecision::Authenticated(authorized) => authorized,
    };
    // ③ scope gate：POST 才可能携带 `tools/call`，需预读体判定方法名。
    if parts.method == Method::POST {
        let bytes = match axum::body::to_bytes(body, SCOPE_PEEK_BODY_LIMIT).await {
            Ok(bytes) => bytes,
            Err(_) => {
                return GateRejection {
                    status: StatusCode::BAD_REQUEST,
                    message: "Bad Request: unable to buffer request body for scope check",
                    www_authenticate: None,
                }
                .into_response();
            }
        };
        if let Some(tool) = tools_call_tool_name(&bytes) {
            let required = required_scope_for_tool(&tool);
            if !authorized.has_scope(required) {
                return insufficient_scope_challenge(&gate.prm.metadata_url(), required)
                    .into_response();
            }
        }
        let request = Request::from_parts(parts, Body::from(bytes));
        return next.run(request).await;
    }
    let request = Request::from_parts(parts, body);
    next.run(request).await
}

/// 从 JSON-RPC 请求体提取 `tools/call` 的工具名（非 tools/call / 解析失败
/// → None——交由 rmcp 内层按协议处置，scope gate 不放行任何未过校验请求）。
fn tools_call_tool_name(body: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    if value.get("method")?.as_str()? != "tools/call" {
        return None;
    }
    value
        .get("params")?
        .get("name")?
        .as_str()
        .map(str::to_string)
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
/// ①授权墙构建（配置 → [`AuthWall`]）+ JWKS 预拉 fail-fast（T04：配置的
/// JWKS 端点不可达 = 显式拒绝启动）；②启动守卫 [`check_startup`]（非回环
/// 必须 TLS + 显式 allowed_hosts + 显式授权配置，SPEC §2.2/§2.3，显式错误
/// 不静默降级）；③TLS 装配（文件加载失败显式报错）；④TcpListener bind；
/// ⑤router（gate + rmcp fail-closed，PRM 公开）。
pub async fn bind_server(
    config: &McpRemoteConfig,
    state: Arc<McpServerState>,
) -> Result<RemoteMcpServer, String> {
    let wall = wall_from_config(config)?;
    if let AuthWall::Bearer { validator, .. } = &wall {
        validator
            .load_all()
            .await
            .map_err(|e| format!("JWKS 预拉失败（fail-closed 拒绝启动）: {e}"))?;
    }
    check_startup(
        config.bind_addr,
        config.tls.as_ref(),
        config.allowed_hosts.as_deref(),
        config.auth.as_ref(),
    )?;
    let tls = match &config.tls {
        Some(files) => Some(TlsAcceptor::from(Arc::new(load_tls_server_config(files)?))),
        None => None,
    };
    let tcp = TcpListener::bind(config.bind_addr)
        .await
        .map_err(|e| format!("bind {} 失败: {e}", config.bind_addr))?;
    let router = build_router_with_wall(config, wall, state);
    Ok(RemoteMcpServer { tcp, tls, router })
}
