//! M10-WP05-T03/T04 远程 MCP 传输面 + 授权面全栈探针（SPEC
//! docs/specs/M10-WP05.md §3.1 T03/T04 行；hub 骨架探针判例 M9-WP05）。
//! 行为契约：PRM 公开面四字段、fail-closed 默认态（任意 Bearer 必 401）、
//! Origin/host 白名单先于授权面、非回环启动守卫矩阵、TLS 线位 roundtrip
//! （自签证书到 PRM/墙）；T04（P23）：token 校验矩阵（合法/坏签/错 iss/
//! aud 错配含他 resource/过期/未知 kid/allowlist 外算法/缺 claim）401 +
//! 挑战指 PRM、scope step-up 403 insufficient_scope、协议头/方法语义
//! （G-3/G-6）。

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use partisync_gateway::mcp::McpServerState;
use partisync_gateway::mcp_remote::{
    bind_server, build_router, check_startup, required_scope_for_tool, AuthConfig, McpRemoteConfig,
    PrmConfig, TlsFiles, TokenValidator, TrustedIssuer, PRM_WELLKNOWN_PATH,
};
use sqlx::sqlite::SqlitePoolOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// ─────────────────────────────────────────────────────────────────────────────
// 测试脚手架（进程内真实 axum 服务，127.0.0.1:0 临时端口）
// ─────────────────────────────────────────────────────────────────────────────

async fn test_state() -> Arc<McpServerState> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite");
    Arc::new(McpServerState::from_pool(pool))
}

/// 起进程内服务（明文；TLS 探针在 PR-3 用 `serve_on` 自行装配）。
async fn spawn_plain(allowed_origins: &[&str], prm: Option<PrmConfig>) -> SocketAddr {
    let config = McpRemoteConfig {
        allowed_origins: allowed_origins.iter().map(|s| s.to_string()).collect(),
        prm: prm.unwrap_or_default(),
        ..Default::default()
    };
    let router = build_router(&config, test_state().await);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move { axum::serve(listener, router).await.expect("serve") });
    addr
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().build().expect("client")
}

fn test_prm() -> PrmConfig {
    PrmConfig {
        resource: "https://mcp.example.test".to_string(),
        authorization_servers: vec!["https://as.example.test".to_string()],
        scopes_supported: ["mcp:read", "mcp:write", "mcp:export"]
            .map(str::to_string)
            .to_vec(),
    }
}

/// 原始 TCP HTTP/1.1 请求（Host 头 forge 用——reqwest 会以 URL host 覆写）。
async fn raw_http(addr: SocketAddr, request: &str) -> (u16, String) {
    let mut stream = tokio::net::TcpStream::connect(addr).await.expect("connect");
    stream.write_all(request.as_bytes()).await.expect("write");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read");
    let text = String::from_utf8_lossy(&buf).to_string();
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .unwrap_or_else(|| panic!("no status line: {text}"))
        .parse()
        .expect("status code");
    (status, text)
}

// ─────────────────────────────────────────────────────────────────────────────
// PRM 公开发现面（SPEC §3.1 T03 探针①）
// ─────────────────────────────────────────────────────────────────────────────

