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

use partisync_ext_host::{Capability, HostState, IndexError, IndexRead, Manifest, MAX_QUERY_BYTES};
use wasmtime::Store;

/// 测试替身：把查询原样回显，包在 JSON 里（模拟真实索引实现的形状）。
struct EchoIndex;

impl IndexRead for EchoIndex {
    fn search(&self, query: &str) -> Result<String, IndexError> {
        Ok(format!(r#"{{"echo":{query},"backend":"echo"}}"#))
    }
}

/// 恒失败替身：模拟「索引后端不可用」——F-2 的核心场景（实现方必须
/// 返回 `Err`，绝不 panic）。
struct FailingIndex;

impl IndexRead for FailingIndex {
    fn search(&self, _query: &str) -> Result<String, IndexError> {
        Err(IndexError::Backend("index engine offline".into()))
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

/// **F-2 核心探针**：实现方返回 `Err` 时，扩展拿到的是**错误 JSON**，
/// 且同一实例可继续成功调用（验证失败路径不污染调用状态）。
///
/// **对照（审查实证更正）**：若实现方 panic，wasmtime 同步路径下 panic
/// 以 Rust panic 形态**直接传播出本测试的 `search.call(...)`**（第一次
/// 调用即失败于 panic，而非 trap 或「第二次失败于 poisoned store」——
/// 「毒化」仅存在于 concurrent API，本 crate 未使用）。`Result` 化把
/// 这条不可控的传播路径收敛为可归因的错误值。
#[test]
fn t04b_backend_error_returns_json_and_keeps_store_usable() {
    let component = index_probe_component();
    let linker = partisync_ext_host::linker_for(partisync_ext_host::engine(), &index_manifest())
        .expect("注权 index.read 的 linker 构造成功");
    let mut store = Store::new(
        partisync_ext_host::engine(),
        HostState::with_index(Arc::new(FailingIndex)),
    );
    let instance = match linker.instantiate(&mut store, &component) {
        Ok(i) => i,
        Err(e) => panic!("实例化必须成功：{e}"),
    };
    let search = instance
        .get_typed_func::<(String,), (String,)>(&mut store, "search")
        .expect("index_probe 导出 search(query)");

    // 第一次调用：后端失败 → 错误 JSON，不是 trap
    let (out,) = search
        .call(&mut store, ("q".to_owned(),))
        .expect("后端失败必须以错误 JSON 返回，不得 panic 越界传播");
    assert!(
        out.contains(r#""error""#) && out.contains("index engine offline"),
        "后端错误必须可归因，得到：{out}"
    );

    // 第二次调用：调用状态无残留（Err 值路径不携带任何污染）
    let (out2,) = search
        .call(&mut store, ("q2".to_owned(),))
        .expect("错误路径后第二次调用必须仍可执行");
    assert!(
        out2.contains("index engine offline"),
        "第二次调用结果：{out2}"
    );
}

/// **F-4 探针**：`preflight` 把「声明了 `index.read` 但组装期未接线」
/// 从运行期静默降级变为**加载期拒**。
#[test]
fn t04b_preflight_rejects_index_read_without_wiring() {
    let unwired = HostState::without_index();
    let err = unwired
        .preflight(&index_manifest())
        .expect_err("声明 index.read 而宿主未接线必须被 preflight 拒");
    assert_eq!(err, IndexError::NotWired);

    // 对照：未声明 index.read 的 manifest 不受影响
    let clock_only = Manifest {
        tool_name: "clock_only".into(),
        capabilities: vec![Capability::ClockRead],
    };
    assert_eq!(unwired.preflight(&clock_only), Ok(()));

    // 对照：已接线则通过
    let wired = HostState::with_index(Arc::new(EchoIndex));
    assert_eq!(wired.preflight(&index_manifest()), Ok(()));

    // 组合态（审查 P2-6）：index.read + clock.read 并声明，已接线 → 过
    let combo = Manifest {
        tool_name: "combo".into(),
        capabilities: vec![Capability::IndexRead, Capability::ClockRead],
    };
    assert_eq!(wired.preflight(&combo), Ok(()));
    // 组合态未接线 → 仍拒（index.read 是唯一需接线项）
    assert_eq!(unwired.preflight(&combo), Err(IndexError::NotWired));
}

/// **F-12 探针**：超长查询串被宿主侧拦截，返回可归因错误 JSON。
#[test]
fn t04b_oversized_query_rejected_by_host_side_limit() {
    let component = index_probe_component();
    let linker = partisync_ext_host::linker_for(partisync_ext_host::engine(), &index_manifest())
        .expect("注权 index.read 的 linker 构造成功");
    let mut store = Store::new(
        partisync_ext_host::engine(),
        HostState::with_index(Arc::new(EchoIndex)),
    );
    let instance = match linker.instantiate(&mut store, &component) {
        Ok(i) => i,
        Err(e) => panic!("实例化必须成功：{e}"),
    };
    let search = instance
        .get_typed_func::<(String,), (String,)>(&mut store, "search")
        .expect("index_probe 导出 search(query)");

    // 上限之下：正常通过
    let ok_query = "q".repeat(MAX_QUERY_BYTES);
    let (out,) = search
        .call(&mut store, (ok_query,))
        .expect("上限内的查询必须正常执行");
    assert!(
        out.contains(r#""backend":"echo""#),
        "上限内应正常返回：{out}"
    );

    // 超出上限：被拒
    let too_long = "q".repeat(MAX_QUERY_BYTES + 1);
    let (out2,) = search
        .call(&mut store, (too_long,))
        .expect("超长查询应以错误 JSON 返回，不得 trap");
    assert!(
        out2.contains(r#""error""#) && out2.contains("query too long"),
        "超长查询必须被限额拦截并可归因，得到：{out2}"
    );
}

/// JSON 错误消息的转义：后端错误含引号 / 反斜杠 / 换行时不得破坏
/// wire 格式（`inject::json_string` 的行为面）。
#[test]
fn t04b_backend_error_message_is_json_escaped() {
    let component = index_probe_component();
    let linker = partisync_ext_host::linker_for(partisync_ext_host::engine(), &index_manifest())
        .expect("注权 index.read 的 linker 构造成功");
    let mut store = Store::new(
        partisync_ext_host::engine(),
        HostState::with_index(Arc::new(QuoteIndex)),
    );
    let instance = match linker.instantiate(&mut store, &component) {
        Ok(i) => i,
        Err(e) => panic!("实例化必须成功：{e}"),
    };
    let search = instance
        .get_typed_func::<(String,), (String,)>(&mut store, "search")
        .expect("index_probe 导出 search(query)");
    let (out,) = search
        .call(&mut store, ("q".to_owned(),))
        .expect("错误路径必须返回 JSON 而非 trap");
    // out 必须是合法 JSON 且能解出 error 字段
    let parsed: serde_json::Value =
        serde_json::from_str(&out).unwrap_or_else(|e| panic!("错误 JSON 不合法：{e}\n原文：{out}"));
    // `IndexError::Backend` 的 Display 带 `index backend error: ` 前缀
    let msg = parsed["error"]
        .as_str()
        .unwrap_or_else(|| panic!("error 字段必须是字符串：{out}"));
    assert_eq!(msg, "index backend error: he said \"hi\" \\ then\nnewline");
}

/// 含需转义字符的后端错误消息。
struct QuoteIndex;

impl IndexRead for QuoteIndex {
    fn search(&self, _query: &str) -> Result<String, IndexError> {
        Err(IndexError::Backend(
            "he said \"hi\" \\ then\nnewline".into(),
        ))
    }
}
