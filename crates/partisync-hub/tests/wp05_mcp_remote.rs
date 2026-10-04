//! M9-WP05-T01 探针：Hub 远程 MCP 端点骨架（SPEC docs/specs/M9-WP05.md §4）。
//!
//! 全部探针打在纯函数核心（`RemoteMcpSkeleton`）上——handler 是薄包装，
//! 不引 tower/HTTP 客户端（SPEC §2.2 零新增 dev-dependency 约束）。

use axum::http::StatusCode;
use partisync_hub::mcp_remote::{
    McpPostOutcome, RemoteMcpSkeleton, MCP_ENDPOINT_PATH, MCP_PROTOCOL_VERSION, PRM_WELLKNOWN_PATH,
};
use serde_json::{json, Value};

fn skeleton() -> RemoteMcpSkeleton {
    RemoteMcpSkeleton::default()
}

fn post(sk: &RemoteMcpSkeleton, authorization: Option<&str>, body: &Value) -> McpPostOutcome {
    sk.handle_mcp_post(authorization, body)
}

// ── PRM 探针（SPEC §4：四字段文档）─────────────────────────────────────

#[test]
fn prm_document_has_required_shape() {
    let doc = skeleton().protected_resource_metadata();
    // RFC 9728 resource + MCP MUST authorization_servers(≥1) + 两辅助字段。
    assert!(doc.get("resource").and_then(Value::as_str).is_some());
    let servers = doc
        .get("authorization_servers")
        .and_then(Value::as_array)
        .expect("PRM 必须含 authorization_servers 数组");
    assert!(
        !servers.is_empty(),
        "authorization_servers 至少一个（MCP MUST）"
    );
    assert!(doc
        .get("scopes_supported")
        .and_then(Value::as_array)
        .is_some());
    assert_eq!(
        doc.get("bearer_methods_supported"),
        Some(&json!(["header"])),
        "bearer_methods_supported = header（RFC 9728 形状）"
    );
}

// ── 401 挑战探针（SPEC §4：缺授权 → 401 + resource_metadata 指向 PRM）──

#[test]
fn missing_authorization_gets_401_challenge_pointing_at_prm() {
    let sk = skeleton();
    let out = post(
        &sk,
        None,
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
    );
    assert_eq!(out.status, StatusCode::UNAUTHORIZED);
    let www = out.www_authenticate.expect("401 必带 WWW-Authenticate");
    assert!(www.starts_with("Bearer"), "scheme = Bearer：{www}");
    assert!(
        www.contains(&format!("resource_metadata=\"{PRM_WELLKNOWN_PATH}\"")),
        "挑战指向 PRM 路径：{www}"
    );
    assert!(out.body.is_none(), "401 无 JSON-RPC 体");
}

#[test]
fn non_bearer_authorization_gets_401_challenge() {
    let sk = skeleton();
    let out = post(
        &sk,
        Some("Basic dXNlcjpwYXNz"),
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize"}),
    );
    assert_eq!(out.status, StatusCode::UNAUTHORIZED);
    assert!(out.www_authenticate.is_some());
}

// ── 握手探针（SPEC §4：initialize / 202 / 404+-32601）─────────────────

#[test]
fn bearer_initialize_returns_skeleton_handshake() {
    let sk = skeleton();
    let out = post(
        &sk,
        Some("Bearer some-opaque-token"),
        &json!({"jsonrpc":"2.0","id":7,"method":"initialize","params":{"protocolVersion":MCP_PROTOCOL_VERSION}}),
    );
    assert_eq!(out.status, StatusCode::OK);
    assert!(out.www_authenticate.is_none());
    let body = out.body.expect("initialize 有 JSON 体");
    assert_eq!(body.get("jsonrpc"), Some(&json!("2.0")));
    assert_eq!(body.get("id"), Some(&json!(7)), "id 回显");
    let result = body.get("result").expect("initialize result");
    assert_eq!(
        result.get("protocolVersion"),
        Some(&json!(MCP_PROTOCOL_VERSION)),
        "protocolVersion = 2026-07-28（rmcp 3.4.0 同版）"
    );
    let server_info = result.get("serverInfo").expect("serverInfo");
    let name = server_info
        .get("name")
        .and_then(Value::as_str)
        .expect("serverInfo.name");
    assert!(
        name.contains("skeleton"),
        "serverInfo.name 明示骨架身份：{name}"
    );
}

#[test]
fn mock_bearer_any_value_is_accepted() {
    // 骨架边界探针：token 一律不验证——「存在即放行」是明示行为而非遗漏
    // （SPEC §4；接真实 OAuth 时本探针必红 = 强制修订入口）。
    let sk = skeleton();
    let out = post(
        &sk,
        Some("Bearer definitely-not-a-real-token"),
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize"}),
    );
    assert_eq!(
        out.status,
        StatusCode::OK,
        "任意 Bearer 值放行（mock 边界）"
    );
}

#[test]
fn notification_without_id_gets_202_no_body() {
    let sk = skeleton();
    let out = post(
        &sk,
        Some("Bearer t"),
        &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    );
    assert_eq!(
        out.status,
        StatusCode::ACCEPTED,
        "notification → 202（2026-07-28 传输语义）"
    );
    assert!(out.body.is_none(), "202 无体");
}

#[test]
fn unimplemented_method_gets_404_with_method_not_found_error() {
    let sk = skeleton();
    let out = post(
        &sk,
        Some("Bearer t"),
        &json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"memory_search"}}),
    );
    assert_eq!(
        out.status,
        StatusCode::NOT_FOUND,
        "未实现方法 → 404（与遗留裸 404 区分）"
    );
    let body = out.body.expect("404 带 JSON-RPC 错误体");
    let error = body.get("error").expect("error 对象");
    assert_eq!(error.get("code"), Some(&json!(-32601)), "Method not found");
    assert_eq!(body.get("id"), Some(&json!(9)), "id 回显");
}

#[test]
fn malformed_body_gets_400_invalid_request() {
    let sk = skeleton();
    let out = post(&sk, Some("Bearer t"), &json!({"jsonrpc":"2.0","id":1}));
    assert_eq!(out.status, StatusCode::BAD_REQUEST, "缺 method → 400");
    let body = out.body.expect("400 带 JSON-RPC 错误体");
    assert_eq!(
        body.get("error").and_then(|e| e.get("code")),
        Some(&json!(-32600))
    );
}

// ── 契约面：常量与 mock PRM 配置（SPEC §2.2）──────────────────────────

#[test]
fn endpoint_constants_match_spec_shapes() {
    assert_eq!(MCP_ENDPOINT_PATH, "/mcp");
    assert_eq!(PRM_WELLKNOWN_PATH, "/.well-known/oauth-protected-resource");
    assert_eq!(MCP_PROTOCOL_VERSION, "2026-07-28");
    assert_eq!(
        RemoteMcpSkeleton::default().authorization_servers.len(),
        1,
        "缺省 mock AS ≥1"
    );
}

// ── 模块 doc 的 mock 边界声明静态断言（SPEC §4 可选项）────────────────

#[test]
fn module_doc_declares_mock_boundary() {
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/mcp_remote.rs"));
    assert!(
        src.contains("不接真实 OAuth"),
        "模块 doc 必须声明 mock 边界（token 不验证）"
    );
    assert!(
        src.contains("不承诺生产可用"),
        "模块 doc 必须声明骨架不承诺生产可用"
    );
}
