//! PartiSync 桌面壳入口（Tauri 2， SPEC M6-WP03 / ADR-0024）。
//!
//! Thin wrapper： 调 [`partisync_desktop::run`] 装载 Tauri Builder。
//! 实际模块在 [`partisync_desktop`] lib crate（src/lib.rs）。

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    partisync_desktop::run().expect("error while running PartiSync desktop shell");
}
