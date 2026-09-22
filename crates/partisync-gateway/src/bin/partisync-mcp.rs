//! `partisync-mcp` —— PartiSync MCP Server 入口（RMCP 2026-07-28 stdio）。
//!
//! 用法：`partisync-mcp [--db <graph.db 路径>]`
//!
//! stdio JSON-RPC 传输，供 MCP 宿主（ZCode/Claude Desktop 等）以子进程方式
//! 拉起。`--db` 缺省 `~/.partisync/graph.db`。

#[tokio::main]
async fn main() {
    let mut db_path: Option<std::path::PathBuf> = None;
    let mut index_root: Option<std::path::PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => {
                db_path = args.next().map(std::path::PathBuf::from);
            }
            "--index-root" => {
                index_root = args.next().map(std::path::PathBuf::from);
            }
            "--help" | "-h" => {
                eprintln!("用法: partisync-mcp [--db <graph.db>] [--index-root <index 目录>]");
                return;
            }
            other => {
                eprintln!(
                    "未知参数: {other}（用法: partisync-mcp [--db <graph.db>] [--index-root <目录>]）"
                );
                std::process::exit(2);
            }
        }
    }

    if let Err(e) = partisync_gateway::mcp::run_mcp_server(db_path, index_root).await {
        // stderr 不污染 stdio JSON-RPC 通道
        eprintln!("partisync-mcp: {e}");
        std::process::exit(1);
    }
}
