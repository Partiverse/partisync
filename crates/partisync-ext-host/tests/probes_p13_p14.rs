//! T03：[P13] 全量拒绝探针 + [P14] 注权可达端到端
//! （SPEC M7-WP01 §3 验收项 1）。
//!
//! 本 PR 的核心是把 [P13] 从「spike 期实例化期默认拒」推进到
//! **实施面**：未注权接口对 component 完全不可达（零 interface 实例），
//! 注权接口则真实可调用。
//!
//! fixture：`clock_probe.wat`（wit-component 生成，可复现路径见文件头）
//! ——import `partisync:ext/clock@0.1.0` 的 `now-millis`，export `now()`。
//! FS / 网络两类复用 T01 smoke 的 `probe_deny.wasm`（wasip2 std，import
//! wasi:filesystem / wasi:sockets / wasi:clocks 三类），覆盖 SPEC §3
//! 验收项 1 的三类 API 面。

use std::io::Write;

use partisync_ext_host::{load_with_manifest, Manifest, ManifestError};
use wasmtime::Store;

fn fixture_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .to_path_buf()
}

/// 从 WAT 源码编译 component（wasmtime `wat` feature 开启，
/// ADR-0025 线位）——fixture 保持源码形态，可读可审、可 diff。
fn clock_probe_component() -> wasmtime::component::Component {
    let wat = include_str!("fixtures/clock_probe.wat");
    wasmtime::component::Component::new(partisync_ext_host::engine(), wat)
        .expect("clock_probe.wat 必须编译为 component（fixture 损坏）")
}

fn write_manifest(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
    let p = dir.join("ext.json");
    let mut f = std::fs::File::create(&p).expect("create manifest");
    f.write_all(body.as_bytes()).expect("write manifest");
    p
}

// ---------------- [P14] 注权可达（双向验证的「可达」侧） ----------------

/// 注权 `clock.read` → 真实 host function 被调用，返回合理毫秒时间戳。
/// 这是 SPEC §3 验收项 1 的正向半边，也是 T02 遗留缺口的闭环
/// （对抗审查 F-4：T02 只验了「注入链路通」，没验「真被调用」）。
#[test]
fn p14_clock_granted_clock_probe_calls_host_function() {
    let component = clock_probe_component();
    let m = Manifest {
        tool_name: "clock_granted".into(),
        capabilities: vec![partisync_ext_host::Capability::ClockRead],
    };
    m.validate().expect("clock.read manifest 合法");
    let linker = partisync_ext_host::linker_for(partisync_ext_host::engine(), &m)
        .expect("注权 clock.read 的 linker 构造成功");

    let mut store = Store::new(partisync_ext_host::engine(), ());
    let instance = match linker.instantiate(&mut store, &component) {
        Ok(i) => i,
        Err(e) => panic!("注权 clock.read 后 clock_probe 必须可实例化：{e}"),
    };
    let now = instance
        .get_typed_func::<(), (u64,)>(&mut store, "now")
        .expect("clock_probe 导出 now()");
    let (t,) = now.call(&mut store, ()).expect("now() 必须可调用");
    // 合理毫秒时间戳下界：2020-01-01T00:00:00Z
    assert!(
        t > 1_577_836_800_000,
        "宿主注入的 now_millis 返回异常值 {t}（未真实调用 host function？）"
    );
}

/// **拒绝面的强度与 SPEC 措辞的调和**（对抗审查 F-2 登记）：SPEC §3
/// 与 properties.md 措辞为「per-call 拒绝」。Component Model 的注权是
/// **实例化期**语义——import 是静态的，宿主 linker 无对应 interface
/// 实例时 component 根本无法实例化，guest 代码一行都不执行。因此
/// 本 crate 达成的是**严格强于** per-call 的形式：拒绝发生在任何
/// 潜在调用之前。properties.md 的 per-call 措辞按「以更强形式满足」
/// 登记，语义无缺口。
#[test]
fn p13_index_read_declared_but_not_wired_rejects_clock_import() {
    let component = clock_probe_component();
    let m = Manifest {
        tool_name: "clock_denied".into(),
        capabilities: vec![partisync_ext_host::Capability::IndexRead],
    };
    m.validate().expect("index.read 在白名单内，manifest 合法");
    let linker = partisync_ext_host::linker_for(partisync_ext_host::engine(), &m)
        .expect("index.read 占位注入不报错（尚未接线）");
    let mut store = Store::new(partisync_ext_host::engine(), ());
    let err = match linker.instantiate(&mut store, &component) {
        Ok(_) => panic!("只注权 index.read 时，clock_probe 的 clock import 必须无法解析"),
        Err(e) => e,
    };
    let text = format!("{err}");
    assert!(
        text.contains("partisync:ext/clock@0.1.0") && text.contains("not found in the linker"),
        "错误未归因到未注权 interface：\n{text}"
    );
}
// ---------------- [P13] 零注权拒绝面（三类 API） ----------------

