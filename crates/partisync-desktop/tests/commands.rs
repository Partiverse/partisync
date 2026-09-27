//! IPC commands 单测（SPEC M6-WP03 §3 T03 验收段）。
//!
//! 6 个 command happy path 覆盖： 用 `tauri::test::mock_builder()` 注入
//! 临时目录 AppState（空 Store + 空 ChunkStore + 空 IndexEngine），
//! 直接调用 `ipc::*` 函数验证不 panic + 返回形状符合 SPEC §2.3。
//!
//! 不走 IPC webview 层（避免 webkit2gtk 依赖）： Tauri 2 的
//! `#[tauri::command]` 不变换函数体， 函数签名 `State<'_, AppState>`
//! 可直接由 `app.state::<AppState>()` 构造并传入。
//!
//! ## 范围（T03）
//! - get_stats / list / cas_stats / duplicates / jobs： 在空库上应成功
//!   返回（files = 0 / dirs = 0 等初值）
//! - search： IndexEngine 懒加载在空索引目录也能成功打开（tantivy + usearch
//!   在空目录下自动初始化）， BM25 查询空索引应返回空 hits

use partisync_desktop::ipc::{
    cas_stats, duplicates, get_stats, jobs, list, search, DuplicatesArgs, ListArgs, SearchArgs,
};
use partisync_desktop::state::AppState;
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
