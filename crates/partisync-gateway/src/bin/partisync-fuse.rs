//! `partisync-fuse` —— PartiSync FUSE 挂载入口（ADR-0026，M8-WP01-T02）。
//!
//! 用法：`partisync-fuse mount <backing_dir> <mountpoint> [--cas <dir>]
//! [--graph <db>] [--peer <db>] [--tick-ms <n>]`
//!
//! 二期语义（SEMANTICS.md，M8-WP07）：一期只读 + 新文件顺序写 + 写回日志
//! 面（unlink/rmdir/rename/整文件替换）+ `--cas` 激活 by-hash 只读命名
//! 空间。`--graph`（M9-WP01-T01）激活写路径接线装配层：挂载写事件 →
//! graph/oplog，可选 `--peer` 驱动 bisync tick。前台阻塞直到 umount。
//! gateway 组装层——实现归 `partisync-fuse` crate（fuser 依赖不出其
//! 边界）/ gateway [`wiring`]（SPEC M9-WP01 §2）。

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut backing: Option<std::path::PathBuf> = None;
    let mut mountpoint: Option<std::path::PathBuf> = None;
    let mut cas_dir: Option<std::path::PathBuf> = None;
    let mut graph_db: Option<std::path::PathBuf> = None;
    let mut peer_db: Option<std::path::PathBuf> = None;
    let mut tick_ms = partisync_gateway::wiring::DEFAULT_TICK_MS;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--cas" if i + 1 < args.len() => {
                cas_dir = Some(std::path::PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--graph" if i + 1 < args.len() => {
                graph_db = Some(std::path::PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--peer" if i + 1 < args.len() => {
                peer_db = Some(std::path::PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--tick-ms" if i + 1 < args.len() => {
                tick_ms = args[i + 1].parse().unwrap_or_else(|_| {
                    eprintln!("--tick-ms 需要正整数");
                    std::process::exit(2);
                });
                i += 2;
            }
            _ if backing.is_none() => {
                backing = Some(std::path::PathBuf::from(&args[i]));
                i += 1;
            }
            _ if mountpoint.is_none() => {
                mountpoint = Some(std::path::PathBuf::from(&args[i]));
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    let (Some(backing), Some(mountpoint)) = (backing, mountpoint) else {
        eprintln!("用法: partisync-fuse mount <backing_dir> <mountpoint> [--cas <dir>] [--graph <db>] [--peer <db>] [--tick-ms <n>]");
        std::process::exit(2);
    };
    if !backing.is_dir() {
        eprintln!("backing 目录不存在: {}", backing.display());
        std::process::exit(2);
    }
    // CAS 装配（--cas 与 --graph 可叠加：by-hash 只读命名空间 + 写接线）
    let cas = cas_dir.map(|dir| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("CAS runtime");
        let cas = rt
            .block_on(partisync_cas::ChunkStore::open(&dir))
            .expect("打开 CAS");
        std::sync::Arc::new(cas)
    });
    eprintln!("partisync-fuse: mounting {backing:?} at {mountpoint:?}（SEMANTICS.md 语义，二期写回+by-hash）");
    // 装配层（--graph）：init（同根校验/播种/tick）必须先于挂载完成
    // （SPEC §2.1 校验失败拒绝启动）；随后 serve 事件循环随 runtime
    // worker 运行，主线程阻塞在 mount_blocking，umount 后统一关停。
    let rt = graph_db.as_ref().map(|_| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("wiring runtime")
    });
    let fuse = if let (Some(rt), Some(graph_db)) = (&rt, &graph_db) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let opts = partisync_gateway::wiring::WiringOpts {
            backing: backing.clone(),
            graph_db: graph_db.clone(),
            peer_db: peer_db.clone(),
            tick_ms,
        };
        let session = rt
            .block_on(partisync_gateway::wiring::WiringSession::init(&opts))
            .unwrap_or_else(|e| {
                eprintln!("partisync-fuse: 装配层初始化失败: {e}");
                std::process::exit(1);
            });
        rt.spawn(session.serve(rx));
        partisync_fuse::PartiFuse::with_wiring(backing.clone(), cas, tx)
    } else {
        match cas {
            Some(cas) => partisync_fuse::PartiFuse::with_cas(backing.clone(), Some(cas)),
            None => partisync_fuse::PartiFuse::new(backing.clone()),
        }
    };
    if let Err(e) = partisync_fuse::mount_blocking(fuse, &mountpoint) {
        eprintln!("partisync-fuse: mount 失败: {e}");
        std::process::exit(1);
    }
    if let Some(rt) = rt {
        rt.shutdown_timeout(std::time::Duration::from_millis(500));
    }
}
