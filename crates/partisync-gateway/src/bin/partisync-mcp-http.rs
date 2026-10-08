//! `partisync-mcp-http` —— PartiSync MCP 远程入口（Streamable HTTP + TLS +
//! 真实 OAuth 2.1 RS 授权，M10-WP05-T03/T04，SPEC docs/specs/M10-WP05.md
//! §2.2/§2.3，ADR-0031 决策 2/3/4）。
//!
//! 缺省 bind `127.0.0.1:8424`（回环可明文）；**非回环 bind 必须 TLS、显式
//! `--allow-host` 且显式授权配置**——启动守卫（[`check_startup`]）直接拒绝
//! 启动，不静默降级。授权面（T04）：`--issuer/--jwks-uri` 配置受信授权服
//! 务器后走 Bearer 验签（iss/aud(RFC 8707)/exp fail-closed，`aud` 取
//! `--resource`）；缺省 = fail-closed 墙态（除 PRM 外一切 401，无数据通路）。
//!
//! PRM 四字段缺省值用 `.invalid` 不可解析 mock 域（不冒充真实授权服务器）；
//! 公网部署必须 `--resource`/`--auth-server` 显式覆写。
//!
//! 用法：
//!
//! ```text
//! partisync-mcp-http [--bind <ip:port>] [--tls-cert <pem>] [--tls-key <pem>]
//!     [--allow-host <host>]... [--allow-origin <origin>]...
//!     [--resource <url>] [--auth-server <url>]... [--scope <scope>]...
//!     [--issuer <url> --jwks-uri <url>]...（成对，可多组）
//!     [--db <graph.db>] [--index-root <dir>]
//! ```

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use partisync_gateway::mcp::build_server_state;
use partisync_gateway::mcp_remote::{
    bind_server, AuthConfig, McpRemoteConfig, PrmConfig, TlsFiles, TrustedIssuer, DEFAULT_BIND_PORT,
};

fn next_arg(args: &mut impl Iterator<Item = String>, flag: &str) -> String {
    args.next().unwrap_or_else(|| {
        eprintln!("partisync-mcp-http: {flag} 需要一个值");
        std::process::exit(2);
    })
}

