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
use partisync_graph::indexer::index_path_job;
use partisync_graph::jobs::{self, JobCtx};
use partisync_graph::store::Store;
use partisync_graph::watch::{self, WatchConfig};

const DEFAULT_DB: &str = "./partisync.db";
const DEFAULT_CAS: &str = "./partisync.cas";

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("index") => index_cmd(&args[1..]).await,
        Some("ui") => ui_cmd(&args[1..]).await,
        Some("watch") => watch_cmd(&args[1..]).await,
        Some("resume") => resume_cmd(&args[1..]).await,
        Some("jobs") => jobs_cmd(&args[1..]).await,
        Some("ls") => ls_cmd(&args[1..]).await,
        Some("find") => find_cmd(&args[1..]).await,
        Some("dedupe") => dedupe_cmd(&args[1..]).await,
        _ => {
            eprintln!(
                "partisync {}\n\n用法:\n  partisync index <root> [--db <path>] [--cas <dir>]\n  partisync ui [--db <path>] [--cas <dir>] [--addr 127.0.0.1:8080]\n  partisync watch <root> [--db <path>] [--cas <dir>] [--debounce-ms 1000]\n  partisync resume [--job <id>]\n  partisync jobs\n  partisync ls <path> [--db <path>]\n  partisync find <q> [--db <path>]\n  partisync dedupe [--top N] [--db <path>]",
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
    // 作业化：index = 可恢复的持久作业（SPEC M0-WP05）
    let job_id = match jobs::create(&store, "index", root).await {
        Ok(id) => id,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };
    let _ = jobs::start(&store, &job_id).await;
    let mut ctx = JobCtx {
        id: job_id.clone(),
        skip_up_to: None,
        stop_after: None,
        done: 0,
    };
    let run_store = store.clone();
    let root_pb = PathBuf::from(&root);
    let run = tokio::task::spawn(async move {
        let res = index_path_job(&run_store, Some(&cas), &root_pb, Some(&mut ctx)).await;
        (res, ctx.done)
    });
    let result = tokio::select! {
        r = run => match r {
            Ok((inner, done)) => inner.map(|rep| (rep, done)),
            Err(e) => Err(PartisyError::with_source(
                partisync_core::error::Severity::Fatal,
                Box::new(e),
            )),
        },
        _ = tokio::signal::ctrl_c() => {
            println!("\nindex: 收到 Ctrl-C，进度已持久化（checkpoint 每 200 文件提交）");
            Err(PartisyError::new(partisync_core::error::Severity::Interrupted))
        }
    };
    match result {
        Ok((report, done)) => {
            let _ = jobs::complete(&store, &job_id, done).await;
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
                "index 完成: root={root} db={db} cas={cas_dir} job={job_id}\n  本次: 文件 {} 目录 {} 分块 {} 跳过符号链接 {}",
                report.files, report.dirs, report.chunked_files, report.skipped_symlinks
            );
            stats
        }
        Err(e) if e.severity == partisync_core::error::Severity::Interrupted => {
            let done = jobs::get(&store, &job_id)
                .await
                .map(|r| r.done_files)
                .unwrap_or(0);
            let _ = jobs::mark_interrupted(&store, &job_id, done as u64).await;
            eprintln!(
                "index 中断: 已提交 {done} 个文件（job={job_id}）；恢复: partisync resume --job {job_id}"
            );
            130
        }
        Err(e) => {
            let _ = jobs::fail(&store, &job_id, &e.to_string()).await;
            eprintln!("index 失败: {e}（severity={:?}）", e.severity);
            1
        }
    }
}

async fn resume_cmd(args: &[String]) -> i32 {
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
    let row = match flag_value(args, "--job") {
        Some(id) => jobs::get(&store, &id).await,
        None => jobs::latest_resumable(&store).await.and_then(|o| {
            o.map(Ok).unwrap_or_else(|| {
                Err(PartisyError {
                    severity: partisync_core::error::Severity::Fatal,
                    source: Some("无可恢复作业".into()),
                })
            })
        }),
    };
    let row = match row {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };
    if !matches!(row.status, 1 | 2) {
        eprintln!(
            "error: 作业 {} 状态为 {}，不可恢复",
            row.id, row.status_name
        );
        return 1;
    }
    println!(
        "resume: job={} checkpoint={:?}（已跳过 {} 文件）",
        row.id, row.checkpoint, row.done_files
    );
    jobs::start(&store, &row.id).await.unwrap();
    let mut ctx = JobCtx::for_resume(&row, None);
    let root = PathBuf::from(&row.root);
    match index_path_job(&store, Some(&cas), &root, Some(&mut ctx)).await {
        Ok(_report) => {
            let _ = jobs::complete(&store, &row.id, ctx.done).await;
            let s = store.stats().await.unwrap();
            println!(
                "resume 完成: 本次处理 {} 文件（续点前已有 {}）\n  图谱: {} 文件 / {} 目录 · 总量 {} · 去重节省 {}",
                ctx.done,
                row.done_files,
                s.files,
                s.dirs,
                fmt_bytes(s.total_bytes),
                fmt_bytes(s.saved_bytes)
            );
            0
        }
        Err(e) if e.severity == partisync_core::error::Severity::Interrupted => {
            let _ = jobs::mark_interrupted(&store, &row.id, ctx.done).await;
            eprintln!("resume 再次中断: {} 文件", ctx.done);
            130
        }
        Err(e) => {
            let _ = jobs::fail(&store, &row.id, &e.to_string()).await;
            eprintln!("resume 失败: {e}");
            1
        }
    }
}

