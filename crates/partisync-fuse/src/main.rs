//! partifuse——PartiSync FUSE 挂载面 CLI（ADR-0026，M8-WP01-T01）。
//!
//! 用法：`partifuse mount <backing_dir> <mountpoint> [--cas <dir>]`
//! （前台阻塞直到 umount；gateway 组装与探针走 lib API）。
//! `--cas` 激活 by-hash 只读命名空间（M8-WP07-T04）。

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut backing: Option<std::path::PathBuf> = None;
    let mut mountpoint: Option<std::path::PathBuf> = None;
    let mut cas_dir: Option<std::path::PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--cas" if i + 1 < args.len() => {
                cas_dir = Some(std::path::PathBuf::from(&args[i + 1]));
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
        eprintln!("用法: partifuse mount <backing_dir> <mountpoint> [--cas <dir>]");
        std::process::exit(2);
    };
    eprintln!("partifuse: mounting {backing:?} at {mountpoint:?}（SEMANTICS.md 语义）");
    let fuse = match cas_dir {
        Some(dir) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("CAS runtime");
            let cas = rt
                .block_on(partisync_cas::ChunkStore::open(&dir))
                .expect("打开 CAS");
            partisync_fuse::PartiFuse::with_cas(backing, Some(std::sync::Arc::new(cas)))
        }
        None => partisync_fuse::PartiFuse::new(backing),
    };
    if let Err(e) = partisync_fuse::mount_blocking(fuse, &mountpoint) {
        eprintln!("mount 失败: {e}");
        std::process::exit(1);
    }
}
