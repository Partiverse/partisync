//! T02 PR-B 集成探针：manifest 校验 + component 加载 + linker 注入的
//! 整合路径（\`load_with_manifest\`）。覆盖 [P13] per-call 拒绝语义与
//! [P14] 加载即拒不变量（拒绝先于任何 host function 暴露）。
//!
//! fixture 复用 spike assets（demo_tool.wasm + probe_deny.wasm）——
//! 与 M6-WP04 spike 字节一致；本探针验产品宿主面的契约，而非 spike。

use std::io::Write;

use partisync_ext_host::{load_with_manifest, HostState, ManifestError};

fn fixture_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .to_path_buf()
}

#[test]
fn load_with_manifest_missing_manifest_io_rejected() {
    let dir = fixture_dir();
    let wasm = dir.join("demo_tool.wasm");
    let manifest = dir.join("demo_tool.json");
    // fixture 内无 manifest——预期 IO 拒绝（[P14] 缺 manifest 即拒）
    let err = match load_with_manifest(&wasm, &manifest) {
        Ok(_) => panic!("缺 manifest 必须拒绝，得到 Ok"),
        Err(e) => e,
    };
    assert!(
        matches!(err, ManifestError::Io(_)),
        "缺 manifest 应为 Io 错误，得到 {err:?}"
    );
}

/// 整合路径：写合法 manifest 到临时目录 → demo_tool.wasm 在同
/// 目录拷贝过来 → load_with_manifest 应通过校验 + 加载 + 注入
/// clock.read linker；该 component 不引用 clock.read host function
/// （demo_tool 自身无 import），但注入面=声明面（[P14] 不变量）合法。
#[test]
fn load_with_manifest_valid_clock_then_instantiate_succeeds() {
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    let src_dir = fixture_dir();
    // 复制 demo_tool.wasm 到临时目录
    let wasm_dst = tmp.path().join("ext.wasm");
    fs::copy(src_dir.join("demo_tool.wasm"), &wasm_dst).unwrap();
    // 写合法 manifest（clock.read 声明）
    let manifest_path = tmp.path().join("ext.json");
    let mut f = fs::File::create(&manifest_path).unwrap();
    f.write_all(br#"{"tool_name":"my_demo","capabilities":["clock.read"]}"#)
        .unwrap();
    // 整合加载：合法 manifest + 零能力 demo tool → 注入 clock.read
    // host function（demo_tool 不调用，但注入面=声明面是 [P14] 不变量）
    let loaded = match load_with_manifest(&wasm_dst, &manifest_path) {
        Ok(p) => p,
        Err(e) => panic!("合法 manifest + 组件应通过：{e}"),
    };
    // linker 根 instance 可取；该 component 零 import 时实例化必然成功
    let mut store = wasmtime::Store::new(partisync_ext_host::engine(), HostState::without_index());
    partisync_ext_host::init_termination_budget(&mut store);
    // M8-WP06：epoch/fuel 开启的 Engine 上自建 Store 需初始化终止预算
    // （装载期预注入只覆盖 ExtTool::load 内部 Store；deadline 大 delta
    // 防 current+delta 溢出）
    store.set_epoch_deadline(u64::MAX / 2);
    store
        .set_fuel(partisync_ext_host::registry::FUEL_BUDGET)
        .expect("fuel init");
    let instance = match loaded.1.instantiate(&mut store, &loaded.0) {
        Ok(i) => i,
        Err(e) => panic!("零 import demo component 在带 clock.read linker 下应可实例化：{e}"),
    };
    // get_typed_func call(input) → output：与 spike 同通路
    let func = instance
        .get_typed_func::<(String,), (String,)>(&mut store, "call")
        .expect("demo_tool 暴露 call(input) → output");
    let (out,) = func
        .call(&mut store, (r#"{"k":"v"}"#.to_owned(),))
        .expect("call must succeed");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["echo"]["k"], "v");
}

/// [P14] 撞名整合探针：合法 .wasm 配撞名 manifest → 加载期拒绝
/// （即使 component 本身合法）。这是「拒绝先于任何 host function 暴露」
/// 的端到端证据：manifest 校验在 component 实例化前完成。
#[test]
fn load_with_manifest_builtin_collision_rejected_before_instantiation() {
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    let src_dir = fixture_dir();
    let wasm_dst = tmp.path().join("clashing.wasm");
    fs::copy(src_dir.join("demo_tool.wasm"), &wasm_dst).unwrap();
    let manifest_path = tmp.path().join("clashing.json");
    let mut f = fs::File::create(&manifest_path).unwrap();
    f.write_all(br#"{"tool_name":"asset_read","capabilities":[]}"#)
        .unwrap();
    let err = match load_with_manifest(&wasm_dst, &manifest_path) {
        Ok(_) => panic!("撞名必拒，得到 Ok"),
        Err(e) => e,
    };
    match err {
        ManifestError::Validate(partisync_ext_host::ValidateError::ToolNameCollision(
            "asset_read",
        )) => {}
        other => panic!("期望撞名拒绝 asset_read，得到 {other:?}"),
    }
}

/// [P13] 整合：probe_deny.wasm（缺权 component）在带 clock.read linker
/// 下实例化必败——linker 注入 clock.read 不会「救」它；probe 的
/// wasi:cli import 仍被默认拒（[P13] 默认拒权不变）。
#[test]
fn load_with_manifest_deny_probe_still_fails_under_clock_linker() {
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    let src_dir = fixture_dir();
    let wasm_dst = tmp.path().join("probe.wasm");
    fs::copy(src_dir.join("probe_deny.wasm"), &wasm_dst).unwrap();
    let manifest_path = tmp.path().join("probe.json");
    let mut f = fs::File::create(&manifest_path).unwrap();
    f.write_all(br#"{"tool_name":"probe_with_clock","capabilities":["clock.read"]}"#)
        .unwrap();
    let loaded = match load_with_manifest(&wasm_dst, &manifest_path) {
        Ok(p) => p,
        Err(e) => panic!("合法 manifest 应通过校验：{e}"),
    };
    let mut store = wasmtime::Store::new(partisync_ext_host::engine(), HostState::without_index());
    partisync_ext_host::init_termination_budget(&mut store);
    let err = match loaded.1.instantiate(&mut store, &loaded.0) {
        Ok(_) => panic!("缺 wasi import component 在 clock.read linker 下必拒"),
        Err(e) => e,
    };
    let text = format!("{err}");
    assert!(
        text.contains("wasi:") && text.contains("not found in the linker"),
        "错误未归因到被拒的 wasi 能力 import:\n{text}"
    );
}
