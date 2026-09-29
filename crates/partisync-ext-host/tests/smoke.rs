//! T01 加载冒烟（SPEC M7-WP01 §3 任务 T01）：engine 单例 + 磁盘缓存 +
//! component 加载 + 实例化调用通路。fixture 复用 spike component 产物
//! （M6-WP04 §2.3：`demo_tool.wasm` = world `partisync:demo/demo-tool`
//! 零宿主能力依赖；`probe_deny.wasm` = wasi:cli command world 缺权探针）。

use wasmtime::Store;

fn fixture(name: &str) -> wasmtime::component::Component {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    partisync_ext_host::load_component(&path).unwrap_or_else(|e| panic!("load {name}: {e}"))
}

/// 加载 + 实例化 + call 通路：零能力 demo tool 在默认拒权 linker 下
/// 完成 JSON→JSON 调用（「默认拒权不牺牲窄核心工具面」的产品面复验）。
#[test]
fn smoke_demo_tool_loads_and_calls() {
    let engine = partisync_ext_host::engine();
    let component = fixture("demo_tool.wasm");
    let linker = partisync_ext_host::deny_linker();
    let mut store = Store::new(engine, ());
    let instance = linker
        .instantiate(&mut store, &component)
        .expect("零能力 demo tool 必须在默认拒权 linker 下实例化");
    let func = instance
        .get_typed_func::<(String,), (String,)>(&mut store, "call")
        .expect("export `call` 未找到");
    let (out,) = func
        .call(&mut store, (r#"{"k":"v"}"#.to_owned(),))
        .expect("call must succeed");
    // 与 spike 对照测试同强度（deny_probes.rs）：解析式断言 echo 回环 +
    // 载荷计量，杜绝 contains 子串误真
    let v: serde_json::Value = serde_json::from_str(&out).expect("输出必须是 JSON");
    assert_eq!(v["echo"]["k"], "v");
    assert_eq!(v["input_bytes"], 9);
}

/// probe component（缺 wasi import）在默认拒权 linker 下实例化即拒，
/// 错误不泄露宿主路径/env（[P13] 冒烟级复验；per-call 全量探针随 T03）。
#[test]
fn smoke_deny_probe_instantiation_fails_without_host_leak() {
    let engine = partisync_ext_host::engine();
    let component = fixture("probe_deny.wasm");
    let linker = partisync_ext_host::deny_linker();
    let mut store = Store::new(engine, ());
    let err = linker
        .instantiate(&mut store, &component)
        .expect_err("默认拒权下缺权 component 实例化必须失败");
    let text = format!("{err}");
    assert!(
        text.contains("wasi:") && text.contains("not found in the linker"),
        "错误未归因到被拒的 wasi 能力 import:\n{text}"
    );
    for leak in ["/Users/", "/home/", "TMPDIR", "HOME=", "PARTISYNC"] {
        assert!(!text.contains(leak), "错误泄露宿主信息 {leak}:\n{text}");
    }
}

/// Engine 单例同一性：多次调用返回同一实例（进程级缓存前提）。
#[test]
fn smoke_engine_is_process_singleton() {
    assert!(std::ptr::eq(
        partisync_ext_host::engine() as *const _,
        partisync_ext_host::engine() as *const _
    ));
}
