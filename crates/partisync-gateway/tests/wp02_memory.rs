//! M9-WP02-T04 工具面探针（SPEC M9-WP02 §2.4/§3）：真实 spawn
//! `partisync-mcp` 子进程，经 stdio JSON-RPC 完整握手——tools/list
//! 三工具 schema 契约断言 + 三工具全功能调用 + 限界拒绝。
//!
//! M10-WP04-T02 延伸：memory_update/memory_delete 生命周期工具面
//! （SPEC M10-WP04 §2.1/§3-T02）——schema 逐字段断言 + 三工具全功能
//! e2e + 限界拒绝矩阵 + instructions 透出。
//!
//! 沿 mcp_e2e.rs 判例（CARGO_BIN_EXE 同 crate bin；tempdir keep 泄漏保活）。

use partisync_graph::Store;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::child_process::TokioChildProcess;
use serde_json::{json, Value};

async fn spawn_server(db_path: &str) -> RunningService<RoleClient, ()> {
    let mut cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_partisync-mcp"));
    cmd.args(["--db", db_path]);
    let (transport, _stderr) = TokioChildProcess::builder(cmd)
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn partisync-mcp");
    let client: RunningService<RoleClient, ()> = rmcp::service::serve_client((), transport)
        .await
        .expect("initialize handshake");
    client
}

/// 真实 graph 库（Store 全 schema 迁移 + device 播种——memory_write 依赖
/// device 表 origin）。server 侧 Store::from_pool 幂等重跑迁移，不冲突。
async fn prepared_db() -> String {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("graph.db");
    let store = Store::open(&db_path).await.expect("open graph db");
    store
        .seed_device_volume("dev-a", "Device A", "fp-a")
        .await
        .expect("seed device");
    drop(store);
    dir.keep().join("graph.db").to_string_lossy().to_string()
}

