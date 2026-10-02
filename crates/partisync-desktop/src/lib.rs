//! PartiSync 桌面壳库（SPEC M6-WP03 / ADR-0024）。
//!
//! ## 模块
//! - [`error`]： IPC 错误码枚举 + 序列化形状 `{kind, msg}`
//! - [`state`]： `AppState`（Store + ChunkStore + IndexEngine OnceCell）
//! - [`ipc`]： 6 个 `#[tauri::command]`（T03 期）
//!
//! ## 二进制
//! `src/main.rs` 是 thin wrapper， 调 [`run`] 装载 Tauri Builder。
//!
//! ## 测试
//! `tests/commands.rs` 用 `tauri::test::mock_builder()` 注入 AppState，
//! 直接调 `ipc::*` 函数验证 6 个 command happy path 不 panic + 返回
//! 形状符合 SPEC §2.3。

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

pub mod error;
pub mod ipc;
pub mod mcp_sidecar;
pub mod state;
pub mod window_state;

use std::path::PathBuf;
use std::time::Instant;

use tauri::{Manager, WindowEvent};

use crate::state::AppState;
use crate::window_state::WindowStateStore;

/// CLI 参数解析（手写， 避免 clap 引入新顶层依赖）。
fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn default_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("partisync-desktop")
}

/// 启动 Tauri 桌面壳。
///
/// # Errors
/// `AppState::open` 失败 → 返回 `crate::error::DesktopError::Internal`。
///
/// # Bench 模式
/// `--bench-cold-start` 启用冷启动基准（SPEC M6-WP03 §3 T04）： `run()`
/// 入口记 `Instant::now()`， `on_page_load` 回调里把 elapsed_ms 打
/// stderr 一行 `__BENCH_READY__ <ms>` 后 `process::exit(0)`。 外部
/// `cargo xtask bench desktop-cold-start` 解析该行取统计。
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // SPEC §2.1： `--data-dir` 覆盖默认数据目录； db/cas/index 未显式给参时
    // 以 data-dir 为基（无 --data-dir 时与 T01 行为逐字节一致）。
    let data_dir = flag_value(&args, "--data-dir")
        .map(PathBuf::from)
        .unwrap_or_else(default_data_dir);
    let db = flag_value(&args, "--db")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("partisync.db"));
    let cas = flag_value(&args, "--cas")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("partisync.cas"));
    let index_root = flag_value(&args, "--index")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("index"));
    let bench_mode = has_flag(&args, "--bench-cold-start");
    let window_state_store = WindowStateStore::new(data_dir.join("window-state.json"));

    let app_state = tauri::async_runtime::block_on(AppState::open(
        db.clone(),
        cas.clone(),
        index_root.clone(),
    ))?;
    // run() 返回后的侧车兜底清理用（CloseRequested 钩子已杀则幂等 no-op）。
    let sidecar = app_state.mcp_sidecar.clone();

    let t0 = Instant::now();
    let mut builder = tauri::Builder::default()
        .manage(app_state)
        .manage(window_state_store)
        .invoke_handler(tauri::generate_handler![
            ipc::get_stats,
            ipc::list,
            ipc::search,
            ipc::search_hybrid,
            ipc::cas_stats,
            ipc::duplicates,
            ipc::jobs,
            ipc::mcp_call,
        ])
        .setup(|app| {
            // T06： 启动恢复上次窗口位置/尺寸（无状态文件 → 回落
            // tauri.conf.json 默认 1200×800 + center: true）。
            let saved = app.state::<WindowStateStore>().load();
            if let Some(win) = app.get_webview_window("main") {
                window_state::apply(&win, saved)?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if !matches!(event, WindowEvent::CloseRequested { .. }) {
                return;
            }
            // T06： 关闭时持久化位置/尺寸（全屏/退化几何由 capture 层跳过，
            // 不落盘 —— macOS 全屏关闭后重启恢复为非全屏）。
            if let Some(state) = window_state::capture(window) {
                let app = window.app_handle();
                if let Err(e) = app.state::<WindowStateStore>().save(&state) {
                    eprintln!("warn: save window state: {e}");
                }
            }
            // T05 遗留债（SPEC §3 T06 范围）： 窗口关闭时优雅终止
            // `partisync-mcp` 侧车子进程。
            let sidecar = window.app_handle().state::<AppState>().mcp_sidecar.clone();
            tauri::async_runtime::spawn(async move { sidecar.shutdown().await });
        });
    if bench_mode {
        // bench 模式: on_page_load 触发后打 ready 时间戳 + 自动退出,
        // 让 xtask bench desktop-cold-start 能 parse stderr 取冷启动耗时。
        builder = builder.on_page_load(move |_window, _payload| {
            eprintln!("__BENCH_READY__ {}", t0.elapsed().as_millis());
            std::process::exit(0);
        });
    }
    builder.run(tauri::generate_context!())?;
    // 兜底： Cmd+Q 等 app 级退出可能不逐窗触发 CloseRequested，
    // run 返回后再清一次侧车（幂等）。
    tauri::async_runtime::block_on(sidecar.shutdown());
    Ok(())
}
