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
    asset_detail, cas_stats, duplicates, get_stats, jobs, list, mcp_call, search, search_hybrid,
    sync_recent, sync_stats, DuplicatesArgs, ListArgs, McpCallArgs, SearchArgs, SyncRecentArgs,
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

    // 写 bash stub： 循环读 stdin， 按 method 应答——
    // initialize(id=0) → 协议握手响应； tools/call → 回显 result。
    // （McpSidecar 自 #37 后带 initialize handshake + 每请求 _meta，
    // stub 需模拟 MCP server 的最小生命周期。）
    let stub_path = tmp.path().join("stub.sh");
    std::fs::write(
        &stub_path,
        "#!/bin/sh\n\
         while read line; do\n\
         case \"$line\" in\n\
         *'\"method\":\"initialize\"'*) echo '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"serverInfo\":{\"name\":\"stub\",\"version\":\"0\"}}}' ;;\n\
         *'\"method\":\"tools/call\"'*) echo '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"ok\":true,\"echo\":\"stub\"}}' ;;\n\
         esac\n\
         done\n",
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

/// M7-WP01-T04：真 `partisync-mcp` 二进制的侧车全链路（desktop → 侧车 →
/// 扩展工具面）。要求 `target/../partisync-mcp` 已构建且
/// `~/.partisync/extensions/` 已装示例扩展（`scripts/install-demo-ext.sh`）；
/// 二者缺任一 → 跳过（e2e 性质，不阻塞离线单测）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_call_real_sidecar_ext_list() {
    let exe = std::env::current_exe().expect("current_exe");
    let bin = exe.parent().unwrap().join("../partisync-mcp");
    let ext_json = dirs::home_dir()
        .unwrap()
        .join(".partisync/extensions/demo_ext.json");
    if !bin.exists() || !ext_json.exists() {
        eprintln!("skip: sidecar binary or demo extension missing");
        return;
    }

    let tmp = TempDir::new().expect("tempdir");
    let db = tmp.path().join("sidecar-e2e.db");
    std::fs::write(&db, b"").expect("touch empty db (sqlite open needs the file)");

    let mut state = AppState::open(
        tmp.path().join("test.db"),
        tmp.path().join("cas"),
        tmp.path().join("index"),
    )
    .await
    .expect("open state");
    state.mcp_sidecar = std::sync::Arc::new(McpSidecar::new(
        bin.canonicalize().expect("canonicalize"),
        db,
        tmp.path().join("index"),
    ));

    let r = {
        let app = mock_builder()
            .manage(state)
            .build(mock_context(noop_assets()))
            .expect("build app");
        mcp_call(
            app.state::<AppState>(),
            McpCallArgs {
                tool: "ext_list".into(),
                args: json!({}),
            },
        )
        .await
        .expect("ext_list 经侧车成功")
    };

    let tools = r
        .get("structuredContent")
        .and_then(|sc| sc.get("tools"))
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        tools.iter().any(|t| t["name"] == "ext_demo_echo"),
        "ext_list 应列出 ext_demo_echo，得到：{r}"
    );
}

/// T06： 窗口状态持久化层在 mock 窗口上的冒烟（SPEC §3 T06）。
///
/// MockRuntime 的窗口 getter 恒返 0×0 几何、setter 是 no-op， 因此：
/// 1. `capture` 命中「退化尺寸不保存」守卫 → `None`（真窗口的全屏/
///    拖动行为归 macOS 手动验收）
/// 2. `apply(Some(state))` 不报错（setter no-op）
/// 3. 文件层 save→load 往返正确（逻辑坐标契约见 src/window_state.rs）
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn window_state_capture_guard_and_apply_smoke() {
    use partisync_desktop::window_state::{
        self, WindowState, WindowStateStore, DEFAULT_HEIGHT, DEFAULT_WIDTH,
    };
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    let (app, tmp) = make_app().await;
    let win = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
        .build()
        .expect("mock window");
    let store = WindowStateStore::new(tmp.path().join("window-state.json"));

    // mock 窗口几何为 0×0 → 守卫跳过， 不产生可持久化状态。
    assert_eq!(window_state::capture_webview(&win), None);
    // 保存过的状态 apply 回窗口（mock setter no-op）→ Ok。
    let state = WindowState {
        x: 100.0,
        y: 50.0,
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
    };
    store.save(&state).expect("save");
    window_state::apply(&win, store.load()).expect("apply");
}

