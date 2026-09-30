//! `partisync-fuse` —— PartiSync FUSE 挂载入口（ADR-0026，M8-WP01-T02）。
//!
//! 用法：`partisync-fuse mount <backing_dir> <mountpoint>`
//!
//! 一期语义（SEMANTICS.md）：只读 + 新文件顺序写；拒绝面显式 EPERM/EACCES
//! （P15）。前台阻塞直到 umount。gateway 组装层——实现归 `partisync-fuse`
//! crate（fuser 依赖不出其边界）。

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 || args[1] != "mount" {
        eprintln!("用法: partisync-fuse mount <backing_dir> <mountpoint>");
        std::process::exit(2);
    }
    let backing = std::path::PathBuf::from(&args[2]);
    let mountpoint = std::path::PathBuf::from(&args[3]);
    if !backing.is_dir() {
        eprintln!("backing 目录不存在: {}", backing.display());
        std::process::exit(2);
    }
    eprintln!("partisync-fuse: mounting {backing:?} at {mountpoint:?}（SEMANTICS.md 语义，一期只读+新文件顺序写）");
    if let Err(e) =
        partisync_fuse::mount_blocking(partisync_fuse::PartiFuse::new(backing), &mountpoint)
    {
        eprintln!("partisync-fuse: mount 失败: {e}");
        std::process::exit(1);
    }
}
