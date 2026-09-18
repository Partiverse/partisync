//! PartiSync 命令行界面（SPEC M0-WP02 契约 §4）。
//!
//! 子命令（M0 范围，手写参数解析——避免为 2 个子命令引 clap）：
//!   partisync index <root> [--db <path>]                 索引目录入库
//!   partisync ui [--db <path>] [--addr 127.0.0.1:8080]   网页演示面
//!
//! 完整子命令矩阵（ls/find/dedupe/serve/sync）随 M0-WP06 与 M1/M2 落地。

mod web;

use std::path::PathBuf;

use partisync_cas::ChunkStore;
use partisync_core::error::PartisyError;
use partisync_graph::indexer::index_path;
use partisync_graph::store::Store;

const DEFAULT_DB: &str = "./partisync.db";
const DEFAULT_CAS: &str = "./partisync.cas";

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("index") => index_cmd(&args[1..]).await,
        Some("ui") => ui_cmd(&args[1..]).await,
        _ => {
            eprintln!(
                "partisync {}\n\n用法:\n  partisync index <root> [--db <path>] [--cas <dir>]\n  partisync ui [--db <path>] [--cas <dir>] [--addr 127.0.0.1:8080]",
                env!("CARGO_PKG_VERSION")
            );
            2
        }
    };
    std::process::exit(code);
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

async fn open_db(db: &str) -> Result<Store, PartisyError> {
    let store = Store::open(&PathBuf::from(db)).await?;
    store
        .seed_device_volume("device-local", "local", db)
        .await?;
    Ok(store)
}

async fn index_cmd(args: &[String]) -> i32 {
    let Some(root) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!("用法: partisync index <root> [--db <path>]");
        return 2;
    };
    let db = flag_value(args, "--db").unwrap_or_else(|| DEFAULT_DB.into());
    let cas_dir = flag_value(args, "--cas").unwrap_or_else(|| DEFAULT_CAS.into());
    let store = match open_db(&db).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };
    let cas = match ChunkStore::open(&PathBuf::from(&cas_dir)).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: 块库: {e}");
            return 1;
        }
    };
    match index_path(&store, Some(&cas), &PathBuf::from(root)).await {
        Ok(report) => {
            let stats = store
                .stats()
                .await
                .map(|s| {
                    println!(
                        "  图谱: {} 文件 / {} 目录 · 总量 {} · 唯一内容 {}（去重节省 {}）",
                        s.files,
                        s.dirs,
                        fmt_bytes(s.total_bytes),
                        s.unique_contents,
                        fmt_bytes(s.saved_bytes)
                    );
                })
                .map(|_| 0)
                .unwrap_or(1);
            println!(
                "index 完成: root={root} db={db} cas={cas_dir}\n  本次: 文件 {} 目录 {} 分块 {} 跳过符号链接 {}",
                report.files, report.dirs, report.chunked_files, report.skipped_symlinks
            );
            stats
        }
        Err(e) => {
            eprintln!("index 失败: {e}（severity={:?}）", e.severity);
            1
        }
    }
}

async fn ui_cmd(args: &[String]) -> i32 {
    let db = flag_value(args, "--db").unwrap_or_else(|| DEFAULT_DB.into());
    let cas_dir = flag_value(args, "--cas").unwrap_or_else(|| DEFAULT_CAS.into());
    let addr = flag_value(args, "--addr").unwrap_or_else(|| "127.0.0.1:8080".into());
    let store = match open_db(&db).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };
    let cas = match ChunkStore::open(&PathBuf::from(&cas_dir)).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: 块库: {e}");
            return 1;
        }
    };
    let app = web::router(store, cas, db.clone());
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: 绑定 {addr} 失败: {e}");
            return 1;
        }
    };
    println!("PartiSync 演示面: http://{addr}（db={db}；仅绑定本机——公网暴露需鉴权，SPEC 非目标）");
    match axum::serve(listener, app).await {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

pub(crate) fn fmt_bytes(n: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < 4 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}