async fn call(client: &RunningService<RoleClient, ()>, name: &str, args: Value) -> Value {
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

async fn call_is_tool_error(client: &RunningService<RoleClient, ()>, name: &str, args: Value) {
    let params = CallToolRequestParams::new(name.to_string())
        .with_arguments(args.as_object().cloned().unwrap_or_default());
    let result: CallToolResult = client
        .peer()
        .call_tool(params)
        .await
        .expect("tools/call succeeds (tool-level error)");
    assert_eq!(
        result.is_error,
        Some(true),
        "expected tool error: {result:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn wp02_memory_tools_list_and_call_contract() {
    let db = prepared_db().await;
    let client = spawn_server(&db).await;

    // ── tools/list 契约（schema 与 SPEC §2.4 一致；唯一权威契约面）──
    let tools = client.peer().list_all_tools().await.expect("tools/list");
    for n in ["memory_write", "memory_search", "memory_verify"] {
        assert!(
            tools.iter().any(|t| t.name.as_ref() == n),
            "missing tool {n}"
        );
    }
    let mw = tools
        .iter()
        .find(|t| t.name.as_ref() == "memory_write")
        .expect("memory_write schema");
    assert_eq!(mw.input_schema["required"], json!(["content"]));
    assert_eq!(
        mw.input_schema["properties"]["content"]["type"],
        json!("string")
    );
    assert_eq!(
        mw.input_schema["properties"]["tags"]["type"],
        json!("array")
    );
    assert_eq!(
        mw.input_schema["properties"]["metadata"]["type"],
        json!("object")
    );
    let ms = tools
        .iter()
        .find(|t| t.name.as_ref() == "memory_search")
        .expect("memory_search schema");
    for k in ["query", "tag", "memory_id", "limit", "offset"] {
        assert!(
            ms.input_schema["properties"].get(k).is_some(),
            "missing {k}"
        );
    }
    let mv = tools
        .iter()
        .find(|t| t.name.as_ref() == "memory_verify")
        .expect("memory_verify schema");
    assert!(mv.input_schema["properties"].get("memory_id").is_some());
    assert!(mv.input_schema.get("required").is_none(), "memory_id 可选");

    // ── memory_write：写入 + 幂等 + 根返回 ──
    let fact1 = "partisync memory write probe fact one";
    let fact2 = "partisync memory write probe fact two 不同事实";
    let v = call(
        &client,
        "memory_write",
        json!({"content": fact1, "tags": ["probe"], "metadata": {"k": "v"}}),
    )
    .await;
    let id1 = v["memory_id"].as_str().expect("memory_id").to_string();
    assert_eq!(v["deduplicated"], json!(false));
    assert!(
        v["root"].as_str().is_some_and(|r| r.len() == 64),
        "root hex"
    );
    let v = call(
        &client,
        "memory_write",
        json!({"content": fact2, "tags": ["probe"]}),
    )
    .await;
    let id2 = v["memory_id"].as_str().expect("memory_id").to_string();
    assert_ne!(id1, id2);
    let v = call(
        &client,
        "memory_write",
        json!({"content": fact1, "tags": ["probe"], "metadata": {"k": "v"}}),
    )
    .await;
    assert_eq!(v["memory_id"], json!(id1), "内容寻址同 id");
    assert_eq!(v["deduplicated"], json!(true), "同身份二次写幂等");

    // ── memory_search：FTS 路径（≥3 字符）+ tag 精确 + id 精确 + 空结果 ──
    let v = call(&client, "memory_search", json!({"query": "probe fact"})).await;
    assert_eq!(v["total"], json!(2));
    let v = call(
        &client,
        "memory_search",
        json!({"query": "fact two", "tag": "probe"}),
    )
    .await;
    assert_eq!(v["total"], json!(1));
    assert_eq!(v["results"][0]["memory_id"], json!(id2));
    let v = call(
        &client,
        "memory_search",
        json!({"query": "fact two", "tag": "nope"}),
    )
    .await;
    assert_eq!(v["total"], json!(0), "tag 精确过滤不命中");
    let v = call(&client, "memory_search", json!({"memory_id": id1})).await;
    assert_eq!(v["total"], json!(1));
    assert_eq!(v["results"][0]["content"], json!(fact1));
    let v = call(
        &client,
        "memory_search",
        json!({"query": "absent content xyz"}),
    )
    .await;
    assert_eq!(v["total"], json!(0));

    // ── memory_verify：全库 + 单叶包含证明 ──
    let v = call(&client, "memory_verify", json!({})).await;
    assert_eq!(v["ok"], json!(true));
    assert_eq!(v["memory_count"], json!(2));
    assert_eq!(v["recomputed_root"], json!(v["root"]), "快照根 = 重算根");
    let v = call(&client, "memory_verify", json!({"memory_id": id1})).await;
    assert_eq!(v["ok"], json!(true));
    assert_eq!(v["memory_id"], json!(id1));
    assert_eq!(
        v["audit_path"].as_array().expect("audit path").len(),
        1,
        "n=2 路径长 1"
    );

    // ── 限界与缺失（工具级错误：不截断、不静默）──
    call_is_tool_error(&client, "memory_write", json!({"content": ""})).await;
    call_is_tool_error(
        &client,
        "memory_write",
        json!({"content": "x".repeat(64 * 1024 + 1)}),
    )
    .await;
    call_is_tool_error(
        &client,
        "memory_write",
        json!({"content": "ok", "tags": vec!["t"; 33]}),
    )
    .await;
    call_is_tool_error(
        &client,
        "memory_write",
        json!({"content": "ok", "metadata": "not-object"}),
    )
    .await;
    call_is_tool_error(&client, "memory_verify", json!({"memory_id": "no-such-id"})).await;
}

/// M10-WP04-T02（SPEC M10-WP04 §2.1/§3-T02）：memory_update / memory_delete
/// 工具面契约——tools/list schema 逐字段断言、三工具全功能 e2e（update 换
/// 身份墓碑旧 id / 软删不动根 / verify deleted+tombstones 透出 / canonical
/// 等价 no-op / 复活经 memory_write）、限界拒绝矩阵、instructions 透出。
#[tokio::test(flavor = "multi_thread")]
async fn wp04_memory_lifecycle_tools_contract() {
    let db = prepared_db().await;
    let client = spawn_server(&db).await;

    // ── tools/list schema 契约（逐字段；schema = 对外唯一权威契约面）──
    let tools = client.peer().list_all_tools().await.expect("tools/list");
    let mu = tools
        .iter()
        .find(|t| t.name.as_ref() == "memory_update")
        .expect("memory_update 入 tools/list");
    assert_eq!(mu.input_schema["type"], json!("object"));
    assert_eq!(mu.input_schema["required"], json!(["memory_id"]));
    assert_eq!(
        mu.input_schema["properties"]["memory_id"]["type"],
        json!("string")
    );
    assert_eq!(
        mu.input_schema["properties"]["content"]["type"],
        json!("string")
    );
    assert_eq!(
        mu.input_schema["properties"]["tags"]["type"],
        json!("array")
    );
    assert_eq!(
        mu.input_schema["properties"]["tags"]["items"]["type"],
        json!("string")
    );
    assert_eq!(
        mu.input_schema["properties"]["metadata"]["type"],
        json!("object")
    );
    let md = tools
        .iter()
        .find(|t| t.name.as_ref() == "memory_delete")
        .expect("memory_delete 入 tools/list");
    assert_eq!(md.input_schema["type"], json!("object"));
    assert_eq!(md.input_schema["required"], json!(["memory_id"]));
    assert_eq!(
        md.input_schema["properties"]["memory_id"]["type"],
        json!("string")
    );

    // ── instructions 透出（get_info 文本同步两工具）──
    let info = client.peer_info().expect("initialize peer info");
    let instructions = info.instructions.as_deref().unwrap_or_default();
    assert!(
        instructions.contains("memory_update") && instructions.contains("memory_delete"),
        "instructions 未透出生命周期工具: {instructions}"
    );

    // ── 全功能 e2e：write → update（换身份墓碑旧 id）──
    let fact1 = "partisync wp04 lifecycle probe fact one";
    let fact2 = "partisync wp04 lifecycle probe fact two";
    let fact3 = "partisync wp04 lifecycle probe fact three 已更新";
    let v = call(
        &client,
        "memory_write",
        json!({"content": fact1, "tags": ["probe"], "metadata": {"k": "v"}}),
    )
    .await;
    let id1 = v["memory_id"].as_str().expect("memory_id").to_string();
    let v = call(&client, "memory_write", json!({"content": fact2})).await;
    let id2 = v["memory_id"].as_str().expect("memory_id").to_string();

    // update：缺省 tags/metadata 回落当前行 → 新 id = identity(fact3, 行 tags, 行 metadata)
    let v = call(
        &client,
        "memory_update",
        json!({"memory_id": id1, "content": fact3}),
    )
    .await;
    assert_eq!(v["old_memory_id"], json!(id1));
    let id3 = v["memory_id"].as_str().expect("memory_id").to_string();
    assert_ne!(id3, id1, "id 内容寻址 ⇒ 更新必换身份");
    assert_eq!(v["tombstoned"], json!(true), "真更新墓碑旧 id");
    let root_after_update = v["root"].as_str().expect("root hex").to_string();
    assert_eq!(root_after_update.len(), 64, "root 定宽 hex");

    // 旧 id search 三路径不见（FTS query / id 精确查）；新 id 可检索
    let v = call(&client, "memory_search", json!({"query": "probe fact one"})).await;
    assert_eq!(v["total"], json!(0), "旧 id 内容 search 不见");
    let v = call(&client, "memory_search", json!({"memory_id": id1})).await;
    assert_eq!(v["total"], json!(0), "旧 id 精确查不见");
    let v = call(&client, "memory_search", json!({"query": "fact three"})).await;
    assert_eq!(v["total"], json!(1));
    assert_eq!(v["results"][0]["memory_id"], json!(id3));

    // memory_verify：旧 id 墓碑 proof 仍过 + deleted=true（实态旗标，T01 报告面）
    let v = call(&client, "memory_verify", json!({"memory_id": id1})).await;
    assert_eq!(v["ok"], json!(true), "墓碑行 inclusion proof 仍通过");
    assert_eq!(v["deleted"], json!(true));

    // 全库 verify：tombstones 计数透出；memory_count = 含墓碑承诺集
    let v = call(&client, "memory_verify", json!({})).await;
    assert_eq!(v["ok"], json!(true));
    assert_eq!(v["tombstones"], json!(1));
    assert_eq!(v["memory_count"], json!(3), "承诺集 = 含墓碑全体行");

    // canonical 等价 no-op：显式同 content（tags/metadata 缺省回落）→ 原 id
    let v = call(
        &client,
        "memory_update",
        json!({"memory_id": id3, "content": fact3}),
    )
    .await;
    assert_eq!(v["memory_id"], json!(id3), "canonical 等价 no-op 返回原 id");
    assert_eq!(v["tombstoned"], json!(false), "no-op 未墓碑任何行");
    assert_eq!(v["root"], json!(root_after_update), "no-op 不动根");
    // 全缺省 update = 等价 no-op
    let v = call(&client, "memory_update", json!({"memory_id": id3})).await;
    assert_eq!(v["memory_id"], json!(id3));
    assert_eq!(v["tombstoned"], json!(false));

    // ── memory_delete：软删不动根 + 墓碑 proof 仍过 ──
    let v = call(&client, "memory_delete", json!({"memory_id": id2})).await;
    assert_eq!(v["memory_id"], json!(id2));
    assert_eq!(v["tombstoned"], json!(true));
    assert_eq!(
        v["root"],
        json!(root_after_update),
        "软删不动根（墓碑留承诺集）"
    );
    let v = call(&client, "memory_search", json!({"query": "probe fact two"})).await;
    assert_eq!(v["total"], json!(0), "墓碑 id search 不见");
    let v = call(&client, "memory_verify", json!({"memory_id": id2})).await;
    assert_eq!(v["ok"], json!(true), "墓碑行 proof 仍过");
    assert_eq!(v["deleted"], json!(true));
    let v = call(&client, "memory_verify", json!({})).await;
    assert_eq!(v["tombstones"], json!(2));
    assert_eq!(v["root"], json!(root_after_update), "全库根 = 删除前根");

    // ── 复活语义经工具面：delete 后同参数 memory_write → deduplicated=false ──
    let v = call(&client, "memory_write", json!({"content": fact2})).await;
    assert_eq!(v["memory_id"], json!(id2), "同身份命中墓碑行复活");
    assert_eq!(v["deduplicated"], json!(false), "复活非幂等命中");
    let v = call(&client, "memory_verify", json!({"memory_id": id2})).await;
    assert_eq!(v["deleted"], json!(false), "复活后墓碑清位");
    let v = call(&client, "memory_search", json!({"query": "probe fact two"})).await;
    assert_eq!(v["total"], json!(1), "复活后 search 可见");

    // ── 限界拒绝矩阵（工具级错误：不截断、不静默）──
    // memory_update：空 content / >64KiB / tags>32 项 / metadata 非 object
    call_is_tool_error(
        &client,
        "memory_update",
        json!({"memory_id": id3, "content": ""}),
    )
    .await;
    call_is_tool_error(
        &client,
        "memory_update",
        json!({"memory_id": id3, "content": "x".repeat(64 * 1024 + 1)}),
    )
    .await;
    call_is_tool_error(
        &client,
        "memory_update",
        json!({"memory_id": id3, "tags": vec!["t"; 33]}),
    )
    .await;
    call_is_tool_error(
        &client,
        "memory_update",
        json!({"memory_id": id3, "metadata": "not-object"}),
    )
    .await;
    // memory_update：不存在 id / 已墓碑 id（id1 由 update 墓碑）
    call_is_tool_error(
        &client,
        "memory_update",
        json!({"memory_id": "no-such-id", "content": "x"}),
    )
    .await;
    call_is_tool_error(
        &client,
        "memory_update",
        json!({"memory_id": id1, "content": "x"}),
    )
    .await;
    // memory_delete：不存在 id / 已墓碑 id（对 id1 二次删）
    call_is_tool_error(&client, "memory_delete", json!({"memory_id": "no-such-id"})).await;
    call_is_tool_error(&client, "memory_delete", json!({"memory_id": id1})).await;
}