/// PRM 返回 ADR-0031 配置值四字段，authorization_servers ≥1，免授权可达。
#[tokio::test(flavor = "multi_thread")]
async fn prm_serves_configured_four_fields() {
    let addr = spawn_plain(&[], Some(test_prm())).await;
    let resp = client()
        .get(format!("http://{addr}{PRM_WELLKNOWN_PATH}"))
        .send()
        .await
        .expect("PRM reachable without token");
    assert_eq!(resp.status(), 200);
    let content_type = resp
        .headers()
        .get("content-type")
        .expect("content-type")
        .to_str()
        .unwrap();
    assert!(
        content_type.starts_with("application/json"),
        "{content_type}"
    );
    let doc: serde_json::Value =
        serde_json::from_slice(&resp.bytes().await.expect("body")).expect("json");
    assert_eq!(doc["resource"], "https://mcp.example.test");
    let servers = doc["authorization_servers"].as_array().expect("array");
    assert!(!servers.is_empty(), "MCP MUST authorization_servers >= 1");
    assert_eq!(servers[0], "https://as.example.test");
    assert_eq!(
        doc["scopes_supported"],
        serde_json::json!(["mcp:read", "mcp:write", "mcp:export"])
    );
    assert_eq!(
        doc["bearer_methods_supported"],
        serde_json::json!(["header"])
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// fail-closed 默认态（SPEC §3.1 T03 探针④）：除 PRM 外一切请求 401 挑战
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn fail_closed_wall_401_for_any_bearer_and_any_path() {
    let addr = spawn_plain(&[], None).await;
    let c = client();
    let mcp = format!("http://{addr}/mcp");
    let initialize = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;

    // 无 token 的合法 initialize 请求 → 401 + 挑战（无数据通路）。
    let resp = c
        .post(&mcp)
        .header("content-type", "application/json")
        .body(initialize)
        .send()
        .await
        .expect("post");
    assert_eq!(resp.status(), 401);
    let www = resp
        .headers()
        .get("www-authenticate")
        .expect("challenge")
        .to_str()
        .unwrap();
    assert!(www.starts_with("Bearer "), "challenge shape: {www}");
    assert!(www.contains("resource_metadata="), "A-2 shape: {www}");
    assert!(
        www.contains(PRM_WELLKNOWN_PATH),
        "challenge points at PRM: {www}"
    );
    assert!(www.contains("mcp:read"), "challenge carries scope: {www}");

    // 任意 Bearer 值（T03 期不校验、也不放行）→ 401。
    for token in ["Bearer arbitrary-token", "Bearer", "Basic dXNlcjpwYXNz"] {
        let resp = c
            .post(&mcp)
            .header("content-type", "application/json")
            .header("authorization", token)
            .body(initialize)
            .send()
            .await
            .expect("post");
        assert_eq!(resp.status(), 401, "any bearer must 401: {token:?}");
    }

    // 其他方法/其他路径同样过墙（除 PRM 外一切请求）。
    for (method, path) in [("GET", "/mcp"), ("DELETE", "/mcp"), ("POST", "/other")] {
        let req = match method {
            "GET" => c.get(format!("http://{addr}{path}")),
            "DELETE" => c.delete(format!("http://{addr}{path}")),
            _ => c.post(format!("http://{addr}{path}")),
        };
        let resp = req.send().await.expect("request");
        assert_eq!(resp.status(), 401, "{method} {path} must hit the wall");
    }

    // PRM 公开面不受墙影响。
    let resp = c
        .get(format!("http://{addr}{PRM_WELLKNOWN_PATH}"))
        .send()
        .await
        .expect("prm");
    assert_eq!(resp.status(), 200);
}

// ─────────────────────────────────────────────────────────────────────────────
// Origin/host 白名单先于授权面（SPEC §3.1 T03 探针③ + §2.2 顺序契约）
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn foreign_origin_rejected_403_before_auth_wall() {
    let addr = spawn_plain(&["https://app.example.test"], None).await;
    let c = client();
    let mcp = format!("http://{addr}/mcp");

    // 非白名单 Origin → 403，且无 401 挑战头（先于授权面的可观察证据）。
    let resp = c
        .post(&mcp)
        .header("origin", "https://evil.example.test")
        .header("content-type", "application/json")
        .body("{}")
        .send()
        .await
        .expect("post");
    assert_eq!(resp.status(), 403, "foreign origin must be forbidden");
    assert!(
        resp.headers().get("www-authenticate").is_none(),
        "origin rejection precedes the auth wall"
    );

    // 白名单 Origin → 穿过 origin 校验，落授权墙 401。
    let resp = c
        .post(&mcp)
        .header("origin", "https://app.example.test")
        .header("content-type", "application/json")
        .body("{}")
        .send()
        .await
        .expect("post");
    assert_eq!(resp.status(), 401);

    // 缺失 Origin → 放行（spec 只约束「存在且非法」），落授权墙 401。
    let resp = c.post(&mcp).send().await.expect("post");
    assert_eq!(resp.status(), 401);
}

#[tokio::test(flavor = "multi_thread")]
async fn foreign_host_rejected_403_loopback_host_passes_to_wall() {
    let addr = spawn_plain(&[], None).await;
    // forge Host（DNS rebinding 形态）→ 403（rmcp 语义镜像）。
    let (status, body) = raw_http(
        addr,
        "POST /mcp HTTP/1.1\r\nHost: evil.example.test\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
    )
    .await;
    assert_eq!(status, 403, "foreign Host must be forbidden");
    assert!(body.contains("Forbidden: Host header"), "{body}");
    // 回环 Host（默认名单，host-only 条目匹配任意端口）→ 落授权墙 401。
    let (status, _) = raw_http(
        addr,
        &format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}",
            addr.port()
        ),
    )
    .await;
    assert_eq!(status, 401);
}

// ─────────────────────────────────────────────────────────────────────────────
// 非回环启动守卫（SPEC §3.1 T03 探针②后半：显式拒绝，不静默降级明文）
// ─────────────────────────────────────────────────────────────────────────────

