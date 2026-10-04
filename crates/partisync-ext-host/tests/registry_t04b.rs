//! T04-B：`ExtTool` / `ExtRegistry` 高层封装测试（SPEC M7-WP01 §2.3 调用面）。
//!
//! fixture `demo_tool.wasm`（spike 产物，world `partisync:demo/demo-tool`，
//! 导出 `call(input: string) -> string`——MCP 工具语义约定的参照实现）。
//! 验证：装载拒绝路径（缺 manifest / 撞名 / preflight）、scan 目录约定、
//! JSON 进 JSON 出调用、F-2 错误路径下 Store 不毒化。

use std::io::Write;
use std::sync::Arc;

use partisync_ext_host::{ExtRegistry, HostState, IndexError, IndexRead, LoadError};

mod common;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

struct EchoIndex;

impl IndexRead for EchoIndex {
    fn search(&self, query: &str) -> Result<String, IndexError> {
        Ok(format!(r#"{{"echo":{query}}}"#))
    }
}

fn write_manifest(dir: &std::path::Path, stem: &str, body: &str) -> std::path::PathBuf {
    let p = dir.join(format!("{stem}.json"));
    let mut f = std::fs::File::create(&p).expect("create manifest");
    f.write_all(body.as_bytes()).expect("write manifest");
    p
}

#[test]
fn load_and_call_demo_tool() {
    // [P21] 起正路装载经测试钥锚（fixtures 签名钥；SPEC §4「测试钥签名
    // 路径」）——`load_with_anchors` 为 `load` 核心体，装载序一致。
    let tool = partisync_ext_host::registry::ExtTool::load_with_anchors(
        fixture("demo_tool.wasm"),
        write_manifest(
            &std::env::temp_dir(),
            "t04b_demo",
            r#"{"tool_name":"demo_ext","capabilities":[]}"#,
        ),
        HostState::without_index(),
        &common::test_anchor(),
    )
    .expect("demo_tool 装载成功");
    assert_eq!(tool.manifest().tool_name, "demo_ext");
    let out = tool.call(r#"{"k":"v"}"#).expect("demo tool call 成功");
    assert!(out.contains(r#""input_bytes":9"#), "demo 输出异常：{out}");
}

/// 缺 manifest → 装载期拒（[P14] IO 阶段）。
#[test]
fn load_rejects_missing_manifest() {
    let err = partisync_ext_host::registry::ExtTool::load(
        fixture("demo_tool.wasm"),
        std::env::temp_dir().join("t04b_nonexistent.json"),
        HostState::without_index(),
    )
    .expect_err("缺 manifest 必须装载期拒绝");
    assert!(matches!(err, LoadError::Manifest(_)));
}

/// F-4：声明 `index.read` 但组装期未接线 → preflight 装载期拒。
#[test]
fn load_rejects_unwired_index_read() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = write_manifest(
        dir.path(),
        "ext",
        r#"{"tool_name":"needs_index","capabilities":["index.read"]}"#,
    );
    // 验签（[P21]）位于 preflight 之前——签名须先通过方达 preflight 拒绝
    let err = partisync_ext_host::registry::ExtTool::load_with_anchors(
        fixture("demo_tool.wasm"),
        &manifest,
        HostState::without_index(),
        &common::test_anchor(),
    )
    .expect_err("未接线时 preflight 必须装载期拒绝");
    assert!(matches!(err, LoadError::Preflight(IndexError::NotWired)));
}

/// F-2 路径：声明 `index.read` + 已接线 → 装载成功（demo_tool 不调
/// index，但注入面 = 声明面）。
#[test]
fn load_accepts_wired_index_read() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = write_manifest(
        dir.path(),
        "ext",
        r#"{"tool_name":"uses_index","capabilities":["index.read"]}"#,
    );
    let tool = partisync_ext_host::registry::ExtTool::load_with_anchors(
        fixture("demo_tool.wasm"),
        &manifest,
        HostState::with_index(Arc::new(EchoIndex)),
        &common::test_anchor(),
    )
    .expect("已接线的 index.read 声明必须装载成功");
    let out = tool.call("x").expect("call 成功");
    assert!(out.contains(r#""input_bytes":1"#), "输出异常：{out}");
}

/// scan 目录约定：`*.json` manifest 驱动、同名 `.wasm`、工具名序。
#[test]
fn scan_directory_convention() {
    let dir = tempfile::tempdir().unwrap();
    // [P21] 起三文件配对：`.wasm` + `.json` + `.minisig`（签名随 fixture
    // 同步拷贝；scan 核心体经测试钥锚装载——SPEC §4「测试钥签名路径」）。
    std::fs::copy(fixture("demo_tool.wasm"), dir.path().join("alpha.wasm")).unwrap();
    std::fs::copy(
        fixture("demo_tool.minisig"),
        dir.path().join("alpha.minisig"),
    )
    .unwrap();
    std::fs::copy(fixture("demo_tool.wasm"), dir.path().join("beta.wasm")).unwrap();
    std::fs::copy(
        fixture("demo_tool.minisig"),
        dir.path().join("beta.minisig"),
    )
    .unwrap();
    write_manifest(
        dir.path(),
        "alpha",
        r#"{"tool_name":"alpha_tool","capabilities":[]}"#,
    );
    write_manifest(
        dir.path(),
        "beta",
        r#"{"tool_name":"beta_tool","capabilities":[]}"#,
    );

    let registry = ExtRegistry::scan_with_anchors(
        dir.path(),
        &HostState::without_index(),
        &common::test_anchor(),
    )
    .expect("scan 成功");
    assert_eq!(registry.len(), 2);
    let names: Vec<_> = registry
        .manifests()
        .iter()
        .map(|m| m.tool_name.as_str())
        .collect();
    assert_eq!(names, ["alpha_tool", "beta_tool"], "工具名序");

    // 按名调用
    let out = registry
        .call("beta_tool", r#"{"x":1}"#)
        .expect("已注册")
        .expect("调用成功");
    assert!(out.contains("input_bytes"), "输出异常：{out}");
    // 未注册 → None
    assert!(registry.call("nonexistent", "{}").is_none());
}

/// 同批扩展撞名 → 装载期显式拒（BTreeMap 静默覆盖的反面）。
#[test]
fn scan_rejects_duplicate_tool_names() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(fixture("demo_tool.wasm"), dir.path().join("a.wasm")).unwrap();
    std::fs::copy(fixture("demo_tool.minisig"), dir.path().join("a.minisig")).unwrap();
    std::fs::copy(fixture("demo_tool.wasm"), dir.path().join("b.wasm")).unwrap();
    std::fs::copy(fixture("demo_tool.minisig"), dir.path().join("b.minisig")).unwrap();
    write_manifest(dir.path(), "a", r#"{"tool_name":"same","capabilities":[]}"#);
    write_manifest(dir.path(), "b", r#"{"tool_name":"same","capabilities":[]}"#);

    let err = ExtRegistry::scan_with_anchors(
        dir.path(),
        &HostState::without_index(),
        &common::test_anchor(),
    )
    .expect_err("撞名必须装载期拒绝");
    assert!(matches!(err, LoadError::Duplicate(_)), "撞名错误：{err}");
}

/// scan 目录不存在 → fail-closed（不静默返回空注册表）。
#[test]
fn scan_missing_directory_fails() {
    let err = ExtRegistry::scan("/nonexistent/ext/dir", &HostState::without_index())
        .expect_err("目录不存在必须失败");
    assert!(matches!(err, LoadError::Scan(_)));
}
