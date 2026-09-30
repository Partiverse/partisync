//! FUSE 挂载面 spike 入口（SPEC M7-WP03 T01）：mountpoint-s3 语义验证载体。
//!
//! 用法：`partifuse-spike mount <backing_dir> <mountpoint>`
//! （前台阻塞直到 umount；自动化探针走 tests/probe_mount.rs）。

use fuser::{Config, MountOption};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 || args[1] != "mount" {
        eprintln!("用法: partifuse-spike mount <backing_dir> <mountpoint>");
        std::process::exit(2);
    }
    let backing = std::path::PathBuf::from(&args[2]);
    let mountpoint = std::path::PathBuf::from(&args[3]);
    let mut cfg = Config::default();
    cfg.mount_options = vec![
        MountOption::FSName("partifuse-spike".into()),
        MountOption::Subtype("partifuse".into()),
        MountOption::DefaultPermissions,
    ];
    eprintln!("partifuse-spike: mounting {backing:?} at {mountpoint:?}（SEMANTICS.md 语义）");
    if let Err(e) = fuser::mount(
        partisync_fuse_spike::PartiFuse::new(backing),
        &mountpoint,
        &cfg,
    ) {
        eprintln!("mount 失败: {e}");
        std::process::exit(1);
    }
}