/// 零注权（manifest 无任何 capability）→ 需宿主能力的 component 实例化
/// 必拒，错误可归因且不泄露宿主路径/env（[P13] 实施面 + [P14] 拒绝先于
/// 任何 host function 暴露）。
#[test]
fn p13_zero_grant_deny_probe_rejected_without_host_leak() {
    let component = clock_probe_component();
    let m = Manifest {
        tool_name: "no_grant".into(),
        capabilities: vec![],
    };
    let linker = partisync_ext_host::linker_for(partisync_ext_host::engine(), &m)
        .expect("零注权 linker 构造成功（零注入）");
    let mut store = Store::new(partisync_ext_host::engine(), ());
    let err = match linker.instantiate(&mut store, &component) {
        Ok(_) => panic!("零注权下 clock_probe 必须实例化失败"),
        Err(e) => e,
    };
    let text = format!("{err}");
    assert!(
        text.contains("partisync:ext/clock@0.1.0"),
        "错误未归因到被拒的 interface import：\n{text}"
    );
    for leak in ["/Users/", "/home/", "TMPDIR", "HOME=", "PARTISYNC"] {
        assert!(!text.contains(leak), "错误泄露宿主信息 {leak}：\n{text}");
    }
}

/// **FS / 网络 / 时钟三类 WASI 能力**（SPEC §3 验收项 1 的三类 API 面）：
/// 零注权 linker 下 `probe_deny.wasm`（wasip2 std，源码里依次触碰
/// `std::fs` / `std::net::TcpStream` / `std::time::SystemTime`）实例化
/// 必拒，错误可归因且不泄露宿主路径/env。
///
/// **覆盖度诚实登记**（对抗审查 F-1）：wasmtime TypeChecker 遍历
/// import 时在**第一个**无法解析处即 bail——实测首个拒绝项是
/// `wasi:io/poll`（wasip2 std 的 pollability 前置依赖），并非 fs/net/
/// clock 三者之一。故本探针证明的是「需宿主能力的 wasip2 component
/// 整体不可达」，**不是**三类逐类隔离。逐类隔离（三个最小 fixture
/// 各 import 恰好一类）留后续任务：宿主白名单冻结表本就不含 FS/
/// 网络，逐类探针的增量是回归保护而非新增安全面。
#[test]
fn p13_wasi_gated_component_denied_under_zero_grant() {
    let dir = fixture_dir();
    let wasm = dir.join("probe_deny.wasm");
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest = write_manifest(tmp.path(), r#"{"tool_name":"fs_probe","capabilities":[]}"#);
    let (component, linker) = match load_with_manifest(&wasm, &manifest) {
        Ok(p) => p,
        Err(e) => panic!("零注权 manifest 应通过校验：{e}"),
    };
    let mut store = Store::new(partisync_ext_host::engine(), ());
    let err = match linker.instantiate(&mut store, &component) {
        Ok(_) => panic!("零注权下 wasip2 std component 必须实例化失败（FS/网络/时钟全拒）"),
        Err(e) => e,
    };
    let text = format!("{err}");
    assert!(
        text.contains("wasi:") && text.contains("not found in the linker"),
        "错误未归因到被拒的 wasi 能力 import：\n{text}"
    );
    for leak in ["/Users/", "/home/", "TMPDIR", "HOME=", "PARTISYNC"] {
        assert!(!text.contains(leak), "错误泄露宿主信息 {leak}：\n{text}");
    }
}

/// manifest 校验失败先于 component 加载（[P14] 拒绝先于任何 host
/// function 暴露的加载侧证据）：撞名 manifest 配合法 component，仍在
/// 加载期被拒——component 从未被编译、linker 从未被构造。
#[test]
fn p14_collision_rejected_before_component_compile() {
    let dir = fixture_dir();
    let tmp = tempfile::tempdir().expect("tempdir");
    let wasm = tmp.path().join("collide.wasm");
    std::fs::copy(dir.join("demo_tool.wasm"), &wasm).expect("copy fixture");
    let manifest = write_manifest(
        tmp.path(),
        r#"{"tool_name":"search","capabilities":["clock.read"]}"#,
    );
    let err = match load_with_manifest(&wasm, &manifest) {
        Ok(_) => panic!("撞名 manifest 必须加载期拒绝"),
        Err(e) => e,
    };
    assert!(
        matches!(err, ManifestError::Validate(_)),
        "撞名应为校验错误"
    );
}
