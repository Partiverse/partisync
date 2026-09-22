//! MCP 协议层端到端测试（SPEC M4-WP03 验收「MCP stdio 传输可被 MCP client 调用」）。
//!
//! 真实 spawn `partisync-mcp` 子进程，经 stdio JSON-RPC 完整握手
//! （initialize → tools/list → tools/call），覆盖五大工具。
//!
//! 环境约束：numkong SVE/SME 探针在 Apple Clang 16 下编译崩溃，
//! 编译目标二进制需 `NK_TARGET_*=0`（与 CI 环境一致）。

use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::child_process::TokioChildProcess;
use serde_json::{json, Value};

/// spawn `partisync-mcp --db <内存准备好的库>`，完成 initialize 握手。
async fn spawn_server(db_path: &str) -> RunningService<RoleClient, ()> {
    let mut cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_partisync-mcp"));
    cmd.args(["--db", db_path]);
    // builder.spawn() 返回 (TokioChildProcess, Option<ChildStderr>)，取 transport 本体
    let (transport, _stderr) = TokioChildProcess::builder(cmd)
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn partisync-mcp");

    // () 有 blanket ClientHandler impl（rmcp handler/client.rs:296）
    let client: RunningService<RoleClient, ()> = rmcp::service::serve_client((), transport)
        .await
        .expect("initialize handshake");
    client
}

/// 建最小内存形态的 graph 库（schema 子集，与测试种子数据）。
async fn prepare_db() -> String {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("graph.db");
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&db_path)
                .create_if_missing(true),
        )
        .await
        .expect("open db");
    for ddl in [
        "CREATE TABLE content (id TEXT PRIMARY KEY, size INTEGER NOT NULL, mime TEXT, kind TEXT)",
        "CREATE TABLE entry (id TEXT PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL, \
         content_id TEXT, size INTEGER NOT NULL DEFAULT 0, mtime_ns INTEGER NOT NULL DEFAULT 0, \
         state INTEGER NOT NULL DEFAULT 0)",
        "CREATE TABLE tag (id TEXT PRIMARY KEY, space_id TEXT NOT NULL DEFAULT 'default', \
         name TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0)",
        "CREATE TABLE entry_tag (tag_id TEXT NOT NULL, entry_path TEXT NOT NULL, \
         deleted INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (tag_id, entry_path))",
        "CREATE TABLE sidecar_items (content_id TEXT NOT NULL, stage TEXT NOT NULL, \
         status INTEGER NOT NULL DEFAULT 0, detail TEXT, artifact TEXT, \
         updated_ns INTEGER NOT NULL, PRIMARY KEY (content_id, stage))",
        "CREATE TABLE jobs (id TEXT PRIMARY KEY, kind TEXT NOT NULL, status INTEGER NOT NULL, \
         root TEXT NOT NULL, checkpoint TEXT, done_files INTEGER NOT NULL DEFAULT 0, \
         error TEXT, created_ns INTEGER NOT NULL, updated_ns INTEGER NOT NULL)",
    ] {
        sqlx::query(ddl).execute(&pool).await.expect("ddl");
    }
    sqlx::query("INSERT INTO content (id, size, mime) VALUES ('c-e2e', 2048, 'image/jpeg')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO entry (id, name, path, content_id, size, mtime_ns) \
         VALUES ('e-e2e', 'contract_scan.jpg', '/INBOX/contract_scan.jpg', 'c-e2e', 2048, 42)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sidecar_items (content_id, stage, status, detail, updated_ns) VALUES \
         ('c-e2e','embed',2,'dim=768 model=bge-m3',1), ('c-e2e','ocr',2,NULL,1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO jobs (id, kind, status, root, checkpoint, done_files, created_ns, updated_ns) \
         VALUES ('j-e2e','sidecar',1,'c-e2e',NULL,2,10,20)",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
    // tempdir 泄漏保活（db 路径须在 server 进程存活期内有效）
    let leaked = dir.keep();
    leaked.join("graph.db").to_string_lossy().to_string()
}

/// 工具调用辅助：断言成功并取 structured_content。
async fn call_ok(client: &RunningService<RoleClient, ()>, name: &str, args: Value) -> Value {
    let params = CallToolRequestParams::new(name.to_string())
        .with_arguments(args.as_object().cloned().unwrap_or_default());
    let result: CallToolResult = client
        .peer()
        .call_tool(params)
        .await
        .expect("tools/call succeeds");
    assert_ne!(result.is_error, Some(true), "tool-level error: {result:?}");
    result.structured_content.expect("structured content")
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_stdio_full_chain() {
    let db = prepare_db().await;
    let client = spawn_server(&db).await;

    // tools/list：五大工具全部注册
    let tools = client.peer().list_all_tools().await.expect("tools/list");
    let mut names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "asset_organize",
            "asset_read",
            "asset_search",
            "dataset_export",
            "job_status"
        ]
    );

    // asset_read：协议往返 + 元数据断言
    let v = call_ok(&client, "asset_read", json!({"content_id": "c-e2e"})).await;
    assert_eq!(v["content_id"], "c-e2e");
    assert_eq!(v["name"], "contract_scan.jpg");
    assert_eq!(v["sidecar_stages"]["embed"], "done");

    // asset_search：索引未注入 → 空结果（非错误）
    let v = call_ok(
        &client,
        "asset_search",
        json!({"query": "contract", "limit": 5}),
    )
    .await;
    assert_eq!(v["total"], 0);

    // asset_organize：preview 不写库
    let v = call_ok(
        &client,
        "asset_organize",
        json!({
            "operations": [{"content_id": "c-e2e", "action": "add_tag", "value": "e2e-tag"}],
            "preview_only": true
        }),
    )
    .await;
    assert!(v["transaction_id"].is_null());

    // asset_organize：真实写入
    let v = call_ok(
        &client,
        "asset_organize",
        json!({
            "operations": [{"content_id": "c-e2e", "action": "add_tag", "value": "e2e-tag"}],
            "preview_only": false
        }),
    )
    .await;
    assert!(v["transaction_id"].is_string());

    // 写入经 read 可见（标签闭环）
    let v = call_ok(&client, "asset_read", json!({"content_id": "c-e2e"})).await;
    assert_eq!(v["tags"], json!(["e2e-tag"]));

    // dataset_export：JSONL 落盘
    let out_dir = std::env::temp_dir().join(format!("mcp_e2e_{}", uuid::Uuid::new_v4()));
    let v = call_ok(
        &client,
        "dataset_export",
        json!({
            "content_ids": ["c-e2e"],
            "format": "jsonl",
            "output_dir": out_dir.to_string_lossy()
        }),
    )
    .await;
    assert_eq!(v["record_count"], 1);
    let manifest = v["manifest_path"].as_str().expect("manifest path");
    let content = std::fs::read_to_string(manifest).expect("read manifest");
    assert!(content.contains("\"content_id\":\"c-e2e\""));
    let _ = std::fs::remove_dir_all(&out_dir);

    // job_status：状态映射
    let v = call_ok(&client, "job_status", json!({"job_id": "j-e2e"})).await;
    let jobs = v["jobs"].as_array().expect("jobs array");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["status"], "running");

    // 未知工具 → 协议错误（invalid_params）
    let err = client
        .peer()
        .call_tool(CallToolRequestParams::new("no_such_tool"))
        .await;
    assert!(err.is_err(), "unknown tool must be a protocol error");

    client.cancel().await.ok();
}
