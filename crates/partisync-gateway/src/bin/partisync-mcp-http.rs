//! `partisync-mcp-http` —— PartiSync MCP 远程入口（Streamable HTTP + TLS，
//! M10-WP05-T03，SPEC docs/specs/M10-WP05.md §2.2，ADR-0031 决策 2/4）。
//!
//! 缺省 bind `127.0.0.1:8424`（回环可明文）；**非回环 bind 必须 TLS 且显式
//! `--allow-host`**——启动守卫（[`check_startup`]）直接拒绝启动，不静默
//! 降级明文。T03 期为 fail-closed 默认态：除 PRM 公开发现面外一切请求 401
//! 挑战（授权面随 T04 开放，墙先于门）。
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
//!     [--db <graph.db>] [--index-root <dir>]
//! ```

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use partisync_gateway::mcp::build_server_state;
use partisync_gateway::mcp_remote::{
    bind_server, McpRemoteConfig, PrmConfig, TlsFiles, DEFAULT_BIND_PORT,
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
         \x20 [--db <graph.db>] [--index-root <目录>]\n\
         缺省: bind 127.0.0.1:{DEFAULT_BIND_PORT}；非回环 bind 必须 TLS + --allow-host（启动守卫）"
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
            // 含启动守卫拒绝（非回环无 TLS / 无显式 allowed_hosts）与
            // TLS 装配、bind 失败——全部显式错误退出，不静默降级。
            eprintln!("partisync-mcp-http: {e}");
            std::process::exit(1);
        }
    };
    let addr = server
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    eprintln!(
        "partisync-mcp-http: listening on {addr} (tls={}，fail-closed：除 PRM 外一切请求 401)",
        config.tls.is_some()
    );
    if let Err(e) = server.serve().await {
        eprintln!("partisync-mcp-http: {e}");
        std::process::exit(1);
    }
}