// ===== M8-WP05-T01：语义检索旗舰（SPEC §2.1 + function-map §4-N1/N3） =====

fn state_dbg(app: &tauri::App<tauri::test::MockRuntime>) -> String {
    use tauri::Manager;
    format!("{:?}", app.state::<AppState>())
}

/// 冷启动不加载嵌入模型（§4-N1 硬线）：`AppState::open` 后 embedder
/// 未初始化；BM25 检索后仍未初始化——语义模型只在语义检索路径加载。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t01_embedder_not_loaded_on_startup_or_bm25() {
    let (app, _tmp) = make_app().await;
    let dbg = state_dbg(&app);
    assert!(
        dbg.contains("embedder_loaded: false"),
        "startup must not load embedder: {dbg}"
    );
    let _ = search(
        app.state::<AppState>(),
        SearchArgs {
            q: "anything".into(),
            limit: Some(5),
        },
    )
    .await
    .expect("bm25 search");
    let dbg = state_dbg(&app);
    assert!(
        dbg.contains("embedder_loaded: false"),
        "bm25 search must not load embedder: {dbg}"
    );
}

/// 空索引语义检索：返回空 hits（不 panic）且确实走了嵌入器路径
/// （§4-N1 反向断言：语义检索**必须**加载模型）。
///
/// **需下载 BGE-small-zh 权重**（秒级，缓存于 `index/embed-cache`）——
/// 沿 M5-WP05 `t03` 判例显式跑：
/// `cargo test -p partisync-desktop --test commands t01_hybrid -- --ignored --nocapture`。
/// CI 常绿的是 [`t01_embedder_not_loaded_on_startup_or_bm25`]（冷启动
/// 预算保护），本测补语义路径的端到端归一形状与真实命中。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "需下载嵌入模型权重：显式跑（-- --ignored --nocapture）"]
async fn t01_hybrid_empty_index_returns_no_hits_and_loads_embedder() {
    let (app, _tmp) = make_app().await;
    let hits = search_hybrid(
        app.state::<AppState>(),
        SearchArgs {
            q: "项目验收报告".into(),
            limit: Some(10),
        },
    )
    .await
    .expect("hybrid search on empty index");
    assert!(hits.is_empty(), "empty index must yield no hits");
    let dbg = state_dbg(&app);
    assert!(
        dbg.contains("embedder_loaded: true"),
        "hybrid search must load embedder: {dbg}"
    );
}

/// 详情 IPC（T02）：不存在 content_id → 空 copies（不 panic）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t02_asset_detail_empty_db_returns_empty_copies() {
    let (app, _tmp) = make_app().await;
    let d = asset_detail(
        app.state::<AppState>(),
        ListArgs {
            prefix: "deadbeef".into(),
        },
    )
    .await
    .expect("detail on empty db");
    assert!(d.copies.is_empty());
    assert_eq!(d.content_id, "deadbeef");
}

// ── M8-WP05-T03：同步状态界面（sync_stats / sync_recent） ──