/// 启动守卫矩阵：回环无条件放行；非回环必须 TLS + 显式 allowed_hosts +
/// 授权配置（T04 接线检查，SPEC §2.3/§6-R9 收口）。
#[test]
fn startup_guard_matrix_non_loopback_requires_tls_and_hosts() {
    let loopback: SocketAddr = "127.0.0.1:8424".parse().expect("addr");
    let non_loopback: SocketAddr = "192.0.2.1:8424".parse().expect("addr"); // TEST-NET-1，纯裁决不 bind
    let tls = TlsFiles {
        cert_path: "cert.pem".into(),
        key_path: "key.pem".into(),
    };
    let auth = AuthConfig {
        issuers: vec![TrustedIssuer {
            issuer: "https://as.example.test".to_string(),
            jwks_uri: "https://as.example.test/jwks.json".to_string(),
        }],
        ..AuthConfig::default()
    };

    // 回环：无 TLS/无授权配置亦放行（本地开发默认态 = fail-closed 墙态）。
    assert!(check_startup(loopback, None, None, None).is_ok());
    assert!(check_startup(loopback, Some(&tls), None, None).is_ok());

    // 非回环无 TLS → 拒绝，错误显式指明 TLS 前置条件。
    let err = check_startup(non_loopback, None, None, None).expect_err("must reject");
    assert!(err.contains("TLS"), "{err}");

    // 非回环 + TLS 但未显式 allowed_hosts → 拒绝（回环默认名单会 403 全拒）。
    let err = check_startup(non_loopback, Some(&tls), None, None).expect_err("must reject");
    assert!(err.contains("allowed_hosts"), "{err}");

    // 非回环 + TLS + 显式空名单 → 拒绝（host 校验整体关闭 = DNS rebinding
    // 防线失效；rmcp 空名单语义为放行，启动面必须拦住）。
    let err = check_startup(non_loopback, Some(&tls), Some(&[]), None).expect_err("must reject");
    assert!(err.contains("空名单"), "{err}");

    // 非回环 + TLS + 显式 allowed_hosts 但授权配置缺省 → 拒绝（T04 接线：
    // 公网面必须显式配置 issuer/JWKS，不给「忘了配授权」留口）。
    let hosts = vec!["mcp.example.test".to_string()];
    let err = check_startup(non_loopback, Some(&tls), Some(&hosts), None).expect_err("must reject");
    assert!(err.contains("授权"), "{err}");

    // 非回环 + TLS + 显式 allowed_hosts + 授权配置 → 放行。
    assert!(check_startup(non_loopback, Some(&tls), Some(&hosts), Some(&auth)).is_ok());
}

/// bind_server 运行时守卫：非回环无 TLS 显式报错；TLS 文件缺失显式报错
/// （不 panic、不回退明文）。
#[tokio::test(flavor = "multi_thread")]
async fn bind_server_rejects_non_loopback_without_tls_and_bad_cert_paths() {
    let state = test_state().await;

    let config = McpRemoteConfig {
        bind_addr: "192.0.2.1:8424".parse().expect("addr"),
        ..Default::default()
    };
    let err = match bind_server(&config, state.clone()).await {
        Ok(_) => panic!("non-loopback without TLS must be rejected"),
        Err(e) => e,
    };
    assert!(err.contains("TLS"), "{err}");

    let config = McpRemoteConfig {
        bind_addr: "127.0.0.1:0".parse().expect("addr"),
        tls: Some(TlsFiles {
            cert_path: "/nonexistent/partisync-test/cert.pem".into(),
            key_path: "/nonexistent/partisync-test/key.pem".into(),
        }),
        ..Default::default()
    };
    let err = match bind_server(&config, state).await {
        Ok(_) => panic!("missing TLS files must be an explicit error"),
        Err(e) => e,
    };
    assert!(err.contains("TLS 证书"), "{err}");
}

