//! T04-A：`index.read` 注权端到端（SPEC M7-WP01 §2.1 trait 注入契约）。
//!
//! fixture `index_probe.wat`（生成路径见文件头）import
//! `partisync:ext/index@0.1.0` 的 `search: func(query: string) -> string`，
//! export 同名 `search`。本文件验证 trait 注入方向：
//! 组装期注入实现 → 扩展可调用 → 入参透传 / 出参原样回传。
//!
//! 与 T03 的 clock 探针同构，但注入的是**用户提供的实现**（`IndexRead`
//! trait）而非宿主内建函数——这是 SPEC §2.1「不直连 index/graph crate」
//! 契约的实测面。

use std::sync::Arc;

use partisync_ext_host::{Capability, HostState, IndexRead, Manifest};
use wasmtime::Store;

/// 测试替身：把查询原样回显，包在 JSON 里（模拟真实索引实现的形状）。
struct EchoIndex;

impl IndexRead for EchoIndex {
    fn search(&self, query: &str) -> String {
        format!(r#"{{"echo":{query},"backend":"echo"}}"#)
    }
}

fn index_probe_component() -> wasmtime::component::Component {
    let wat = include_str!("fixtures/index_probe.wat");
    wasmtime::component::Component::new(partisync_ext_host::engine(), wat)
        .expect("index_probe.wat 必须编译为 component（fixture 损坏）")
}

fn index_manifest() -> Manifest {
    let m = Manifest {
        tool_name: "index_user".into(),
        capabilities: vec![Capability::IndexRead],
    };
    m.validate().expect("index.read 在白名单内");
    m
}

/// 注权 `index.read` + 组装期注入实现 → 扩展可调用，查询串原样抵达
/// trait 实现，扩展回传的字符串原样返回给宿主调用面。
#[test]
fn t04a_index_read_calls_injected_trait_impl() {
    let component = index_probe_component();
    let linker = partisync_ext_host::linker_for(partisync_ext_host::engine(), &index_manifest())
        .expect("注权 index.read 的 linker 构造成功");
    let mut store = Store::new(
        partisync_ext_host::engine(),
        HostState::with_index(Arc::new(EchoIndex)),
    );
    let instance = match linker.instantiate(&mut store, &component) {
        Ok(i) => i,
        Err(e) => panic!("注权 index.read 后 index_probe 必须可实例化：{e}"),
    };
    let search = instance
        .get_typed_func::<(String,), (String,)>(&mut store, "search")
        .expect("index_probe 导出 search(query)");
    let (out,) = search
        .call(&mut store, (r#"{"query":"beach sunset"}"#.to_owned(),))
        .expect("search 必须可调用");
    assert_eq!(out, r#"{"echo":{"query":"beach sunset"},"backend":"echo"}"#);
}

/// 未注权 `index.read`（manifest 空 capability）→ import 不可解析，
/// 实例化必拒（[P13]：未授予能力的接口不可达）。这是 index.read
/// 注权面的「拒绝半边」，与 T03 clock 探针同构。
#[test]
fn t04a_index_read_ungranted_rejects_instantiation() {
    let component = index_probe_component();
    let m = Manifest {
        tool_name: "no_index".into(),
        capabilities: vec![],
    };
    let linker = partisync_ext_host::linker_for(partisync_ext_host::engine(), &m)
        .expect("零注权 linker 构造成功");
    let mut store = Store::new(partisync_ext_host::engine(), HostState::without_index());
    let err = match linker.instantiate(&mut store, &component) {
        Ok(_) => panic!("未注权 index.read 时 index_probe 必须实例化失败"),
        Err(e) => e,
    };
    let text = format!("{err}");
    assert!(
        text.contains("partisync:ext/index@0.1.0") && text.contains("not found in the linker"),
        "错误未归因到未注权 index interface：\n{text}"
    );
    // [P13]：拒绝信息不泄露宿主路径 / 环境变量（与既有 P13 探针同构）
    for leak in ["/Users/", "/home/", "TMPDIR", "HOME=", "PARTISYNC"] {
        assert!(!text.contains(leak), "错误泄露宿主信息 {leak}：\n{text}");
    }
}

/// 注权但**组装期未注入**（`HostState::index = None`）→ 注入面由
/// manifest 声明决定，实例化成功；调用返回错误 JSON 而非 trap。
/// 这是 SPEC §2.1「注权面（声明）与宿主接线（实现）分离」的实测面。
#[test]
fn t04a_index_granted_but_unwired_returns_error_json() {
    let component = index_probe_component();
    let linker = partisync_ext_host::linker_for(partisync_ext_host::engine(), &index_manifest())
        .expect("注权 index.read 的 linker 构造成功");
    let mut store = Store::new(partisync_ext_host::engine(), HostState::without_index());
    let instance = match linker.instantiate(&mut store, &component) {
        Ok(i) => i,
        Err(e) => panic!("注权后即便宿主未接线，实例化仍应成功：{e}"),
    };
    let search = instance
        .get_typed_func::<(String,), (String,)>(&mut store, "search")
        .expect("index_probe 导出 search(query)");
    let (out,) = search
        .call(&mut store, ("probe".to_owned(),))
        .expect("search 必须可调用（返回错误 JSON 而非 trap）");
    assert!(
        out.contains(r#""error""#) && out.contains("index.read unavailable"),
        "未接线时必须返回可归因的错误 JSON，得到：{out}"
    );
}
