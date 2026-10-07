//! M10-WP05-T03 远程 MCP 传输面全栈探针（SPEC docs/specs/M10-WP05.md §3.1
//! T03 行；hub 骨架探针判例 M9-WP05）。行为契约：PRM 公开面四字段、
//! fail-closed 默认态（任意 Bearer 必 401）、Origin/host 白名单先于授权面、
//! 非回环启动守卫矩阵、TLS 线位 roundtrip（自签证书到 PRM/墙）。

use std::net::SocketAddr;
use std::sync::Arc;

use partisync_gateway::mcp::McpServerState;
use partisync_gateway::mcp_remote::{
    bind_server, build_router, check_startup, McpRemoteConfig, PrmConfig, TlsFiles,
    PRM_WELLKNOWN_PATH,
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

/// 启动守卫矩阵：回环无条件放行；非回环必须 TLS，且必须显式 allowed_hosts。
#[test]
fn startup_guard_matrix_non_loopback_requires_tls_and_hosts() {
    let loopback: SocketAddr = "127.0.0.1:8424".parse().expect("addr");
    let non_loopback: SocketAddr = "192.0.2.1:8424".parse().expect("addr"); // TEST-NET-1，纯裁决不 bind
    let tls = TlsFiles {
        cert_path: "cert.pem".into(),
        key_path: "key.pem".into(),
    };

    // 回环：无 TLS 亦放行（本地开发默认态）。
    assert!(check_startup(loopback, None, None).is_ok());
    assert!(check_startup(loopback, Some(&tls), None).is_ok());

    // 非回环无 TLS → 拒绝，错误显式指明 TLS 前置条件。
    let err = check_startup(non_loopback, None, None).expect_err("must reject");
    assert!(err.contains("TLS"), "{err}");

    // 非回环 + TLS 但未显式 allowed_hosts → 拒绝（回环默认名单会 403 全拒）。
    let err = check_startup(non_loopback, Some(&tls), None).expect_err("must reject");
    assert!(err.contains("allowed_hosts"), "{err}");

    // 非回环 + TLS + 显式 allowed_hosts → 放行。
    let hosts = vec!["mcp.example.test".to_string()];
    assert!(check_startup(non_loopback, Some(&tls), Some(&hosts)).is_ok());
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
