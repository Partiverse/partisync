//! Hub 远程 MCP 端点**骨架**（M9-WP05-T01，SPEC docs/specs/M9-WP05.md §2.2）。
//!
//! **mock/骨架边界（硬性，一期不承诺生产可用）**：
//! - 不接真实 OAuth——`Authorization: Bearer` 头**存在即放行**，token 一律
//!   不验证（该边界由探针 `mock_bearer_any_value_is_accepted` 钉死为明示
//!   行为而非遗漏；接真实 OAuth 时此探针必红 = 强制修订入口）；
//! - 无 TLS 终结、无 Origin 校验、无 quota 接线（演示面仅 127.0.0.1）；
//! - 无工具执行、不承载真实数据——`initialize` 之外的 JSON-RPC 方法一律
//!   `404 + -32601`；
//! - PRM 文档中的 `authorization_servers` 缺省为 mock 值，不代表任何可用
//!   授权服务器。
//!
//! 形状依据（2026-07-28 spec，起草期 2026-10-04 在线实证，引文见评估文档
//! docs/reviews/M9-WP05-mcp2-remote-eval.md §2）：
//! - PRM（RFC 9728）`GET /.well-known/oauth-protected-resource`：MCP servers
//!   MUST implement PRM 且文档 MUST 含 `authorization_servers`（≥1）；
//! - 401 挑战：`WWW-Authenticate: Bearer resource_metadata="<PRM 路径>"`；
//! - 单一 MCP endpoint `POST /mcp`：notification → `202 Accepted` 无体；
//!   未实现方法 → `404 + JSON-RPC -32601`（与遗留裸 404 区分）；
//! - 2026-07-28 无协议级 session（GET 流/`Mcp-Session-Id` 已移除）——本
//!   骨架天然 stateless，每请求自足。

use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use serde_json::{json, Value as JsonValue};

/// 本骨架锚定的协议版本（rmcp 3.4.0 同版，Cargo.toml:64 ADR-0019）。
pub const MCP_PROTOCOL_VERSION: &str = "2026-07-28";

/// PRM well-known 根路径（RFC 9728；MCP 认可的两种放置形态之一，见
/// 评估文档 A-2）。
pub const PRM_WELLKNOWN_PATH: &str = "/.well-known/oauth-protected-resource";

/// MCP endpoint 路径（2026-07-28 单端点惯例，spec 例 `…/mcp`）。
pub const MCP_ENDPOINT_PATH: &str = "/mcp";

/// JSON-RPC：Method not found（404 体携带，spec G-6）。
const METHOD_NOT_FOUND: i64 = -32601;
/// JSON-RPC：Invalid Request（body 形状非法时的骨架级 400 体）。
const INVALID_REQUEST: i64 = -32600;

/// 骨架配置（PRM 文档字段来源；全部缺省为 mock 值）。
#[derive(Debug, Clone)]
pub struct RemoteMcpSkeleton {
    /// RFC 9728 `resource`（canonical URI；缺省 mock，不代表可达端点）。
    pub resource: String,
    /// RFC 9728 `authorization_servers`（MCP MUST ≥1；缺省 mock AS）。
    pub authorization_servers: Vec<String>,
    /// RFC 9728 `scopes_supported`（最小集口径，评估文档 T-R6）。
    pub scopes_supported: Vec<String>,
}

impl Default for RemoteMcpSkeleton {
    fn default() -> Self {
        Self {
            resource: "https://mcp.partisync.invalid".to_string(),
            authorization_servers: vec!["https://auth.partisync.invalid".to_string()],
            scopes_supported: vec!["mcp:basic".to_string()],
        }
    }
}

/// POST /mcp 的纯函数裁决结果（handler 薄包装的目标形状）。
#[derive(Debug, PartialEq)]
pub struct McpPostOutcome {
    pub status: StatusCode,
    /// Some = `WWW-Authenticate` 头值（401 挑战）。
    pub www_authenticate: Option<String>,
    /// Some = JSON 响应体；None = 无体（202 notification 路径）。
    pub body: Option<JsonValue>,
}

impl RemoteMcpSkeleton {
    /// 构造骨架 router（PRM 发现 + MCP endpoint）。内部自带 state，
    /// 返回 `Router<()>`，调用方直接 `merge`。
    pub fn router(self) -> Router {
        Router::new()
            .route(PRM_WELLKNOWN_PATH, get(prm_handler))
            .route(MCP_ENDPOINT_PATH, post(mcp_post_handler))
            .with_state(Arc::new(self))
    }

