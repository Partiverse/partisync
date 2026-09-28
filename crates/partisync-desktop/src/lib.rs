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
pub mod state;

use std::path::PathBuf;

use crate::state::AppState;

/// CLI 参数解析（手写， 避免 clap 引入新顶层依赖）。
fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

fn default_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("partisync-desktop")
}

fn default_db() -> PathBuf {
    default_data_dir().join("partisync.db")
}

fn default_cas() -> PathBuf {
    default_data_dir().join("partisync.cas")
}

fn default_index() -> PathBuf {
    default_data_dir().join("index")
}

/// 启动 Tauri 桌面壳。
///
/// # Errors
/// `AppState::open` 失败 → 返回 `crate::error::DesktopError::Internal`。
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let db = flag_value(&args, "--db")
        .map(PathBuf::from)
        .unwrap_or_else(default_db);
    let cas = flag_value(&args, "--cas")
        .map(PathBuf::from)
        .unwrap_or_else(default_cas);
    let index_root = flag_value(&args, "--index")
        .map(PathBuf::from)
        .unwrap_or_else(default_index);

    let app_state = tauri::async_runtime::block_on(AppState::open(
        db.clone(),
        cas.clone(),
        index_root.clone(),
    ))?;

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            ipc::get_stats,
            ipc::list,
            ipc::search,
            ipc::cas_stats,
            ipc::duplicates,
            ipc::jobs,
        ])
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())?;
    Ok(())
}