/// 空库（device 表未登记）：统计全零 + 时间线空（不 panic）——SPEC §3 T03
/// 空态验收。`device_id()` 失败按捕获侧同款兜底 `device-local`。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_sync_stats_on_empty_db_returns_zeros() {
    let (app, _tmp) = make_app().await;
    let s = sync_stats(app.state::<AppState>())
        .await
        .expect("sync_stats on empty db");
    assert_eq!(s.applied, 0);
    assert_eq!(s.skipped_self, 0);
    assert_eq!(s.skipped_lww, 0, "LWW 落选行不入库，读侧恒 0");
    assert_eq!(s.conflicts, 0);
    assert_eq!(s.devices, 0);
    assert_eq!(s.last_sync_ns, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_sync_recent_on_empty_db_returns_empty() {
    let (app, _tmp) = make_app().await;
    let rows = sync_recent(app.state::<AppState>(), SyncRecentArgs { limit: Some(10) })
        .await
        .expect("sync_recent on empty db");
    assert!(rows.is_empty());
}

/// 种子同步持久态（M2 判例直写，不走 capture 写路径）：device 登记 +
/// oplog 两行（远端 origin / 本机 origin 各一）+ 冲突血缘一行。
async fn seed_sync_fixture(state: &AppState) {
    state
        .store
        .seed_device_volume("device-self", "本机", "vol-self")
        .await
        .expect("seed device");
    state
        .store
        .record_oplog(
            "default",
            0,
            "entry",
            "n1",
            "upsert",
            "device-remote",
            r#"{"path":"/Projects/aurora/报告.docx","name":"报告.docx","content_id":"9f3a2c1d"}"#,
        )
        .await
        .expect("seed oplog remote");
    state
        .store
        .record_oplog(
            "default",
            0,
            "entry",
            "n2",
            "remove",
            "device-self",
            r#"{"path":"/tmp/old.txt"}"#,
        )
        .await
        .expect("seed oplog self");
    state
        .store
        .record_conflict(
            "default",
            "/会议/notes.md",
            "/会议/notes.md",
            "/会议/notes.conflict-device-b.md",
            "device-b",
            "hlc-seed-1",
        )
        .await
        .expect("seed conflict");
    // F1 口径（M9-WP01-T04）：水位 = 持久 sync_watermark（push
    // note_applied 产物）；HLC 键 phys 段定宽 hex 毫秒（hlc.rs to_key）。
    state
        .store
        .note_applied(
            "device-remote",
            "0000018f5e8c4000-00000001-0000000000000042",
        )
        .await
        .expect("seed watermark");
}

/// F1 水位键 phys 段的纳秒期望值（0x18f5e8c4000 ms → ns）。
const SEED_WATERMARK_NS: i64 = 0x18f5e8c4000 * 1_000_000;

/// 种子库统计口径：applied 只数远端 origin；本机行归 skipped_self；
/// 冲突独立计数；devices = 远端 origin 去重。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_sync_stats_with_seeded_oplog_counts_by_origin() {
    let (app, _tmp) = make_app().await;
    let state = app.state::<AppState>();
    seed_sync_fixture(&state).await;
    let s = sync_stats(state).await.expect("sync_stats seeded");
    assert_eq!(s.applied, 1);
    assert_eq!(s.skipped_self, 1);
    assert_eq!(s.conflicts, 1);
    assert_eq!(s.devices, 1);
    assert_eq!(
        s.last_sync_ns,
        Some(SEED_WATERMARK_NS),
        "F1：水位 HLC phys 段换算"
    );
}

/// F1 回归（M9-WP01-T04）：push ACK trim 清空 pending_oplog 后，devices /
/// last_sync 不归零（改由持久 sync_watermark 派生——M8-WP05-ui-report D1）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t04_f1_sync_stats_survives_ack_trim() {
    let (app, _tmp) = make_app().await;
    let state = app.state::<AppState>();
    seed_sync_fixture(&state).await;
    // 模拟对端 ACK：按真实 pending 键全量裁剪
    let rows = state.store.pending_oplog().await.expect("pending");
    let keys: Vec<String> = rows.iter().map(|r| r.hlc.clone()).collect();
    assert!(!keys.is_empty(), "fixture 应有 pending 行");
    state.store.trim_oplog(&keys).await.expect("trim");
    let s = sync_stats(state).await.expect("sync_stats after trim");
    assert_eq!(s.applied, 0, "ACK 后 pending 面（applied）应清空");
    assert_eq!(s.devices, 1, "F1：ACK trim 后 devices 不得归零");
    assert_eq!(
        s.last_sync_ns,
        Some(SEED_WATERMARK_NS),
        "F1：ACK trim 后 last_sync 不得归零"
    );
}

