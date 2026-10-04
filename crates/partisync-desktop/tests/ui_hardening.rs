//! M9-WP03-T01 UI 硬化探针（SPEC M9-WP03 §2.1/§3）。
//!
//! R5 落锤（SPEC §6）：JS 单测基建 = Rust 侧**静态契约探针**——
//! `include_str!` 读 `ui/app-core-v3.js` 源码做规则断言（CI 常绿，
//! 零 Node 依赖），T04 全 tab GUI 巡检截图兜底行为面。
//!
//! 契约（任务卡「innerHTML 站点清单」）：
//! - `esc()` 定义唯一且完整转义 `& < > " '` 五字符；
//! - 禁止裸插值：清单内动态字段必须经 `esc(...)`/`trunc + esc` 包裹，
//!   源码中 `${e.name}` 形态裸插值零命中；
//! - N4 开关接线：`mode-transcript` radio → `searchMode = "transcript"`
//!   → `searchArgs` 显式传 `include_transcript: true`。

const UI_JS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/ui/app-core-v3.js"));

/// esc() 转义完整性（SPEC §2.1：防 markup 破格 + 属性逃逸）。
#[test]
fn esc_helper_defined_once_with_full_charset() {
    let defs = UI_JS.matches("function esc(").count();
    assert_eq!(defs, 1, "esc() 必须定义且仅定义一次");
    for (needle, what) in [
        ("replaceAll(\"&\", \"&amp;\")", "&"),
        ("replaceAll(\"<\", \"&lt;\")", "<"),
        ("replaceAll(\">\", \"&gt;\")", ">"),
        ("replaceAll('\"', \"&quot;\")", "\""),
        ("replaceAll(\"'\", \"&#39;\")", "'"),
    ] {
        assert!(UI_JS.contains(needle), "esc() 缺 {} 转义", what);
    }
}

/// 禁止裸插值（任务卡清单内全部动态字段——innerHTML 注入面收敛）。
#[test]
fn no_raw_interpolation_of_dynamic_fields() {
    const BARE: &[&str] = &[
        "${e.name}",
        "${it.name}",
        "${name}",
        "${c.path}",
        "${e.path}",
        "${h.tool}",
        "${it.origin_device}",
        "${it.dir}",
        "${e?.kind",
        // 注意：`${kind}` 在 showError()（textContent，安全）中合法，不入禁列
        "${g.content_id.slice",
        "${r.id.slice",
        "${r.kind}",
        "${r.checkpoint",
        "${t.name}",
        "${q.slice",
        "${h.highlight ||",
        // M9-WP03-T02 记忆面板动态字段（memory_search 命中行 / 包含证明 /
        // 验证横幅）——字符串插值一律 esc；数字（score.toFixed/length）不入列。
        "${m.memory_id",
        "${m.content",
        "${m.tags",
        "${m.origin_device",
        "${p.leaf_hash",
        "${p.root",
        "${r.memory_id",
        "${v.memory_count",
    ];
    let mut leaked = Vec::new();
    for pat in BARE {
        if UI_JS.contains(pat) {
            leaked.push(*pat);
        }
    }
    assert!(
        leaked.is_empty(),
        "裸插值必须经 esc() 包裹（M9-WP03-T01 回归）：{leaked:?}"
    );
}

/// esc() 实际使用规模（24 站点收敛后的量级下限——防探针被整体删空）。
#[test]
fn esc_applied_at_expected_scale() {
    let uses = UI_JS.matches("esc(").count();
    // 1 处定义 + 收敛后 ≥ 25 处调用（清单内全部动态字段）
    assert!(
        uses >= 26,
        "esc() 调用数不足（当前 {uses}），站点收敛被回退？"
    );
}

/// N4 开关接线（SPEC §2.1 + §6-R1 拍板 = 开关化）：
/// transcript radio 显式传参，关键词/语义 radio 维持 None（后端常开）。
#[test]
fn transcript_toggle_wired_to_include_transcript() {
    assert!(
        UI_JS.contains("searchMode = \"transcript\""),
        "mode-transcript radio 必须切到 transcript 模式"
    );
    assert!(
        UI_JS.contains("if (searchMode === \"transcript\") args.include_transcript = true;"),
        "searchArgs 必须对 transcript 模式显式传 include_transcript=true"
    );
    assert!(
        !UI_JS.contains("include_transcript 已在后端常开"),
        "旧 N4「开关仅为语义标注」注释必须移除（诚实化）"
    );
}