fn print_usage() {
    eprintln!(
        "用法: partisync-mcp-http [--bind <ip:port>] [--tls-cert <pem>] [--tls-key <pem>]\n\
         \x20 [--allow-host <host>]... [--allow-origin <origin>]...\n\
         \x20 [--resource <url>] [--auth-server <url>]... [--scope <scope>]...\n\
         \x20 [--issuer <url> --jwks-uri <url>]...（成对，可多组受信授权服务器）\n\
         \x20 [--db <graph.db>] [--index-root <目录>]\n\
         缺省: bind 127.0.0.1:{DEFAULT_BIND_PORT}；非回环 bind 必须 TLS + --allow-host + 授权配置（启动守卫）"
    );
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let mut bind: Option<String> = None;
    let mut tls_cert: Option<PathBuf> = None;
    let mut tls_key: Option<PathBuf> = None;
    let mut allow_hosts: Vec<String> = Vec::new();
    let mut allow_origins: Vec<String> = Vec::new();
    let mut resource: Option<String> = None;
    let mut auth_servers: Vec<String> = Vec::new();
    let mut scopes: Vec<String> = Vec::new();
    let mut issuer: Option<String> = None;
    let mut jwks_uri: Option<String> = None;
    let mut issuers: Vec<TrustedIssuer> = Vec::new();
    let mut db_path: Option<PathBuf> = None;
    let mut index_root: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => bind = Some(next_arg(&mut args, "--bind")),
            "--tls-cert" => tls_cert = Some(PathBuf::from(next_arg(&mut args, "--tls-cert"))),
            "--tls-key" => tls_key = Some(PathBuf::from(next_arg(&mut args, "--tls-key"))),
            "--allow-host" => allow_hosts.push(next_arg(&mut args, "--allow-host")),
            "--allow-origin" => allow_origins.push(next_arg(&mut args, "--allow-origin")),
            "--resource" => resource = Some(next_arg(&mut args, "--resource")),
            "--auth-server" => auth_servers.push(next_arg(&mut args, "--auth-server")),
            "--scope" => scopes.push(next_arg(&mut args, "--scope")),
            "--issuer" => issuer = Some(next_arg(&mut args, "--issuer")),
            "--jwks-uri" => jwks_uri = Some(next_arg(&mut args, "--jwks-uri")),
            "--db" => db_path = Some(PathBuf::from(next_arg(&mut args, "--db"))),
            "--index-root" => index_root = Some(PathBuf::from(next_arg(&mut args, "--index-root"))),
            "--help" | "-h" => {
                print_usage();
                return;
            }
            other => {
                eprintln!("partisync-mcp-http: 未知参数: {other}");
                print_usage();
                std::process::exit(2);
            }
        }
    }

    // 证书对必须成对给出（单边即配置错误，不做隐式补全）。
    let tls = match (tls_cert, tls_key) {
        (None, None) => None,
        (Some(cert_path), Some(key_path)) => Some(TlsFiles {
            cert_path,
            key_path,
        }),
        _ => {
            eprintln!("partisync-mcp-http: --tls-cert 与 --tls-key 必须成对给出");
            std::process::exit(2);
        }
    };

    // 受信 issuer 对必须成对给出（单边即配置错误）；可重复配置多组。
    match (issuer, jwks_uri) {
        (None, None) => {}
        (Some(iss), Some(uri)) => issuers.push(TrustedIssuer {
            issuer: iss,
            jwks_uri: uri,
        }),
        _ => {
            eprintln!("partisync-mcp-http: --issuer 与 --jwks-uri 必须成对给出");
            std::process::exit(2);
        }
    }

    let config = McpRemoteConfig {
        bind_addr: bind
            .as_deref()
            .map(|b| {
                b.parse().unwrap_or_else(|e| {
                    eprintln!("partisync-mcp-http: --bind {b} 不是合法地址（ip:port）: {e}");
                    std::process::exit(2);
                })
            })
            .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], DEFAULT_BIND_PORT))),
        tls,
        // 显式名单（含空列表覆写语义：--allow-host 一次未给 = None = 回环
        // 默认名单）；Origin 名单空 = 校验关（rmcp 默认语义，回环开发态）。
        allowed_hosts: (!allow_hosts.is_empty()).then_some(allow_hosts),
        allowed_origins: allow_origins,
        prm: PrmConfig {
            resource: resource.unwrap_or_else(|| PrmConfig::default().resource),
            authorization_servers: if auth_servers.is_empty() {
                PrmConfig::default().authorization_servers
            } else {
                auth_servers
            },
            scopes_supported: if scopes.is_empty() {
                PrmConfig::default().scopes_supported
            } else {
                scopes
            },
        },
        // 授权配置缺省 = fail-closed 墙态（AuthWall::FailClosedAll）。
        auth: (!issuers.is_empty()).then(|| AuthConfig {
            issuers,
            ..AuthConfig::default()
        }),
    };

    let state = match build_server_state(db_path, index_root).await {
        Ok(state) => state,
        Err(e) => {
            eprintln!("partisync-mcp-http: {e}");
            std::process::exit(1);
        }
    };

    let server = match bind_server(&config, Arc::new(state)).await {
        Ok(server) => server,
        Err(e) => {
            // 含启动守卫拒绝（非回环无 TLS / 无显式 allowed_hosts / 无授权
            // 配置）、JWKS 预拉失败与 TLS 装配、bind 失败——全部显式错误
            // 退出，不静默降级。
            eprintln!("partisync-mcp-http: {e}");
            std::process::exit(1);
        }
    };
    let addr = server
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let auth_mode = if config.auth.is_some() {
        "bearer 验签（fail-closed）"
    } else {
        "墙态：除 PRM 外一切请求 401（授权配置缺省）"
    };
    eprintln!(
        "partisync-mcp-http: listening on {addr} (tls={}，{auth_mode})",
        config.tls.is_some()
    );
    if let Err(e) = server.serve().await {
        eprintln!("partisync-mcp-http: {e}");
        std::process::exit(1);
    }
}
