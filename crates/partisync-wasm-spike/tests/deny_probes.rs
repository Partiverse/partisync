//! Property test [P13]（docs/tests/properties.md）：未授予能力（capability）的
//! wasm component 调用 FS/网络/时钟 API 必须失败（默认拒权），且失败信息
//! 不泄露宿主路径/环境。SPEC M6-WP04 §2.3「沙箱默认面」/ §2.4。

use partisync_wasm_spike::host;
use wasmtime::Store;

/// 探针 guest（assets/probe_deny.wasm）在零 import linker 下实例化必须失败，
/// 且错误链中不得出现宿主绝对路径或环境变量名。
#[test]
fn p13_deny_probe_missing_capability_instantiation_fails_without_host_leak() {
    let engine = host::engine().expect("engine");
    let component = host::component(&engine, "probe_deny.wasm").expect("probe component");
    let linker = host::deny_linker(&engine);
    let mut store = Store::new(&engine, ());

    // probe component 是 wasi:cli command world（无 WIT 导出函数），instantiate
    // 只做 import 解析——默认拒权 linker 下解析必须失败。
    let err = linker
        .instantiate(&mut store, &component)
        .expect_err("默认拒权下 probe component 实例化必须失败");

    // wasmtime::Error 的 Display 已含完整 context 链
    let text = format!("{err}");

    // 1) 拒绝可归因：错误必须点名缺失的 wasi 能力 import（wasmtime 报首个
    //    无法解析的 import；逐接口细粒度归因是 M7+ 宿主 linker 设计，见 SPEC R6）
    assert!(
        text.contains("wasi:") && text.contains("not found in the linker"),
        "错误未归因到被拒的 wasi 能力 import:\n{text}"
    );

    // 2) [P13] 不泄露：宿主路径 / 环境变量不得出现在错误文本
    for leak in ["/Users/", "/home/", "TMPDIR", "HOME=", "PARTISYNC"] {
        assert!(!text.contains(leak), "错误泄露宿主信息 {leak}:\n{text}");
    }
}

/// 对照组：零宿主能力的 demo tool（无 wasi import 需求）在同一个
/// 默认拒权 linker 下必须能实例化并完成 JSON→JSON 调用 ——
/// 「默认拒权」不牺牲窄核心工具面（R6 spike 级回答）。
#[test]
fn p13_contrast_tool_without_capability_needs_still_calls() {
    let engine = host::engine().expect("engine");
    let component = host::component(&engine, "demo_tool.wasm").expect("demo component");
    let linker = host::deny_linker(&engine);
    let mut tool = host::DemoTool::instantiate(&engine, &component, &linker)
        .expect("无能力需求的 tool 在默认拒权 linker 下必须可实例化");

    let out = tool.call(r#"{"k":"v"}"#).expect("call must succeed");
    let v: serde_json::Value = serde_json::from_str(&out).expect("输出必须是 JSON");
    assert_eq!(v["echo"]["k"], "v");
    assert_eq!(v["input_bytes"], 9);
}