/// 种子库时间线：oplog 尾部 + 冲突血缘合并，at_ns 降序；payload 解析出
/// name/dir/content_id；冲突行 op = "conflict"。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_sync_recent_with_seeded_rows_merges_timeline() {
    let (app, _tmp) = make_app().await;
    let state = app.state::<AppState>();
    seed_sync_fixture(&state).await;
    let rows = sync_recent(state, SyncRecentArgs { limit: Some(10) })
        .await
        .expect("sync_recent seeded");
    assert_eq!(rows.len(), 3);
    assert!(rows.windows(2).all(|w| w[0].at_ns >= w[1].at_ns), "降序");
    let conflict = rows.iter().find(|r| r.conflict).expect("conflict row");
    assert_eq!(conflict.op, "conflict");
    assert_eq!(conflict.name, "notes.md");
    assert_eq!(conflict.origin_device, "device-b");
    assert!(conflict.content_id.is_none());
    let upsert = rows.iter().find(|r| r.op == "upsert").expect("upsert row");
    assert_eq!(upsert.name, "报告.docx");
    assert_eq!(upsert.dir, "/Projects/aurora/");
    assert_eq!(upsert.content_id.as_deref(), Some("9f3a2c1d"));
    let remove = rows.iter().find(|r| r.op == "remove").expect("remove row");
    assert_eq!(remove.name, "old.txt");
}

/// 非法 payload JSON 兜底（AI 审查 F4）：不 panic，path/name 退化到
/// entity_id，dir 空，content_id None——单条脏数据不 fail 时间线。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_sync_recent_tolerates_invalid_payload_json() {
    let (app, _tmp) = make_app().await;
    let state = app.state::<AppState>();
    state
        .store
        .seed_device_volume("device-self", "本机", "vol-self")
        .await
        .expect("seed device");
    state
        .store
        .record_oplog(
            "default",
            0,
            "entry",
            "raw-entry-n5",
            "upsert",
            "device-remote",
            "not-json",
        )
        .await
        .expect("seed bad payload");
    let rows = sync_recent(state, SyncRecentArgs { limit: Some(10) })
        .await
        .expect("sync_recent with invalid payload");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "raw-entry-n5");
    assert_eq!(rows[0].dir, "");
    assert!(rows[0].content_id.is_none());
}

/// M9-WP02-T04： `memory_*` 工具经 `mcp_call` IPC 透传（桌面源码零改动
/// 判据——工具名是任意字符串参数， 三工具入 MCP 面即自动可达）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_call_memory_tool_passthrough_stub() {
    let tmp = TempDir::new().expect("tempdir");

    let stub_path = tmp.path().join("stub-memory.sh");
    std::fs::write(
        &stub_path,
        "#!/bin/sh\n\
         while read line; do\n\
         case \"$line\" in\n\
         *'\"method\":\"initialize\"'*) echo '{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"serverInfo\":{\"name\":\"stub\",\"version\":\"0\"}}}' ;;\n\
         *'\"method\":\"tools/call\"'*) echo '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"ok\":true,\"tool\":\"memory_write\",\"passthrough\":true}}' ;;\n\
         esac\n\
         done\n",
    )
    .expect("write stub");
    let mut perms = std::fs::metadata(&stub_path).expect("stat").permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
    }
    std::fs::set_permissions(&stub_path, perms).expect("chmod");

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

    let app = mock_builder()
        .manage(state)
        .build(mock_context(noop_assets()))
        .expect("build app");
    let result = mcp_call(
        app.state::<AppState>(),
        McpCallArgs {
            tool: "memory_write".into(),
            args: json!({"content": "desktop passthrough probe"}),
        },
    )
    .await
    .expect("mcp_call");
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["tool"], json!("memory_write"));
    assert_eq!(result["passthrough"], json!(true));
}
