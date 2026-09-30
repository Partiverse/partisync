//! PartiFuse——FUSE 挂载面实现（ADR-0026，SPEC M8-WP01 一期）。
//!
//! 语义契约（[SEMANTICS.md](SEMANTICS.md)）：mountpoint-s3 式「诚实非
//! POSIX」——
//! - **随机读**：已存在文件任意 offset 读（`O_RDONLY`）
//! - **新文件顺序写**：`create`（O_CREAT|O_EXCL）→ 顺序追加写（offset 必须
//!   等于当前长度，否则 EINVAL）→ release 后不可再写
//! - **显式拒绝面**：unlink/rmdir/rename/mkdir/mknod/symlink/setattr→EPERM；
//!   已存在文件写打开→EACCES——**拒绝先于任何破坏性效果**（P15）
//!
//! 权限模型（ADR-0026 前置条件 1 处置）：**关闭 `DefaultPermissions`**，
//! 内核不做二次权限过滤，授权决策属 PartiSync 层；attr 统一报告挂载进程
//! uid/gid（见 [`fs::PartiFuse::new`]）。
//!
//! 审计追踪：ADR-0026（线位 fuser >=0.18.0,<0.19）；fuser API 经源码查证
//! （M7-WP03-T01 报告 §2），零凭记忆。
//!
//! 审计编号: SEC-无（无鉴权/密码学面；权限模型评审记录见 ADR-0026 §决策前置条件处置表）
//! 状态: Implemented 2026-09-30 by GLM-5.3-Flash (ZCode) (M8-WP01-T01)

mod fs;

pub use fs::PartiFuse;

/// 挂载入口（阻塞直到 umount）。`partifuse` bin 与 gateway 组装共用。
///
/// # Errors
/// 挂载失败（无 /dev/fuse、权限不足、mountpoint 非法）→ io::Error。
pub fn mount_blocking(fs: fs::PartiFuse, mountpoint: &std::path::Path) -> std::io::Result<()> {
    let mut cfg = fuser::Config::default();
    cfg.mount_options = vec![
        fuser::MountOption::FSName("partifuse".into()),
        fuser::MountOption::Subtype("partifuse".into()),
    ];
    // 注意：不启用 DefaultPermissions（权限模型见 crate doc）；
    // 不启用 AutoUnmount（避免依赖 fusermount 助手——pure 实现直走 mount(2)）。
    fuser::mount(fs, mountpoint, &cfg)
}

/// 后台挂载（探针/测试用）：立即返回 [`fuser::BackgroundSession`]。
///
/// # Errors
/// 同 [`mount_blocking`]。
pub fn spawn_mount(
    fs: fs::PartiFuse,
    mountpoint: &std::path::Path,
) -> std::io::Result<fuser::BackgroundSession> {
    let mut cfg = fuser::Config::default();
    cfg.mount_options = vec![fuser::MountOption::FSName("partifuse".into())];
    fuser::spawn_mount(fs, mountpoint, &cfg)
}