async fn jobs_cmd(_args: &[String]) -> i32 {
    let db = flag_value(_args, "--db").unwrap_or_else(|| DEFAULT_DB.into());
    let store = match open_db(&db).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };
    match jobs::list(&store).await {
        Ok(rows) => {
            if rows.is_empty() {
                println!("（无作业）");
                return 0;
            }
            println!(
                "{:<27} {:<7} {:<11} {:>7}  checkpoint",
                "ID", "KIND", "STATUS", "DONE"
            );
            for r in rows {
                println!(
                    "{:<27} {:<7} {:<11} {:>7}  {:?}",
                    r.id, r.kind, r.status_name, r.done_files, r.checkpoint
                );
            }
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

async fn ls_cmd(args: &[String]) -> i32 {
    let Some(path) = args.first().filter(|a| !a.starts_with("--")).cloned() else {
        eprintln!("用法: partisync ls <path> [--db <path>]");
        return 2;
    };
    let db = flag_value(args, "--db").unwrap_or_else(|| DEFAULT_DB.into());
    let Ok(store) = open_db(&db).await else {
        return 1;
    };
    match store.children(&path).await {
        Ok(rows) => {
            if rows.is_empty() {
                println!("（空目录或路径不存在）");
                return 1;
            }
            println!("{:<8} {:>12}  CONTENT  NAME", "TYPE", "SIZE");
            for e in rows {
                let (t, size, ch) = if e.kind == 1 {
                    ("dir", "—".to_string(), "—".to_string())
                } else {
                    (
                        "file",
                        fmt_bytes(e.size),
                        e.content_id.map_or("—".into(), |c| c[..8].to_string()),
                    )
                };
                println!("{:<8} {:>12}  {:<8}  {}", t, size, ch, e.name);
            }
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

async fn find_cmd(args: &[String]) -> i32 {
    let Some(q) = args.first().filter(|a| !a.starts_with("--")).cloned() else {
        eprintln!("用法: partisync find <q> [--db <path>]");
        return 2;
    };
    let db = flag_value(args, "--db").unwrap_or_else(|| DEFAULT_DB.into());
    let Ok(store) = open_db(&db).await else {
        return 1;
    };
    match store.search(&q, 200).await {
        Ok(rows) => {
            let n = rows.len();
            for e in &rows {
                println!("{:>12}  {}", fmt_bytes(e.size), e.path);
            }
            println!("（{n} 条，上限 200）");
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

async fn dedupe_cmd(args: &[String]) -> i32 {
    let db = flag_value(args, "--db").unwrap_or_else(|| DEFAULT_DB.into());
    let top = flag_value(args, "--top")
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    let Ok(store) = open_db(&db).await else {
        return 1;
    };
    match store.duplicates(top).await {
        Ok(groups) => {
            let saved = store.stats().await.map(|s| s.saved_bytes).unwrap_or(0);
            for g in &groups {
                println!(
                    "{} × {}  [{}]",
                    g.copies.len(),
                    fmt_bytes(g.size),
                    &g.content_id[..12]
                );
                for c in &g.copies {
                    println!("    {}", c.path);
                }
            }
            println!(
                "── 共 {} 组（显示前 {top}）；文件级去重节省 {}",
                groups.len(),
                fmt_bytes(saved)
            );
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

async fn watch_cmd(args: &[String]) -> i32 {
    let Some(root) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!("用法: partisync watch <root> [--db <path>] [--cas <dir>] [--debounce-ms 1000]");
        return 2;
    };
    let db = flag_value(args, "--db").unwrap_or_else(|| DEFAULT_DB.into());
    let cas_dir = flag_value(args, "--cas").unwrap_or_else(|| DEFAULT_CAS.into());
    let debounce = std::time::Duration::from_millis(
        flag_value(args, "--debounce-ms")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1000),
    );
    // macOS /tmp 为符号链接：FSEvents 上报规范路径，必须先规范化（否则事件被
    // to_vpath 静默丢弃——本次实测踩坑）
    let root = PathBuf::from(root)
        .canonicalize()
        .map_err(|e| {
            eprintln!("error: 解析路径: {e}");
            std::process::exit(1);
        })
        .unwrap();
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
    // 幂等全量索引打底，再进入事件循环
    if let Err(e) = index_path_job(&store, Some(&cas), &root, None).await {
        eprintln!("error: 初始索引: {e}");
        return 1;
    }
    println!(
        "watch: {}（去抖 {}ms；Ctrl-C 退出）",
        root.display(),
        debounce.as_millis()
    );
    // notify 回调线程；Ctrl-C 终止任务——未冲刷的 fs 事件不落 journal，
    // 但下次 watch 的幂等全量索引会收敛（P8：fs 是事实源）
    let watch_root = root.clone();
    let mut watcher_task = tokio::task::spawn_blocking(move || {
        watch::run_blocking(store, Some(cas), watch_root, WatchConfig { debounce })
    });
    tokio::select! {
        _ = tokio::signal::ctrl_c() => println!("\nwatch: 收到 Ctrl-C，退出"),
        res = &mut watcher_task => match res {
            Ok(Ok(())) => println!("watch: 事件源断开，退出"),
            Ok(Err(e)) => eprintln!("error: {e}"),
            Err(e) => eprintln!("error: {e}"),
        },
    }
    watcher_task.abort();
    0
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
