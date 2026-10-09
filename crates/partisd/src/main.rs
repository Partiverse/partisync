//! partisd —— PartiSync 底座常驻守护进程（ADR-0032，M11-WP02-T04）。
//!
//! 组装 graph/CAS/index（索引只读，P24）并暴露 **loopback MCP 服务面**
//! ——与 `partisync-mcp`（stdio sidecar）同一套工具面（组装复用 gateway
//! `build_server_state`），即 partiverse V2/V3 集成接缝（S1）。
//!
//! 用法：
//!
//! ```text
//! partisd [--db <graph.db>] [--index-root <dir>] [--cas <dir>(预留)]
//!         [--listen <ip:port>] [--pid-file <path>]
//! ```
//!
//! - 默认 `--listen 127.0.0.1:7650`；**红线：仅环回**（ADR-0032 决策 5，
//!   非环回 bind 直接拒绝启动；远程/认证沿 M10-WP05 机制另线）。
//! - 生命周期：前台运行；`--pid-file`（默认 `partisd.pid`）独占创建防双
//!   实例（陈留 pid 经 /proc 存活检查后覆写）；SIGTERM/SIGINT 优雅退出
//!   （移除 pidfile，退出码 0）。
//!
//! 工具面与 stdio sidecar 一致：asset_search / asset_read / asset_organize
//! / dataset_export / job_status / memory_*（gateway `build_server_state`）。
//! 索引打开失败不致命——asset_search 降级为空（口径同 sidecar）。

use axum::Router;
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::tower::{
    StreamableHttpServerConfig, StreamableHttpService,
};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

const DEFAULT_LISTEN: &str = "127.0.0.1:7650";
const MCP_PATH: &str = "/mcp";

fn next_arg(args: &mut impl Iterator<Item = String>, flag: &str) -> String {
    args.next().unwrap_or_else(|| {
        eprintln!("partisd: {flag} 需要一个值");
        std::process::exit(2);
    })
}

fn print_usage() {
    eprintln!(
        "用法: partisd [--db <graph.db>] [--index-root <目录>] [--cas <目录>(预留)]\n\
         \x20 [--listen <ip:port>] [--pid-file <path>]\n\
         缺省: listen 127.0.0.1:7650（红线:仅环回，ADR-0032 决策 5）; pid-file partisd.pid"
    );
}

