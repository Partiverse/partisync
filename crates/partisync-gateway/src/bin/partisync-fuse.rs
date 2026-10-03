//! `partisync-fuse` —— PartiSync FUSE 挂载入口（ADR-0026，M8-WP01-T02）。
//!
//! 用法：`partisync-fuse mount <backing_dir> <mountpoint> [--cas <dir>]`
//!
//! 二期语义（SEMANTICS.md，M8-WP07）：一期只读 + 新文件顺序写 + 写回日志
//! 面（unlink/rmdir/rename/整文件替换）+ `--cas` 激活 by-hash 只读命名
//! 空间。前台阻塞直到 umount。gateway 组装层——实现归 `partisync-fuse`
//! crate（fuser 依赖不出其边界）。

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
        eprintln!("用法: partisync-fuse mount <backing_dir> <mountpoint> [--cas <dir>]");
        std::process::exit(2);
    };
    if !backing.is_dir() {
        eprintln!("backing 目录不存在: {}", backing.display());
        std::process::exit(2);
    }
    eprintln!("partisync-fuse: mounting {backing:?} at {mountpoint:?}（SEMANTICS.md 语义，二期写回+by-hash）");
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
        eprintln!("partisync-fuse: mount 失败: {e}");
        std::process::exit(1);
    }
}