    /// PRM 文档（RFC 9728 形状；纯函数，可独立断言）。
    pub fn protected_resource_metadata(&self) -> JsonValue {
        json!({
            "resource": self.resource,
            "authorization_servers": self.authorization_servers,
            "scopes_supported": self.scopes_supported,
            "bearer_methods_supported": ["header"],
        })
    }

    /// `POST /mcp` 裁决（纯函数；`authorization` 为原始头值）。
    ///
    /// 分派序：授权缺失/非 Bearer → 401 挑战；无 `id` 视为 notification →
    /// 202；`initialize` → 骨架握手响应；其余 → 404 + `-32601`。
    pub fn handle_mcp_post(&self, authorization: Option<&str>, body: &JsonValue) -> McpPostOutcome {
        let is_bearer = authorization.is_some_and(|v| {
            v.trim()
                .get(..6)
                .map(|s| s.eq_ignore_ascii_case("Bearer"))
                .unwrap_or(false)
        });
        if !is_bearer {
            return McpPostOutcome {
                status: StatusCode::UNAUTHORIZED,
                www_authenticate: Some(format!(
                    "Bearer resource_metadata=\"{PRM_WELLKNOWN_PATH}\", scope=\"{}\"",
                    self.scopes_supported.join(" ")
                )),
                body: None,
            };
        }
        // mock 边界：Bearer 存在即放行，token 一律不验证（见模块 doc）。
        let obj = match body.as_object() {
            Some(o) => o,
            None => {
                return jsonrpc_error(
                    StatusCode::BAD_REQUEST,
                    INVALID_REQUEST,
                    "body 必须是 JSON-RPC message",
                    None,
                );
            }
        };
        let id = obj.get("id").cloned();
        let method = obj.get("method").and_then(JsonValue::as_str);
        let Some(method) = method else {
            return jsonrpc_error(
                StatusCode::BAD_REQUEST,
                INVALID_REQUEST,
                "缺少 JSON-RPC method",
                id,
            );
        };
        // 2026-07-28：notification（无 id）→ 202 Accepted 无体。
        let Some(id) = id else {
            return McpPostOutcome {
                status: StatusCode::ACCEPTED,
                www_authenticate: None,
                body: None,
            };
        };
        match method {
            "initialize" => McpPostOutcome {
                status: StatusCode::OK,
                www_authenticate: None,
                body: Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": MCP_PROTOCOL_VERSION,
                        "capabilities": { "tools": { "listChanged": false } },
                        "serverInfo": {
                            "name": "partisync-hub-mcp-remote-skeleton",
                            "version": "0.2.0",
                        },
                        "instructions": "M9-WP05 骨架（不承诺生产可用）：仅握手评估面；token 不验证（mock）；无工具执行。",
                    },
                })),
            },
            // 其余方法一律未实现（404 + -32601，spec 与遗留裸 404 区分）。
            _ => jsonrpc_error(
                StatusCode::NOT_FOUND,
                METHOD_NOT_FOUND,
                format!("骨架未实现方法: {method}"),
                Some(id),
            ),
        }
    }
}

fn jsonrpc_error(
    status: StatusCode,
    code: i64,
    message: impl Into<String>,
    id: Option<JsonValue>,
) -> McpPostOutcome {
    McpPostOutcome {
        status,
        www_authenticate: None,
        body: Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message.into() },
        })),
    }
}

async fn prm_handler(State(skeleton): State<Arc<RemoteMcpSkeleton>>) -> Json<JsonValue> {
    Json(skeleton.protected_resource_metadata())
}

async fn mcp_post_handler(
    State(skeleton): State<Arc<RemoteMcpSkeleton>>,
    headers: HeaderMap,
    Json(body): Json<JsonValue>,
) -> Response {
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    outcome_to_response(skeleton.handle_mcp_post(authorization, &body))
}

fn outcome_to_response(outcome: McpPostOutcome) -> Response {
    let mut resp = match outcome.body {
        Some(body) => (outcome.status, Json(body)).into_response(),
        None => outcome.status.into_response(),
    };
    if let Some(www) = outcome.www_authenticate {
        if let Ok(value) = HeaderValue::from_str(&www) {
            resp.headers_mut().insert(header::WWW_AUTHENTICATE, value);
        }
    }
    resp
}
