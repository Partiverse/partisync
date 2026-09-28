//! xtask 基准模块（SPEC M6-WP03 §5: xtask/src/bench/）。
//!
//! 桌面壳冷启动基准 → [`desktop_cold_start::run`]。
//! 后续 task（M7+）可在此目录下追加新基准子模块，main.rs 注册为
//! `cargo xtask bench <name>` 子命令。

pub mod desktop_cold_start;

/// `cargo xtask bench <name>` 调度入口。返回 true = 成功。
pub fn dispatch(name: &str, args: &[String]) -> bool {
    match name {
        "desktop-cold-start" => desktop_cold_start::run(args),
        other => {
            eprintln!("unknown bench: {other}; available: desktop-cold-start");
            false
        }
    }
}
