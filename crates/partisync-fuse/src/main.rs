//! partifuse——PartiSync FUSE 挂载面 CLI（ADR-0026，M8-WP01-T01）。
//!
//! 用法：`partifuse mount <backing_dir> <mountpoint>`（前台阻塞直到 umount；
//! gateway 组装与探针走 lib API）。

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 || args[1] != "mount" {
        eprintln!("用法: partifuse mount <backing_dir> <mountpoint>");
        std::process::exit(2);
    }
    let backing = std::path::PathBuf::from(&args[2]);
    let mountpoint = std::path::PathBuf::from(&args[3]);
    eprintln!("partifuse: mounting {backing:?} at {mountpoint:?}（SEMANTICS.md 语义）");
    if let Err(e) =
        partisync_fuse::mount_blocking(partisync_fuse::PartiFuse::new(backing), &mountpoint)
    {
        eprintln!("mount 失败: {e}");
        std::process::exit(1);
    }
}