// ─────────────────────────────────────────────────────────────────────────────
// TLS 线位 roundtrip（SPEC §3.1 T03 探针②前半：自签证书经 TLS 到 PRM/墙）
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn tls_roundtrip_self_signed_cert_reaches_prm_and_wall() {
    // 自签 fixture（ADR-0031 决策 4：rcgen dev 证书，SPEC §6-R6 仓内可复现分支；
    // "127.0.0.1" 经 rcgen CertificateParams::new 落为 iPAddress SAN）。
    let certified =
        rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_string(), "localhost".to_string()])
            .expect("self-signed cert");
    let dir = tempfile::tempdir().expect("tmpdir");
    let cert_path = dir.path().join("cert.pem");
    let key_path = dir.path().join("key.pem");
    std::fs::write(&cert_path, certified.cert.pem()).expect("write cert");
    std::fs::write(&key_path, certified.signing_key.serialize_pem()).expect("write key");

    let config = McpRemoteConfig {
        bind_addr: "127.0.0.1:0".parse().expect("addr"),
        tls: Some(TlsFiles {
            cert_path,
            key_path,
        }),
        prm: test_prm(),
        ..Default::default()
    };
    let server = bind_server(&config, test_state().await)
        .await
        .expect("bind with TLS");
    let addr = server.local_addr().expect("addr");
    let task = tokio::spawn(async move { server.serve().await.expect("serve") });

    // 自签证书作为唯一受信 root 注入客户端（**不关闭证书校验**）：经 TLS
    // 线位走完整 X.509 验证链（IP SAN 匹配 127.0.0.1）roundtrip 到 PRM。
    // 单请求 5s 超时 + 10s 重试预算——workspace 全量并行下 serve 任务
    // 调度可能迟滞，重试必须能扛过 CPU 争抢窗口。
    let root = reqwest::Certificate::from_der(certified.cert.der().as_ref()).expect("DER cert");
    let client = reqwest::Client::builder()
        .add_root_certificate(root)
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("client");
    let url = format!("https://{addr}{PRM_WELLKNOWN_PATH}");
    let mut resp = None;
    let mut last_err = None;
    for _ in 0..200 {
        match client.get(&url).send().await {
            Ok(r) => {
                resp = Some(r);
                break;
            }
            Err(e) => {
                last_err = Some(e.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
    }
    let resp = resp.unwrap_or_else(|| {
        panic!(
            "PRM reachable over TLS roundtrip (serve task alive={}); last error: {}",
            !task.is_finished(),
            last_err.as_deref().unwrap_or("<none>")
        )
    });
    assert_eq!(resp.status(), 200);
    let doc: serde_json::Value =
        serde_json::from_slice(&resp.bytes().await.expect("body")).expect("json");
    assert_eq!(doc["resource"], "https://mcp.example.test");
    assert!(
        !doc["authorization_servers"]
            .as_array()
            .expect("array")
            .is_empty(),
        "authorization_servers >= 1 over TLS"
    );

    // 授权墙经 TLS 线位同样 fail-closed：任意 Bearer → 401 挑战（墙先于门）。
    let resp = client
        .post(format!("https://{addr}/mcp"))
        .header("content-type", "application/json")
        .header("authorization", "Bearer arbitrary-token")
        .body(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#)
        .send()
        .await
        .expect("post over TLS");
    assert_eq!(resp.status(), 401, "any bearer must 401 over TLS");
    let www = resp
        .headers()
        .get("www-authenticate")
        .expect("challenge")
        .to_str()
        .expect("utf-8");
    assert!(www.contains("resource_metadata="), "A-2 shape: {www}");

    task.abort();
}

// ─────────────────────────────────────────────────────────────────────────────
// T04 授权面（P23，SPEC §2.3/§3.1）：mock AS 仓内线程（jsonwebtoken 同件
// 签发 + JWKS 端点）+ token 校验矩阵 + scope step-up + G-3/G-6 语义
// ─────────────────────────────────────────────────────────────────────────────

const PROBE_RESOURCE: &str = "https://mcp.example.test";
const PROBE_PRM_URL: &str = "https://mcp.example.test/.well-known/oauth-protected-resource";
const PROTOCOL_VERSION: &str = "2026-07-28";
const META_KEY: &str = "io.modelcontextprotocol/protocolVersion";
const PROBE_KID: &str = "probe-key-1";

/// 合法 initialize 请求体（头-体一致 [`PROTOCOL_VERSION`]，G-3 规则）。
const INIT_BODY: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2026-07-28","capabilities":{},"clientInfo":{"name":"wp05-t04-probe","version":"0.1.0"}}}"#;

/// tools/list 请求体（非 initialize 面需 `_meta` 含 protocolVersion +
/// clientCapabilities——2026-07-28 现代路径 required keys，rmcp
/// RequestMetaObject 实证）。
const TOOLS_LIST_BODY: &str = r#"{"jsonrpc":"2.0","id":3,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}"#;

/// mock AS（dev-only，不入产品依赖图）：ES256（P-256）签发 + JWKS HTTP
/// 端点。选 ES256 而非 EdDSA：jsonwebtoken `Jwk::from_encoding_key` 的
/// Ed 分支只接受 PKCS#8 v1 48 字节形态（jwk.rs:534 长度门），rcgen/ring
/// `generate_pkcs8` 产出 v2（含公钥段）被拒；EC 分支无此约束。
/// 双钥设计——`key` 入 JWKS（受信），`alt_key` 不入（坏签行注入用）。
struct MockAs {
    key: EncodingKey,
    alt_key: EncodingKey,
    issuer: String,
    addr: SocketAddr,
}

fn p256_key() -> EncodingKey {
    let kp = rcgen::KeyPair::generate().expect("p-256 keypair");
    EncodingKey::from_ec_der(&kp.serialize_der())
}

async fn spawn_mock_as() -> MockAs {
    let key = p256_key();
    let alt_key = p256_key();
    let mut jwk = Jwk::from_encoding_key(&key, Algorithm::ES256).expect("jwk from key");
    jwk.common.key_id = Some(PROBE_KID.to_string());
    let jwks_doc = serde_json::to_value(JwkSet { keys: vec![jwk] }).expect("jwks json");
    let app = axum::Router::new().route(
        "/jwks.json",
        axum::routing::get(move || {
            let doc = jwks_doc.clone();
            async move { axum::Json(doc) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move { axum::serve(listener, app).await.expect("serve") });
    MockAs {
        key,
        alt_key,
        issuer: "https://as.example.test".to_string(),
        addr,
    }
}

impl MockAs {
    /// 以指定 kid/钥签发 token（`alt_key` 行 = 坏签注入）。
    fn issue(&self, kid: &str, key: &EncodingKey, claims: serde_json::Value) -> String {
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(kid.to_string());
        encode(&header, &claims, key).expect("sign token")
    }
}

/// 标准 claims（iss/aud/sub/scope/iat/exp）。
fn base_claims(iss: &str, aud: &str, scope: &str, exp_offset_secs: i64) -> serde_json::Value {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    serde_json::json!({
        "iss": iss,
        "aud": aud,
        "sub": "wp05-t04-probe",
        "scope": scope,
        "iat": now,
        "exp": now + exp_offset_secs,
    })
}

/// 起带授权面的 gateway 实例（bind_server 内含 JWKS 预拉 fail-fast 路径；
/// bind 127.0.0.1:0 临时端口——缺省 8424 在全量并行下会互相冲突）。
async fn spawn_authed(as_: &MockAs) -> SocketAddr {
    let config = McpRemoteConfig {
        bind_addr: "127.0.0.1:0".parse().expect("addr"),
        prm: test_prm(),
        auth: Some(AuthConfig {
            issuers: vec![TrustedIssuer {
                issuer: as_.issuer.clone(),
                jwks_uri: format!("http://{}/jwks.json", as_.addr),
            }],
            ..AuthConfig::default()
        }),
        ..Default::default()
    };
    let server = bind_server(&config, test_state().await)
        .await
        .expect("bind authed server");
    let addr = server.local_addr().expect("addr");
    tokio::spawn(async move { server.serve().await.expect("serve") });
    addr
}

/// 带 Bearer/协议头的 JSON-RPC POST（Accept 双 MIME——rmcp streamable
/// HTTP 对 POST 强制 `application/json + text/event-stream` 协商；
/// `extra` 供 SEP-2243 `Mcp-Method`/`Mcp-Name` 头——2026-07-28 现代路径
/// 非 initialize 请求必带且与体一致）。
async fn rpc(
    c: &reqwest::Client,
    url: &str,
    token: Option<&str>,
    protocol_header: Option<&str>,
    body: &str,
    extra: &[(&str, &str)],
) -> reqwest::Response {
    let mut req = c
        .post(url)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream");
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    if let Some(v) = protocol_header {
        req = req.header("MCP-Protocol-Version", v);
    }
    for (name, value) in extra {
        req = req.header(*name, *value);
    }
    req.body(body.to_string()).send().await.expect("rpc post")
}

fn tools_call_body(tool: &str, args: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{{"name":"{tool}","arguments":{args},"_meta":{{"{META_KEY}":"{PROTOCOL_VERSION}","io.modelcontextprotocol/clientCapabilities":{{}}}}}}}}"#
    )
}

/// 挑战形状断言（P23-a：Bearer 挑战 + resource_metadata 指向 PRM；
/// `expect_error_param` = 是否带 `error="invalid_token"`）。
fn assert_bearer_challenge(www: &str, label: &str, expect_error_param: bool) {
    assert!(www.starts_with("Bearer "), "{label}: {www}");
    assert!(www.contains("resource_metadata="), "{label}: {www}");
    assert!(www.contains(PROBE_PRM_URL), "{label}: {www}");
    assert_eq!(
        www.contains("error=\"invalid_token\""),
        expect_error_param,
        "{label}: {www}"
    );
}

/// token 校验矩阵（P23-a/c）：合法 token 放行至 rmcp；坏签 / 错 iss /
/// aud 错配（他 resource 签发——passthrough 禁止）/ 过期 / 未知 kid /
/// allowlist 外算法 / 缺 aud claim / 畸形 token / 任意 Bearer 值必 401，
/// 挑战 resource_metadata 指向 PRM。
#[tokio::test(flavor = "multi_thread")]
async fn authz_token_matrix_valid_passes_invalid_rows_401() {
    let as_ = spawn_mock_as().await;
    let addr = spawn_authed(&as_).await;
    let c = client();
    let mcp = format!("http://{addr}/mcp");

    // 合法 token → 放行至 rmcp（initialize 200 + JSON-RPC result）。
    let good = base_claims(&as_.issuer, PROBE_RESOURCE, "mcp:read", 600);
    let token = as_.issue(PROBE_KID, &as_.key, good.clone());
    let resp = rpc(
        &c,
        &mcp,
        Some(&token),
        Some(PROTOCOL_VERSION),
        INIT_BODY,
        &[],
    )
    .await;
    assert_eq!(resp.status(), 200, "valid token must reach rmcp");
    let body: serde_json::Value =
        serde_json::from_slice(&resp.bytes().await.expect("body")).expect("json");
    assert!(
        body.get("result").is_some(),
        "initialize must result: {body}"
    );

    // 无 token / 非 Bearer → 401 挑战（无 error 参数，T03 形状保留）。
    let resp = rpc(&c, &mcp, None, Some(PROTOCOL_VERSION), INIT_BODY, &[]).await;
    assert_eq!(resp.status(), 401, "missing token must 401");
    assert_bearer_challenge(
        resp.headers()
            .get("www-authenticate")
            .expect("challenge")
            .to_str()
            .expect("utf-8"),
        "missing token",
        false,
    );
    let resp = c
        .post(&mcp)
        .header("content-type", "application/json")
        .header("authorization", "Basic dXNlcjpwYXNz")
        .body(INIT_BODY)
        .send()
        .await
        .expect("post");
    assert_eq!(resp.status(), 401, "non-bearer scheme must 401");
    assert_bearer_challenge(
        resp.headers()
            .get("www-authenticate")
            .expect("challenge")
            .to_str()
            .expect("utf-8"),
        "non-bearer",
        false,
    );

    // 矩阵行（每行必 401 + error="invalid_token" + 挑战指 PRM）。
    let mut expired = good.clone();
    expired["exp"] = serde_json::json!(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_secs() as i64
            - 3600
    );
    let mut missing_aud = good.clone();
    missing_aud.as_object_mut().expect("object").remove("aud");
    let hs_key = EncodingKey::from_secret(b"wp05-t04-probe-hs-secret");
    let rows: Vec<(&str, String)> = vec![
        // 坏签：同 kid、未入 JWKS 的异钥签名。
        (
            "bad signature",
            as_.issue(PROBE_KID, &as_.alt_key, good.clone()),
        ),
        // 错 iss：未知 issuer（不做 JWKS 查找直接拒）。
        (
            "wrong issuer",
            as_.issue(
                PROBE_KID,
                &as_.key,
                base_claims("https://evil.example", PROBE_RESOURCE, "mcp:read", 600),
            ),
        ),
        // aud 错配 = 他 resource 签发 token（T-R1 passthrough 防线）。
        (
            "audience mismatch (other resource)",
            as_.issue(
                PROBE_KID,
                &as_.key,
                base_claims(&as_.issuer, "https://other.example", "mcp:read", 600),
            ),
        ),
        // 过期。
        ("expired", as_.issue(PROBE_KID, &as_.key, expired)),
        // 未知 kid（他 AS 轮换形态；miss 强刷一次仍无）。
        (
            "unknown kid",
            as_.issue("ghost-key", &as_.key, good.clone()),
        ),
        // allowlist 外算法（HS256 自签，即验签能力齐备也不放行）。
        ("algorithm not allowed", {
            let mut header = Header::new(Algorithm::HS256);
            header.kid = Some(PROBE_KID.to_string());
            encode(&header, &good, &hs_key).expect("sign hs256")
        }),
        // 缺 aud claim（required-claims fail-closed，防绕过受众绑定）。
        (
            "missing aud claim",
            as_.issue(PROBE_KID, &as_.key, missing_aud),
        ),
        // 畸形 token / 任意 Bearer 值（T03「任意 Bearer 必 401」在 Bearer
        // 墙下的演化行——非法 token 面必 401，断言不删）。
        ("malformed token", "not.a.jwt".to_string()),
        ("arbitrary bearer", "arbitrary-token".to_string()),
    ];
    for (label, row_token) in &rows {
        let resp = rpc(
            &c,
            &mcp,
            Some(row_token),
            Some(PROTOCOL_VERSION),
            INIT_BODY,
            &[],
        )
        .await;
        assert_eq!(resp.status(), 401, "{label} must 401");
        assert_bearer_challenge(
            resp.headers()
                .get("www-authenticate")
                .expect("challenge")
                .to_str()
                .expect("utf-8"),
            label,
            true,
        );
    }
}

/// scope gate（P23-b）：`tools/call` 所需 scope ∉ token scope 集 → 403 +
/// `error="insufficient_scope"`（单挑战含所需 scope + resource_metadata）；
/// 协议元面（initialize/tools/list）与 `mcp:read` 工具随 read-only token
/// 放行；全 scope token 达工具执行。
#[tokio::test(flavor = "multi_thread")]
async fn scope_gate_tools_call_step_up_403_metadata_face_passes() {
    let as_ = spawn_mock_as().await;
    let addr = spawn_authed(&as_).await;
    let c = client();
    let mcp = format!("http://{addr}/mcp");
    let read_only = as_.issue(
        PROBE_KID,
        &as_.key,
        base_claims(&as_.issuer, PROBE_RESOURCE, "mcp:read", 600),
    );
    let full = as_.issue(
        PROBE_KID,
        &as_.key,
        base_claims(
            &as_.issuer,
            PROBE_RESOURCE,
            "mcp:read mcp:write mcp:export",
            600,
        ),
    );

    // 协议元面随 read-only token（scope 校验只挂 tools/call，ADR-0031 决策 5）。
    let resp = rpc(
        &c,
        &mcp,
        Some(&read_only),
        Some(PROTOCOL_VERSION),
        INIT_BODY,
        &[],
    )
    .await;
    assert_eq!(
        resp.status(),
        200,
        "initialize must pass with read-only token"
    );
    let resp = rpc(
        &c,
        &mcp,
        Some(&read_only),
        Some(PROTOCOL_VERSION),
        TOOLS_LIST_BODY,
        &[("Mcp-Method", "tools/list")],
    )
    .await;
    assert_eq!(
        resp.status(),
        200,
        "tools/list must pass with read-only token"
    );

    // mcp:read 工具 → 放行至 rmcp。
    let resp = rpc(
        &c,
        &mcp,
        Some(&read_only),
        Some(PROTOCOL_VERSION),
        &tools_call_body("memory_search", "{}"),
        &[("Mcp-Method", "tools/call"), ("Mcp-Name", "memory_search")],
    )
    .await;
    assert_eq!(
        resp.status(),
        200,
        "read tool must reach rmcp with mcp:read"
    );

    // write / export / 未知工具（保守归高域）→ 403 step-up，单挑战含所需 scope。
    for (tool, required) in [
        ("memory_write", "mcp:write"),
        ("dataset_export", "mcp:export"),
        ("unknown_tool", "mcp:write"),
    ] {
        let resp = rpc(
            &c,
            &mcp,
            Some(&read_only),
            Some(PROTOCOL_VERSION),
            &tools_call_body(tool, "{}"),
            &[("Mcp-Method", "tools/call"), ("Mcp-Name", tool)],
        )
        .await;
        assert_eq!(resp.status(), 403, "{tool} must step-up 403");
        let www = resp
            .headers()
            .get("www-authenticate")
            .expect("challenge")
            .to_str()
            .expect("utf-8");
        assert!(www.starts_with("Bearer "), "{tool}: {www}");
        assert!(
            www.contains("error=\"insufficient_scope\""),
            "{tool}: {www}"
        );
        assert!(
            www.contains(&format!("scope=\"{required}\"")),
            "{tool} challenge must carry required scope: {www}"
        );
        assert!(www.contains("resource_metadata="), "{tool}: {www}");
    }

    // 全 scope token → 不再 403，到达工具真执行（memory_write 幂等语义的
    // 逐字段断言归 T05 e2e；本行钉 scope gate 放行 + 工具层受理 200）。
    let resp = rpc(
        &c,
        &mcp,
        Some(&full),
        Some(PROTOCOL_VERSION),
        &tools_call_body("memory_write", r#"{"content":"wp05-t04 scope gate probe"}"#),
        &[("Mcp-Method", "tools/call"), ("Mcp-Name", "memory_write")],
    )
    .await;
    assert_eq!(
        resp.status(),
        200,
        "full-scope token must reach tool execution"
    );
    let body: serde_json::Value =
        serde_json::from_slice(&resp.bytes().await.expect("body")).expect("json");
    assert!(
        body.get("result").is_some(),
        "memory_write must execute: {body}"
    );
}

/// G-3/G-6 协议与方法语义（SPEC §3.1 T04 行；rmcp 3.4.0 源码行级实证：
/// tower.rs -32020 header_mismatch → 400、METHOD_NOT_FOUND → 404、
/// server_side_http.rs accepted_response → 202 空体）。
#[tokio::test(flavor = "multi_thread")]
async fn protocol_header_and_rpc_semantics_g3_g6() {
    let as_ = spawn_mock_as().await;
    let addr = spawn_authed(&as_).await;
    let c = client();
    let mcp = format!("http://{addr}/mcp");
    let token = as_.issue(
        PROBE_KID,
        &as_.key,
        base_claims(&as_.issuer, PROBE_RESOURCE, "mcp:read", 600),
    );

    // G-3 缺头：非 initialize 请求（stateless_protocol_metadata_required）
    // 无 MCP-Protocol-Version（_meta 声明 2026-07-28 需头佐证）→ 400 + -32020。
    let resp = rpc(&c, &mcp, Some(&token), None, TOOLS_LIST_BODY, &[]).await;
    assert_eq!(resp.status(), 400, "missing protocol header must reject");
    let body: serde_json::Value =
        serde_json::from_slice(&resp.bytes().await.expect("body")).expect("json");
    assert_eq!(body["error"]["code"], -32020, "{body}");

    // G-3 头-体不一致（SEP-2243）：Mcp-Method 头 ≠ 体 method → 400 + -32020。
    let resp = rpc(
        &c,
        &mcp,
        Some(&token),
        Some(PROTOCOL_VERSION),
        TOOLS_LIST_BODY,
        &[("Mcp-Method", "tools/call")],
    )
    .await;
    assert_eq!(
        resp.status(),
        400,
        "Mcp-Method header/body mismatch must reject"
    );
    let body: serde_json::Value =
        serde_json::from_slice(&resp.bytes().await.expect("body")).expect("json");
    assert_eq!(body["error"]["code"], -32020, "{body}");

    // G-3 头-体不一致（initialize 专属规则）：头 2025-06-18 ≠
    // params.protocolVersion 2026-07-28 → 400 + -32600。
    let resp = rpc(&c, &mcp, Some(&token), Some("2025-06-18"), INIT_BODY, &[]).await;
    assert_eq!(
        resp.status(),
        400,
        "header/body version mismatch must reject"
    );
    let body: serde_json::Value =
        serde_json::from_slice(&resp.bytes().await.expect("body")).expect("json");
    assert_eq!(body["error"]["code"], -32600, "{body}");

    // G-6 未知方法（SEP-2243 头体一致放行到分派）→ 404 + JSON-RPC -32601。
    let unknown = format!(
        r#"{{"jsonrpc":"2.0","id":9,"method":"foo/bar","params":{{"_meta":{{"{META_KEY}":"{PROTOCOL_VERSION}","io.modelcontextprotocol/clientCapabilities":{{}}}}}}}}"#
    );
    let resp = rpc(
        &c,
        &mcp,
        Some(&token),
        Some(PROTOCOL_VERSION),
        &unknown,
        &[("Mcp-Method", "foo/bar")],
    )
    .await;
    assert_eq!(resp.status(), 404, "unknown method must 404");
    let body: serde_json::Value =
        serde_json::from_slice(&resp.bytes().await.expect("body")).expect("json");
    assert_eq!(body["error"]["code"], -32601, "{body}");

    // G-6 notification（无 id，Mcp-Method 一致）→ 202 + 空体。
    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#;
    let resp = rpc(
        &c,
        &mcp,
        Some(&token),
        Some(PROTOCOL_VERSION),
        notif,
        &[("Mcp-Method", "notifications/initialized")],
    )
    .await;
    assert_eq!(resp.status(), 202, "notification must be accepted");
    assert!(
        resp.bytes().await.expect("body").is_empty(),
        "notification response must have no body"
    );
}

/// bind 期 JWKS 预拉 fail-fast（T04 启动守卫）：配置的 JWKS 端点不可达 →
/// 显式拒绝启动，不留「第一个请求才发现」窗口。
#[tokio::test(flavor = "multi_thread")]
async fn bind_server_fails_fast_when_jwks_unreachable() {
    let config = McpRemoteConfig {
        auth: Some(AuthConfig {
            issuers: vec![TrustedIssuer {
                issuer: "https://as.example.test".to_string(),
                // discard 端口 9：connection refused。
                jwks_uri: "http://127.0.0.1:9/jwks.json".to_string(),
            }],
            ..AuthConfig::default()
        }),
        ..Default::default()
    };
    let err = match bind_server(&config, test_state().await).await {
        Ok(_) => panic!("unreachable JWKS must refuse startup"),
        Err(e) => e,
    };
    assert!(err.contains("JWKS 预拉失败"), "{err}");
}

/// 工具 → scope 全量映射（P23-b 载表，ADR-0031 决策 5 / 评估件 §3）：
/// 11 内建工具逐项 + ext_* 动态面与未知工具保守归 mcp:write。
#[test]
fn required_scope_mapping_covers_full_tool_surface() {
    for tool in [
        "asset_search",
        "asset_read",
        "job_status",
        "memory_search",
        "memory_verify",
        "ext_list",
    ] {
        assert_eq!(required_scope_for_tool(tool), "mcp:read", "{tool}");
    }
    for tool in [
        "asset_organize",
        "memory_write",
        "memory_update",
        "memory_delete",
        "ext_anything",
        "totally_unknown_tool",
    ] {
        assert_eq!(required_scope_for_tool(tool), "mcp:write", "{tool}");
    }
    assert_eq!(required_scope_for_tool("dataset_export"), "mcp:export");
}

/// 校验器配置 fail-closed：空 issuer 集 / 空 algorithm allowlist 拒绝构建。
#[tokio::test]
async fn token_validator_rejects_degenerate_auth_config() {
    let err = TokenValidator::new(&AuthConfig::default(), PROBE_RESOURCE.to_string())
        .expect_err("empty issuers must reject");
    assert!(err.contains("issuer 集为空"), "{err}");
    let cfg = AuthConfig {
        issuers: vec![TrustedIssuer {
            issuer: "https://as.example.test".to_string(),
            jwks_uri: "https://as.example.test/jwks.json".to_string(),
        }],
        algorithms: Vec::new(),
    };
    let err = TokenValidator::new(&cfg, PROBE_RESOURCE.to_string())
        .expect_err("empty algorithm allowlist must reject");
    assert!(err.contains("allowlist 为空"), "{err}");
}
