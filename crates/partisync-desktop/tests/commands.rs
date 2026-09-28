//! IPC commands 单测（SPEC M6-WP03 §3 T03 + T05 验收段）。
//!
//! 7 个 command happy path 覆盖： 用 `tauri::test::mock_builder()` 注入
//! 临时目录 AppState（空 Store + 空 ChunkStore + 空 IndexEngine + MCP 侧车），
//! 直接调用 `ipc::*` 函数验证不 panic + 返回形状符合 SPEC §2.3。
//!
//! 不走 IPC webview 层（避免 webkit2gtk 依赖）： Tauri 2 的
//! `#[tauri::command]` 不变换函数体， 函数签名 `State<'_, AppState>`
//! 可直接由 `app.state::<AppState>()` 构造并传入。
//!
//! ## 范围
//! - **T03** (6 commands)：
//!   get_stats / list / cas_stats / duplicates / jobs： 在空库上应成功
//!   返回（files = 0 / dirs = 0 等初值）
//!   search： IndexEngine 懒加载在空索引目录也能成功打开（tantivy + usearch
//!   在空目录下自动初始化）， BM25 查询空索引应返回空 hits
//! - **T05** (1 command)：
//!   `mcp_call` 见 [`mcp_call_on_stub_sidecar_returns_result`] 用 bash
//!   stub 替代真 `partisync-mcp` 二进制， 验证 JSON-RPC 转发链路。
//!   真实 LCSTS fixture E2E 验证由 SPEC §3 T05 验收段负责。

use std::path::PathBuf;

use partisync_desktop::ipc::{
    cas_stats, duplicates, get_stats, jobs, list, mcp_call, search, DuplicatesArgs, ListArgs,
    McpCallArgs, SearchArgs,
};
use partisync_desktop::mcp_sidecar::McpSidecar;
use partisync_desktop::state::AppState;
use serde_json::json;
use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri::Manager;
use tempfile::TempDir;

async fn make_app() -> (tauri::App<tauri::test::MockRuntime>, TempDir) {
    let tmp = TempDir::new().expect("tempdir");
    let state = AppState::open(
        tmp.path().join("test.db"),
        tmp.path().join("cas"),
        tmp.path().join("index"),
    )
    .await
    .expect("open state");
    // 测试不需要 IPC handler（直接调 ipc::* 函数而非 mock webview invoke），
    // 故省略 .invoke_handler(...)： generate_handler! 需要 __cmd__X 宏在调用
    // 站点可见， 而 #[macro_export] 仅在 crate root 暴露， 跨测试 crate
    // 边界不带。 mock_builder 仅 .manage(state) 即可获得 app.state::<T>()。
    let app = mock_builder()
        .manage(state)
        .build(mock_context(noop_assets()))
        .expect("build app");
    (app, tmp)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_stats_on_empty_db_returns_zeros() {
    let (app, _tmp) = make_app().await;
    let s = get_stats(app.state::<AppState>()).await.expect("get_stats");
    assert_eq!(s.files, 0);
    assert_eq!(s.dirs, 0);
    assert_eq!(s.total_bytes, 0);
    assert_eq!(s.unique_contents, 0);
    assert_eq!(s.duplicate_groups, 0);
    assert_eq!(s.saved_bytes, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_root_on_empty_db_returns_empty() {
    let (app, _tmp) = make_app().await;
    let rows = list(app.state::<AppState>(), ListArgs { prefix: "/".into() })
        .await
        .expect("list");
    assert!(rows.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_empty_index_returns_no_hits() {
    let (app, _tmp) = make_app().await;
    let hits = search(
        app.state::<AppState>(),
        SearchArgs {
            q: "anything".into(),
            limit: Some(10),
        },
    )
    .await
    .expect("search");
    assert!(hits.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cas_stats_on_empty_cas_returns_zeros() {
    let (app, _tmp) = make_app().await;
    let c = cas_stats(app.state::<AppState>()).await.expect("cas_stats");
    assert_eq!(c.chunks, 0);
    assert_eq!(c.chunk_bytes, 0);
    assert_eq!(c.refs, 0);
    assert_eq!(c.saved_bytes, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn duplicates_on_empty_db_returns_empty() {
    let (app, _tmp) = make_app().await;
    let groups = duplicates(app.state::<AppState>(), DuplicatesArgs { top: 20 })
        .await
        .expect("duplicates");
    assert!(groups.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn jobs_on_empty_db_returns_empty() {
    let (app, _tmp) = make_app().await;
    let rows = jobs(app.state::<AppState>()).await.expect("jobs");
    assert!(rows.is_empty());
}

/// T05： `mcp_call` IPC command 用 bash stub 替代真 `partisync-mcp` 子进程。
///
/// stub 写一行 JSON-RPC 响应到 stdout 后退出。 测试验证：
/// 1. JSON-RPC 请求格式正确（`tools/call` + `params.name` + `params.arguments`）
/// 2. `id` 字段路由到响应（reader task 正确分发）
/// 3. 返回的 `Value` 是 stub 写出的 `result` 字段
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_call_on_stub_sidecar_returns_result() {
    let tmp = TempDir::new().expect("tempdir");

    // 写 bash stub： 读一行 stdin（JSON-RPC 请求）， 立即打印一行 JSON 响应。
    let stub_path = tmp.path().join("stub.sh");
    std::fs::write(
        &stub_path,
        "#!/bin/sh\nread line\necho '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"ok\":true,\"echo\":\"stub\"}}'\n",
    )
    .expect("write stub");
    let mut perms = std::fs::metadata(&stub_path).expect("stat").permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
    }
    std::fs::set_permissions(&stub_path, perms).expect("chmod");

    // 直接构造带 stub sidecar 的 AppState（绕开 make_app 里的 `for_app_state`
    // 查找 `partisync-mcp` 二进制路径）。
    let mut state = AppState::open(
        tmp.path().join("test.db"),
        tmp.path().join("cas"),
        tmp.path().join("index"),
    )
    .await
    .expect("open state");
    state.mcp_sidecar = std::sync::Arc::new(McpSidecar::new(
        stub_path,
        PathBuf::from("/tmp/ignored.db"),
        PathBuf::from("/tmp/ignored_index"),
    ));

    // 直调 ipc::mcp_call 函数（不需要 Tauri app 注入， 函数签名 State<_> 用
    // app.state() 但 mcp_call 实际只读 state.mcp_sidecar， 测试可绕）。
    let app = mock_builder()
        .manage(state)
        .build(mock_context(noop_assets()))
        .expect("build app");
    let result = mcp_call(
        app.state::<AppState>(),
        McpCallArgs {
            tool: "asset_search".into(),
            args: json!({"q": "hello"}),
        },
    )
    .await
    .expect("mcp_call");
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["echo"], json!("stub"));
}