/// pidfile 独占创建防双实例；已存在时经 /proc 存活检查区分「在跑」与
/// 「陈留」（崩溃残留可覆写接管）。
fn acquire_pidfile(path: &Path) -> Result<(), String> {
    use std::io::Write;
    let write_pid = |p: &Path| -> Result<(), String> {
        let mut f =
            std::fs::File::create(p).map_err(|e| format!("写 pidfile {}: {e}", p.display()))?;
        writeln!(f, "{}", std::process::id())
            .map_err(|e| format!("写 pidfile {}: {e}", p.display()))
    };
    match std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
    {
        Ok(mut f) => writeln!(f, "{}", std::process::id())
            .map_err(|e| format!("写 pidfile {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let stale = std::fs::read_to_string(path)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            let alive = !stale.is_empty() && Path::new(&format!("/proc/{stale}")).exists();
            if alive {
                return Err(format!(
                    "另一 partisd 实例疑似在跑（pid {stale}，pidfile {}）。确认已停止后删除该文件重试",
                    path.display()
                ));
            }
            eprintln!("partisd: 陈留 pidfile（pid={stale:?} 已不存在），覆写接管");
            write_pid(path)
        }
        Err(e) => Err(format!("创建 pidfile {}: {e}", path.display())),
    }
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let mut db: Option<PathBuf> = None;
    let mut index_root: Option<PathBuf> = None;
    let mut cas: Option<PathBuf> = None;
    let mut listen: Option<String> = None;
    let mut pid_file: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => db = Some(PathBuf::from(next_arg(&mut args, "--db"))),
            "--index-root" => index_root = Some(PathBuf::from(next_arg(&mut args, "--index-root"))),
            "--cas" => cas = Some(PathBuf::from(next_arg(&mut args, "--cas"))),
            "--listen" => listen = Some(next_arg(&mut args, "--listen")),
            "--pid-file" => pid_file = Some(PathBuf::from(next_arg(&mut args, "--pid-file"))),
            "--help" | "-h" => {
                print_usage();
                return;
            }
            other => {
                eprintln!("partisd: 未知参数: {other}");
                print_usage();
                std::process::exit(2);
            }
        }
    }
    let _ = cas; // 预留：MCP 装配面从 db 路径约定派生 CAS；写者/作业面落地时接线

    let pid_path = pid_file.unwrap_or_else(|| PathBuf::from("partisd.pid"));
    if let Err(e) = acquire_pidfile(&pid_path) {
        eprintln!("partisd: {e}");
        std::process::exit(2);
    }

    let addr: SocketAddr = match listen.as_deref().unwrap_or(DEFAULT_LISTEN).parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!(
                "partisd: --listen {} 不是合法地址（ip:port）: {e}",
                listen.unwrap_or_default()
            );
            std::process::exit(2);
        }
    };
    // 红线（ADR-0032 决策 5）：仅环回。远程/认证沿 M10-WP05 机制另线。
    if !addr.ip().is_loopback() {
        eprintln!(
            "partisd: 拒绝启动——bind {} 非环回（ADR-0032 决策 5：partisd 面无鉴权，仅限 127.0.0.1 信任域）",
            addr
        );
        let _ = std::fs::remove_file(&pid_path);
        std::process::exit(2);
    }

    let state = std::sync::Arc::new(
        match partisync_gateway::mcp::build_server_state(db, index_root).await {
            Ok(state) => state,
            Err(e) => {
                eprintln!("partisd: {e}");
                let _ = std::fs::remove_file(&pid_path);
                std::process::exit(1);
            }
        },
    );

    let service = StreamableHttpService::new(
        {
            let state = std::sync::Arc::clone(&state);
            move || -> std::io::Result<std::sync::Arc<partisync_gateway::mcp::McpServerState>> {
                Ok(std::sync::Arc::clone(&state))
            }
        },
        std::sync::Arc::new(NeverSessionManager::default()),
        // 与远程面（mcp_remote.rs）同款 fail-closed 三件套；host/origin 名单
        // 留空 = 回环信任域默认（ADR-0032 决策 5）。
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true)
            .with_stateless_protocol_metadata_required(true),
    );
    let app: Router = Router::new().route_service(MCP_PATH, service);

    let tcp = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("partisd: bind {addr} 失败: {e}");
            let _ = std::fs::remove_file(&pid_path);
            std::process::exit(1);
        }
    };
    let bound = tcp
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| addr.to_string());
    eprintln!(
        "partisd: ready — pid={} http://{bound}{MCP_PATH} (loopback，无鉴权——ADR-0032 决策 5)",
        std::process::id()
    );

    let shutdown = async {
        let ctrl_c = tokio::signal::ctrl_c();
        #[cfg(unix)]
        {
            let mut term =
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("partisd: SIGTERM handler 装配失败: {e}");
                        return;
                    }
                };
            tokio::select! {
                _ = ctrl_c => { eprintln!("partisd: SIGINT"); }
                _ = term.recv() => { eprintln!("partisd: SIGTERM"); }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = ctrl_c.await;
        }
    };

    let serve = axum::serve(tcp, app).with_graceful_shutdown(shutdown);
    if let Err(e) = serve.await {
        eprintln!("partisd: {e}");
        let _ = std::fs::remove_file(&pid_path);
        std::process::exit(1);
    }
    let _ = std::fs::remove_file(&pid_path);
    eprintln!("partisd: clean exit");
    std::process::exit(0);
}
