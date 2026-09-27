//! PartiSync 桌面壳（Tauri 2， SPEC M6-WP03 / ADR-0024）
//!
//! 本文件仅承载最小 buildable scaffold： 一个 1200x800 主窗口装载
//! `ui/index.html`（M6-WP03-T01）。 后续任务（M6-WP03-T02 起）逐步
//! 引入 IPC commands（get_stats / list / search / cas_stats /
//! duplicates / jobs / mcp_call）、 MCP 侧车、 窗口状态记忆。

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    tauri::Builder::default()
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("error while running PartiSync desktop shell");
}