// ── M9-WP03-T02：记忆浏览面板静态契约（SPEC §2.2） ──

const UI_HTML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/ui/index.html"));

/// 记忆面板接线（工具名三具全走 mcp_call + payload 形状字面量——与
/// commands.rs `t02_memory_panel_dataflow_via_stub_sidecar` 的请求行断言
/// 逐键对账：那边测传输面，这边绑 UI 构造面）。
#[test]
fn memory_panel_tools_and_payload_contract() {
    for tool in ["memory_search", "memory_write", "memory_verify"] {
        assert!(
            UI_JS.contains(&format!("memCall(\"{tool}\"")),
            "面板未接线 {tool}"
        );
    }
    assert!(
        UI_JS.contains("return mcPayload(await call(\"mcp_call\", { tool, args }));"),
        "memCall 必须走既有 mcp_call IPC（Rust 侧零新增 command）"
    );
    // 工具级错误（CallToolResult isError）文本透传 error-region（§2.2 遥测口径）。
    assert!(
        UI_JS.contains("showError(`[Memory] ${text}`);"),
        "isError 文本必须走 error-region 既有链路"
    );
    // memory_search payload：query/tag 条件键 + limit/offset 恒传。
    assert!(
        UI_JS.contains("const args = { limit: 50, offset: 0 };"),
        "limit/offset 必须恒传"
    );
    assert!(UI_JS.contains("if (q) args.query = q;"), "query 条件键");
    assert!(UI_JS.contains("if (tag) args.tag = tag;"), "tag 条件键");
    // 行级验证 + 写入 payload（tags/metadata 条件键）。
    assert!(
        UI_JS.contains("memCall(\"memory_verify\", { memory_id: memoryId })"),
        "行级「验证」必须按 memory_id 精确查"
    );
    assert!(
        UI_JS.contains("if (tags.length) args.tags = tags;")
            && UI_JS.contains("args.metadata = meta;"),
        "写入 payload：tags/metadata 条件键缺失"
    );
}

/// 记忆 tab 结构 + DOM id 对账：JS 里 `$()` 引用的 mem*/btn-mem* 元素必须
/// 在 index.html 存在（双 include_str 对账，防 id 漂移）；tab 切换数组含
/// memory；5s 轮询面不含记忆 tab（任务卡：防行级证明展开态被打断）。
#[test]
fn memory_tab_structure_and_dom_id_parity() {
    assert!(UI_HTML.contains("data-tab=\"memory\""), "nav 缺「记忆」tab");
    assert!(
        UI_HTML.contains("id=\"view-memory\""),
        "缺 #view-memory section"
    );
    assert!(
        UI_JS.contains("\"browse\", \"search\", \"memory\", \"sync\""),
        "tab 切换数组未含 memory"
    );
    let mut missing: Vec<String> = Vec::new();
    let mut rest = UI_JS;
    while let Some(pos) = rest.find("$(\"") {
        let tail = &rest[pos + 3..];
        let Some(end) = tail.find('"') else { break };
        let id = &tail[..end];
        if (id.starts_with("mem-") || id.starts_with("btn-mem-"))
            && !UI_HTML.contains(&format!("id=\"{id}\""))
        {
            missing.push(id.to_string());
        }
        rest = &tail[end + 1..];
    }
    assert!(
        missing.is_empty(),
        "JS 引用的记忆面板元素在 index.html 缺失：{missing:?}"
    );
    // 轮询分支只刷 browse/sync（loadMemories 不进 setInterval 体）。
    let interval_at = UI_JS.find("setInterval").expect("轮询存在");
    let interval_body = &UI_JS[interval_at..];
    let close = interval_body.find("}, 5000);").expect("interval 闭合");
    assert!(
        !interval_body[..close].contains("loadMemories"),
        "记忆面板不得进 5s 轮询（防展开态被打断）"
    );
}
