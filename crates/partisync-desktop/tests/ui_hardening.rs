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
        // M10-WP01-T02 收紧：`${h.highlight ||` → `${h.highlight`（覆盖
        // 一切形态——snippet 渲染唯一入口是 snippetHtml(h)，见 T02 探针）。
        "${h.highlight",
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
        // M10-WP03-T03 浏览行 mtime 列：相对/绝对时间一律经 mtimeDisp()，
        // title 经 esc(timeFmt(...))——裸 `${e.mtime_ns` 插值禁入模板。
        "${e.mtime_ns",
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

/// M10-WP01-T01：命中卡 name 行优先透出 graph 文件名（「GUI 搜索形同
/// 虚设」修复的 UI 面——filename 缺失才回落哈希切片）。
#[test]
fn hit_card_prefers_graph_filename() {
    assert!(
        UI_JS.contains("${h.filename ? esc(h.filename) : esc(h.content_id.slice(0, 8)) + \"…\"}"),
        "命中卡 name 行必须优先 filename（回落哈希切片）"
    );
}

// ── M10-WP01-T02：命中词高亮 snippet 渲染契约（SPEC §2.2 / §3） ──

const UI_CSS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/ui/styles-v3.css"));

/// snippet 渲染 = esc() 全串**之后** sentinel→`<mark>`/`</mark>` 替换
/// （次序锁死——反序 = 文档内容经 sentinel 逃逸为 HTML 的注入面）；
/// 模板零未转义 `${h.highlight}` 插值（唯一入口 snippetHtml(h)）；
/// `<mark>` 样式存在于 styles-v3.css。
#[test]
fn t02_snippet_mark_escapes_then_replaces_sentinel() {
    assert!(
        UI_JS.contains("function snippetHtml("),
        "snippet 渲染必须收敛到唯一 helper snippetHtml()"
    );
    assert!(
        UI_JS.contains(
            "return esc(h.highlight).replaceAll(\"[[\", \"<mark>\").replaceAll(\"]]\", \"</mark>\");"
        ),
        "必须 esc 全串之后再做 sentinel→<mark> 替换（次序不可反）"
    );
    assert!(
        UI_JS.contains("<div class=\"snippet\">${snippetHtml(h)}</div>"),
        "命中卡 snippet 行必须经 snippetHtml(h) 渲染"
    );
    assert!(
        !UI_JS.contains("${h.highlight"),
        "模板不得出现未转义 ${{h.highlight}} 插值（不变量，SPEC §2.2）"
    );
    assert!(
        UI_CSS.contains(".hit .snippet mark"),
        "<mark> 高亮样式必须存在于 styles-v3.css"
    );
}

/// M10-WP01-T02 GUI 验收实测发现：CSP `script-src 'self'` 拦截 index.html
/// 的 inline `onsubmit="return false"`（M9 硬化收敛漏网）→ 检索 form
/// 默认提交 → 页面 reload，检索 UI 完全不可用。锁定：submit 必须经 JS
/// preventDefault 兜底（资产 + 记忆两处 query-row），index.html 不得
/// 回退到 inline onsubmit 依赖。
#[test]
fn t02_search_form_submit_prevent_default_wired() {
    assert!(
        UI_JS.contains(
            "document.querySelectorAll(\"form.query-row\").forEach(f =>\n  f.addEventListener(\"submit\", (e) => e.preventDefault()));"
        ),
        "检索 form 必须经 JS preventDefault 阻止默认提交（CSP 挡 inline onsubmit）"
    );
    assert!(
        !UI_HTML.contains("onsubmit="),
        "index.html 不得依赖 inline onsubmit（CSP script-src 'self' 拦截 = 死代码 + 误导）"
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

// ── M9-WP03-T03：旗舰检索记忆通道静态契约（SPEC §2.3 + §3「分区展示
// 测试」的 JS 展示面；数据流面由 commands.rs
// `t03_flagship_memory_dual_channel_via_stub_sidecar` 钉住） ──

/// 「含记忆」勾选接线：checkbox 默认勾选 + doSearch 读取 .checked +
/// 双通道并行 + memory_search payload 恰为 {query}（SPEC §2.3 字面）。
#[test]
fn t03_flagship_memory_toggle_wired() {
    assert!(
        UI_HTML.contains("id=\"mode-memory\" checked"),
        "「含记忆」checkbox 必须存在且默认勾选（SPEC §2.3）"
    );
    assert!(
        UI_JS.contains("$(\"mode-memory\").checked"),
        "doSearch 必须读取「含记忆」勾选状态"
    );
    assert!(
        UI_JS.contains("memCall(\"memory_search\", { query: q })"),
        "记忆通道必须经 mcp_call memory_search 且 payload 恰为 {{query}}（SPEC §2.3）"
    );
    assert!(
        UI_JS.contains("await Promise.all([call(cmd, searchArgs(q)), memPromise])"),
        "双通道必须并行发起，且资产通道参数沿 searchArgs 现状（资产区不受影响）"
    );
}

// ── M10-WP01-T03：检索结果-详情联动静态契约（SPEC §2.3 + §3） ──

/// 联动探针（SPEC §3「联动探针」）：命中卡 click → `showDetail(h.content_id,
/// h.filename)` 接线断言 + 打开详情路径零 #srows 写入（硬约束：点击不清空
/// 结果列表）+ 关闭仅隐藏面板 + 整卡可点击的可访问语义与样式。
#[test]
fn t03_hit_card_click_wires_show_detail_and_keeps_results() {
    // 1) 接线断言（SPEC §3 字面）：命中卡 onclick → 既有 showDetail 渲染
    //    契约（asset_detail IPC 零新增 command），入参恰为 (content_id,
    //    filename)——标题回落语义收敛在 showDetail 内（§2.1 沿革）。
    assert!(
        UI_JS.contains(
            "card.onclick = () => showDetail(h.content_id, h.filename, \"sdetail-panel\");"
        ),
        "命中卡 click 必须接 showDetail(h.content_id, h.filename)（SPEC §2.3）"
    );
    // 2) 点击不清空结果列表：命中卡接线块内零 innerHTML 写入——详情
    //    面板独立于 #srows（SPEC §2.3 硬约束）。
    let bind_at = UI_JS.find("article.hit[data-cid]").expect("命中卡接线存在");
    let seg = &UI_JS[bind_at..];
    let bind_end = seg.find("});").expect("命中卡接线闭合");
    let wiring = &seg[..bind_end];
    assert!(
        !wiring.contains("innerHTML"),
        "打开详情路径不得重写 #srows（点击不得清空结果列表）"
    );
    // 3) 键盘可达（可访问语义）：Enter/Space 触发 + role=button/tabindex。
    assert!(
        UI_JS.contains("e.key === \"Enter\" || e.key === \" \""),
        "命中卡必须支持键盘触发"
    );
    assert!(
        UI_JS.contains("role=\"button\" tabindex=\"0\" data-cid="),
        "命中卡模板必须带 role=button + tabindex 可访问语义"
    );
    // 4) 面板结构：检索视图内联详情面板 + 常驻关闭按钮（DOM id 对账）。
    assert!(
        UI_HTML.contains("id=\"sdetail-panel\"") && UI_HTML.contains("id=\"sdetail-close\""),
        "index.html 缺检索详情面板/关闭按钮"
    );
    assert!(
        UI_JS.contains("$(\"sdetail-close\").onclick"),
        "关闭按钮必须接线"
    );
    let close_at = UI_JS
        .find("$(\"sdetail-close\").onclick")
        .expect("关闭接线存在");
    let close_seg = &UI_JS[close_at..];
    let close_end = close_seg.find(";").expect("关闭接线闭合");
    assert!(
        close_seg[..close_end].contains("display = \"none\"")
            && !close_seg[..close_end].contains("innerHTML"),
        "关闭详情只隐藏面板，不得动结果列表"
    );
    // 5) 整卡可点击样式（cursor:pointer，骨架行除外 + 键盘焦点态）。
    assert!(
        UI_CSS.contains(".hit:not(.skel-row) { cursor: pointer; }"),
        "命中卡缺 cursor:pointer（骨架行除外）"
    );
    assert!(UI_CSS.contains(".hit:focus-visible"), "命中卡缺键盘焦点态");
    // 6) 并发守卫（PR 161 评审 low）：面板级请求序号——迟到响应静默
    //    丢弃，快速连点两张命中卡时旧 asset_detail 响应不得覆盖面板。
    for needle in [
        "const seq = (detailSeq[panelId] = (detailSeq[panelId] || 0) + 1);",
        "if (seq !== detailSeq[panelId]) return;",
    ] {
        assert!(UI_JS.contains(needle), "showDetail 缺并发守卫：{needle}");
    }
    assert_eq!(
        UI_JS
            .matches("if (seq !== detailSeq[panelId]) return;")
            .count(),
        2,
        "并发守卫必须同时覆盖成功渲染与失败透出两条路径"
    );
    // 7) 详情列锁宽（PR 161 评审 low）：长副本路径不得把 340px 面板
    //    撑宽挤压结果列表。
    assert!(
        UI_CSS.contains(".s-stage .detail { min-width: 0; max-width: 340px; }"),
        "检索详情面板缺 min-width:0 / max-width 锁宽约束"
    );
}

/// 分区展示 + 跨通道不混排（§6-R2）：通道名分区标题、记忆行三字段
/// （content 截断 + tags + score）esc 全覆盖、资产行模板零记忆字段、
/// 双通道结果不合并数组（各自内部排序不变）。
#[test]
fn t03_flagship_memory_sections_and_no_cross_merge() {
    assert!(
        UI_JS.contains("资产通道") && UI_JS.contains("记忆通道 · memory_search"),
        "分区标题必须标明通道名（SPEC §2.3）"
    );
    // 记忆行 = content 截断 + tags + score（字符串动态字段一律 esc；
    // score 数字沿判例不入禁列）。
    assert!(
        UI_JS.contains("esc(trunc(m.content, 90))"),
        "记忆行 content 必须截断且经 esc"
    );
    assert!(
        UI_JS.contains("esc(memTags(m.tags))"),
        "记忆行 tags 必须经 memTags 解析 + esc"
    );
    assert!(
        UI_JS.contains("(m.score ?? 0).toFixed(2)"),
        "记忆行必须有 score 展示"
    );
    // 资产行模板区间（rows.map(h => { 起至首个闭合）零记忆字段——分区
    // 不混排的渲染面：资产 hit 卡现状不变。
    let asset_at = UI_JS.find("rows.map(h => {").expect("资产行渲染存在");
    let seg = &UI_JS[asset_at..];
    let close = seg.find("}).join(\"\")").expect("资产行渲染闭合");
    let asset_tpl = &seg[..close];
    for banned in ["mem.", "memTags", "memory"] {
        assert!(
            !asset_tpl.contains(banned),
            "资产行模板不得混入记忆通道字段（§6-R2 不混排）：{banned}"
        );
    }
    // 双通道结果不得合并成一个数组再排序（各自内部排序不变，§6-R2）。
    for banned in ["[...rows, ...mem", "rows.concat(mem", "mem.rows.concat("] {
        assert!(
            !UI_JS.contains(banned),
            "跨通道合并数组被禁用（score 不可比）：{banned}"
        );
    }
}

// ── M10-WP01-T04：检索过滤面静态契约（SPEC §2.4）——纯客户端维度：
// chips 每次结果渲染后从命中集 filename 派生（大小写归一；无扩展名/
// 孤儿行归「(无)」）；点击 chip 仅过滤已渲染命中（不重发查询、不触
// 后端）；meta 同步「显示 n / 共 m」；全不选 = 不过滤。 ──

/// chips 派生：extOf 从 filename 派生扩展名——大小写归一（toLowerCase）+
/// 无扩展名/孤儿行（filename 空）归「(无)」；chips 集合来源必须是命中集
/// filename（SPEC §2.4 字面）。
#[test]
fn t04_filter_chips_derive_from_filename_normalized() {
    assert!(
        UI_JS.contains("function extOf(filename) {"),
        "chips 派生函数 extOf 必须存在"
    );
    assert!(
        UI_JS.contains("return \"(无)\";"),
        "无扩展名/孤儿行必须归「(无)」（SPEC §2.4）"
    );
    let at = UI_JS
        .find("function extOf(filename) {")
        .expect("extOf 存在");
    let seg = &UI_JS[at..];
    let end = seg.find("\nfunction ").unwrap_or(seg.len());
    let body = &seg[..end];
    assert!(
        body.contains(".toLowerCase()"),
        "扩展名必须大小写归一（SPEC §2.4）"
    );
    assert!(
        UI_JS.contains("const e = extOf(h.filename);"),
        "chips 集合必须从命中集 filename 派生"
    );
}

/// 选中过滤仅影响展示：toggleExt = 切选中态 + 重渲（chips + 结果区），
/// 函数体内禁止任何 IPC / 重查询站点（SPEC §2.4「不重发查询、不触后端」）。
#[test]
fn t04_chip_toggle_filters_client_side_only() {
    let at = UI_JS
        .find("function toggleExt(ext) {")
        .expect("toggleExt 必须存在");
    let seg = &UI_JS[at..];
    let end = seg.find("\nfunction ").unwrap_or(seg.len());
    let body = &seg[..end];
    assert!(
        body.contains("selectedExts")
            && body.contains("renderChips();")
            && body.contains("renderSearchResults();"),
        "chip 点击 = 切换选中态 + 重渲 chips 与结果区"
    );
    for banned in ["invoke(", "call(", "doSearch(", "fetch("] {
        assert!(
            !body.contains(banned),
            "过滤不得触后端/重发查询（SPEC §2.4）：{banned}"
        );
    }
}

/// 过滤只作用于已渲染命中快照：filteredHits 空选中集原样返回 lastHits
/// （全不选 = 不过滤），非空时按 extOf 归一值过滤 lastHits。
#[test]
fn t04_filter_applies_to_rendered_hits_only() {
    let at = UI_JS
        .find("function filteredHits() {")
        .expect("filteredHits 必须存在");
    let seg = &UI_JS[at..];
    let end = seg.find("\nfunction ").unwrap_or(seg.len());
    let body = &seg[..end];
    assert!(
        body.contains("if (!selectedExts.size) return lastHits;"),
        "全不选 = 不过滤（SPEC §2.4）"
    );
    assert!(
        body.contains("lastHits.filter((h) => selectedExts.has(extOf(h.filename)))"),
        "过滤只能作用于已渲染命中快照 lastHits（客户端）"
    );
}

/// meta「显示 n / 共 m」同步 + 新查询/空查询/失败路径重置过滤选中态
/// （chips 必须反映当轮命中集，不得残留上一轮选中）。
#[test]
fn t04_meta_shown_over_total_and_state_reset() {
    assert!(
        UI_JS.contains("显示 <b>${rows.length}</b> / 共 ${total}"),
        "meta 必须同步「显示 n / 共 m」（SPEC §2.4）"
    );
    assert!(
        UI_JS.contains("资产 显示 <b>${rows.length}</b> / 共 ${total}"),
        "记忆同显时资产侧 meta 同样带「显示 n / 共 m」"
    );
    assert!(
        UI_JS.matches("selectedExts.clear();").count() >= 2,
        "新查询成功/空查询/失败路径都必须重置过滤选中态"
    );
}

/// chips 容器 DOM 对账：JS 引用的 #filter-chips 必须在 index.html 存在
/// 且带 role=group 可访问语义；chip 按钮带 aria-pressed 选中态；
/// chips 样式（默认态 + 选中态 + 空容器收纳）在 styles-v3.css 落地。
#[test]
fn t04_filter_chips_dom_and_style_parity() {
    assert!(
        UI_JS.contains("$(\"filter-chips\")"),
        "renderChips 必须渲染到 #filter-chips 容器"
    );
    assert!(
        UI_HTML.contains("id=\"filter-chips\""),
        "index.html 缺 #filter-chips 容器"
    );
    assert!(
        UI_HTML.contains("role=\"group\" aria-label=\"按扩展名过滤\""),
        "chips 容器必须带 group 语义 + 过滤用途 aria 标注"
    );
    assert!(
        UI_JS.contains("aria-pressed="),
        "chip 必须带 aria-pressed 选中态语义"
    );
    assert!(
        UI_JS.contains("b.onclick = () => toggleExt(b.dataset.ext);"),
        "chip 点击必须接线 toggleExt（GUI 实操实测漏绑 = chips 静态摆设）"
    );
    for needle in [".chip {", ".chip.on", ".chips:empty"] {
        assert!(
            UI_CSS.contains(needle),
            "styles-v3.css 缺 chips 样式：{needle}"
        );
    }
}

/// 检索 form 防整页重载锁（T04 GUI 实操发现的既有 bug 的回归锁）：
/// CSP `script-src 'self'` 必拦 inline `onsubmit`（形同虚设），form 默认
/// 提交 = 整页重载回 browse tab、检索结果全丢——chips 过滤面在 GUI 不可
/// 演示。锁：检索 form 区间零 `onsubmit=`/`type="submit"`；JS 侧 Enter
/// keydown preventDefault + form submit 兜底 preventDefault 双保险。
/// （记忆面板同名 form 模式为既有遗留，不在本卡范围——PR 正文披露。）
#[test]
fn t04_search_form_never_reloads_page() {
    let at = UI_HTML
        .find("<form class=\"query-row\"")
        .expect("检索 form 存在（检索 section 先于记忆 section）");
    let seg = &UI_HTML[at..];
    let end = seg.find("</form>").expect("检索 form 闭合");
    let form = &seg[..end];
    assert!(
        !form.contains("onsubmit="),
        "检索 form 禁 inline onsubmit（CSP script-src 'self' 必拦，形同虚设）"
    );
    assert!(
        !form.contains("type=\"submit\""),
        "检索 form 禁 submit 按钮（点击 = form 默认提交 = 整页重载丢结果）"
    );
    assert!(
        UI_JS.contains("if (e.key === \"Enter\") { e.preventDefault(); doSearch(); }"),
        "检索输入 Enter 必须 preventDefault 后再检索（防隐式提交重载）"
    );
    assert!(
        UI_JS.contains("addEventListener(\"submit\", (e) => e.preventDefault());"),
        "检索 form 必须 submit 兜底 preventDefault（CSP 下 inline onsubmit 不生效）"
    );
}

// ── M10-WP01-T05：检索空态引导静态契约（SPEC §2.5）——index_stats IPC
// 接线 + 三分支（初始态徽标 / 索引空 reindex 引导 / 有索引无命中既有
// 文案）；approx_count 近似值只做 0/>0 粗分支，不承诺精确（§6-R5）。 ──

/// 空态分支文案：索引空（docs==0 零命中）→ `partisync reindex` 引导
/// （CLI 命令直出）；有索引无命中 → 既有「换个更短的关键词 / 切语义」
/// 建议文案维持；GUI 内执行 reindex 按钮不存在（§4 非目标）。
#[test]
fn t05_empty_state_reindex_branch_and_no_hit_copy() {
    assert!(
        UI_JS.contains("indexDocs === 0"),
        "空态分支必须以 indexDocs===0 为索引空判据（SPEC §2.5）"
    );
    assert!(
        UI_JS.contains("全文索引还没有建立——运行 <b>partisync reindex</b> 建立内容索引"),
        "索引空必须明示 partisync reindex 引导（SPEC §2.5 字面）"
    );
    assert!(
        UI_JS.contains("换个更短的关键词"),
        "有索引无命中必须维持既有建议文案（SPEC §2.5 分支 3）"
    );
    assert!(
        UI_JS.contains("或切到「语义」模式放宽匹配"),
        "既有「切语义模式」建议文案不得回退"
    );
    assert!(
        !UI_HTML.contains("reindex-btn") && !UI_JS.contains("reindex("),
        "禁止 GUI 内执行 reindex（SPEC §4 非目标：仅文案引导）"
    );
}

/// index_stats 接线：经既有 call() 链路一次拉取缓存（含失败态）不轮询；
/// 初始态徽标「全文索引 N docs」仅在拉取成功时渲染；检索 tab 切换接线；
/// footer command 计数对账。
#[test]
fn t05_index_stats_badge_cached_and_no_polling() {
    assert_eq!(
        UI_JS.matches("call(\"index_stats\")").count(),
        1,
        "index_stats 调用站点必须唯一（loadIndexStats 内一次拉取）"
    );
    let at = UI_JS
        .find("async function loadIndexStats() {")
        .expect("loadIndexStats 必须存在");
    let seg = &UI_JS[at..];
    let end = ["\nasync function ", "\nfunction "]
        .iter()
        .filter_map(|m| seg.find(m))
        .min()
        .unwrap_or(seg.len());
    let body = &seg[..end];
    assert!(
        body.contains("if (indexStatsDone) return;") && body.contains("indexStatsDone = true;"),
        "loadIndexStats 必须缓存（含失败态）——一次拉取，不轮询（SPEC §2.5）"
    );
    assert!(
        body.contains("call(\"index_stats\")"),
        "loadIndexStats 必须经既有 call() 链路调 index_stats"
    );
    assert!(
        UI_JS.contains("async function renderSearchIdle() {"),
        "检索 tab 初始态渲染器必须存在"
    );
    assert!(
        UI_JS.contains("输入关键词或自然语言问题开始检索"),
        "初始态输入引导文案保留"
    );
    assert!(
        UI_JS.contains("全文索引 ${indexDocs} docs"),
        "初始态必须显示全文索引规模徽标（SPEC §2.5「全文索引 N docs」）"
    );
    assert!(
        UI_JS.contains("indexDocs !== null"),
        "未拉取/拉取失败（null）时徽标必须隐藏"
    );
    assert!(
        UI_JS.contains("if (t === \"search\") renderSearchIdle();"),
        "切到检索 tab 必须渲染初始态（索引规模可见）"
    );
    // 不进 5s 轮询（徽标为会话级快照；§6-R5 不承诺精确）
    let interval_at = UI_JS.find("setInterval").expect("轮询存在");
    let close = UI_JS[interval_at..]
        .find("}, 5000);")
        .expect("interval 闭合");
    assert!(
        !UI_JS[interval_at..interval_at + close].contains("index_stats"),
        "索引规模不得进 5s 轮询（一次拉取缓存）"
    );
    assert!(
        UI_HTML.contains("IPC 12 commands"),
        "footer IPC command 计数须与 generate_handler 对账（+index_stats = 12）"
    );
}

/// 裸插值禁列增补（M10-WP03-T02 详情面板动态字段）：content_id / mtime
/// 载荷插值一律经 esc()/timeFmt()/sizeFmt() 包裹，不得以 `${contentId`、
/// `${d.copies` 裸形态进 innerHTML 模板。
#[test]
fn t02_detail_no_raw_interpolation_of_detail_fields() {
    // "${d.copies[" 只禁对象/数组取值插值；`${d.copies.length}` 数字插值沿
    // score.toFixed 判例不入禁列。
    const BARE: &[&str] = &["${contentId", "${d.copies[", "${c.path", "${c.mtime_ns"];
    let mut leaked = Vec::new();
    for pat in BARE {
        if UI_JS.contains(pat) {
            leaked.push(*pat);
        }
    }
    assert!(
        leaked.is_empty(),
        "详情模板裸插值必须经 esc()/timeFmt()/sizeFmt() 包裹（M10-WP03 §2.2）：{leaked:?}"
    );
}

/// 详情面板 DOM 对账（沿 mem 面板判例）：JS 引用的 detail-* 元素必须在
/// index.html 存在（防 id 漂移）；detail-body 为唯一渲染容器。
#[test]
fn t02_detail_dom_parity() {
    assert!(
        UI_HTML.contains("id=\"detail-panel\""),
        "index.html 缺 #detail-panel"
    );
    assert!(
        UI_HTML.contains("id=\"detail-close\""),
        "index.html 缺 × 静态头按钮（detail-head，面板内容重渲不丢）"
    );
    assert!(
        UI_HTML.contains("aria-label=\"关闭详情\""),
        "× 按钮必须带可访问标注（SPEC §2.2 aria-label 契约）"
    );
    assert!(
        UI_HTML.contains("id=\"detail-body\""),
        "index.html 缺 #detail-body 内容容器"
    );
    assert!(
        UI_JS.contains("$(\"detail-body\")"),
        "showDetail 必须渲染到 #detail-body（× 静态头不被 innerHTML 重写）"
    );
}

/// 关闭双通道 + 硬约束（SPEC §2.2）：× 按钮 + Esc 接线；closeDetail 函数
/// 体 = display:none + 内容清空，且零 call/invoke/browse、零列表（#rows）
/// 触碰——关闭不清空/重载浏览列表、零重发 IPC；再点行重开沿既有行 onclick。
#[test]
fn t02_detail_close_dual_channel_and_zero_requery() {
    assert!(
        UI_JS.contains("$(\"detail-close\").onclick = closeDetail;"),
        "× 必须接线 closeDetail（CSP 禁 inline onclick）"
    );
    assert!(
        UI_JS.contains("document.addEventListener(\"keydown\", escClose);"),
        "Esc 必须接线 escClose（双通道关闭）"
    );
    let at = UI_JS
        .find("function closeDetail() {")
        .expect("closeDetail 必须存在");
    let seg = &UI_JS[at..];
    let end = seg.find("\nfunction ").unwrap_or(seg.len());
    let body = &seg[..end];
    assert!(
        body.contains("panel.style.display = \"none\";"),
        "关闭 = 面板 display:none（SPEC §2.2）"
    );
    assert!(
        body.contains("innerHTML = \"\";"),
        "关闭 = 面板内容清空（SPEC §2.2）"
    );
    for banned in ["call(", "invoke(", "browse(", "$(\"rows\")"] {
        assert!(
            !body.contains(banned),
            "关闭不得清空/重载浏览列表或重发 IPC（SPEC §2.2 硬约束）：{banned}"
        );
    }
}

/// Esc 可见性门控（SPEC §2.2 + §7-R4）：仅面板可见时生效；输入框聚焦
/// （INPUT/TEXTAREA）不拦截，不抢输入框焦点语义。
#[test]
fn t02_esc_close_gated_on_visibility_and_input_focus() {
    let at = UI_JS
        .find("function escClose(e) {")
        .expect("escClose 必须存在");
    let seg = &UI_JS[at..];
    let end = seg.find("\nfunction ").unwrap_or(seg.len());
    let body = &seg[..end];
    assert!(
        body.contains("e.key !== \"Escape\"") && body.contains("return;"),
        "Esc 键值判据 + 不可见时直接交还"
    );
    assert!(
        body.contains("panel.style.display === \"none\""),
        "可见性门控：面板不可见时 Esc 不生效（SPEC §2.2）"
    );
    assert!(
        body.contains("INPUT") && body.contains("TEXTAREA"),
        "输入框聚焦时 Esc 不拦截（SPEC §7-R4）"
    );
    assert!(
        body.contains("closeDetail();"),
        "门控通过后走 closeDetail 单点关闭"
    );
}

/// 信息补全（SPEC §2.2，零 IPC 变更）：「修改时间」行 = 首副本
/// `copies[0].mtime_ns`（与 size 取 rows.first() 同口径）；mtime_ns 缺失/0
/// → 「—」回落（timeFmt 既有判据）；加载中途关闭 → 回包弃渲（可见性门控）。
#[test]
fn t02_detail_mtime_row_first_copy_with_dash_fallback() {
    assert!(
        UI_JS.contains("<dt>修改时间</dt><dd>${timeFmt(d.copies[0]?.mtime_ns)}</dd>"),
        "修改时间行必须取首副本 copies[0].mtime_ns（SPEC §2.2，载荷已有字段零 IPC 变更）"
    );
    assert!(
        UI_JS.contains("if (!ns) return \"—\";"),
        "mtime_ns 缺失/0 → 「—」回落（timeFmt 既有判据不得回退）"
    );
    assert!(
        UI_JS
            .matches("if (panel.style.display === \"none\") return;")
            .count()
            >= 2,
        "可见性门控 ≥2 处：escClose + showDetail 回包弃渲（加载中途关闭不回填已关面板）"
    );
}

/// 复制（SPEC §2.2 + §7-R2）：完整 content_id + 每条副本路径两站点
/// data-copy-text（数据源 = 原始值全量，不抄 DOM 展示切片）；
/// navigator.clipboard.writeText 主路径 + execCommand 回落；成功反馈 =
/// 「已复制」1.5s 回落 + 防重入（原始 label 持久存 dataset + clearTimeout
/// 旧 timer，沿 M10-WP02 §2.2 / PR #166 F3 判例）。
#[test]
fn t02_copy_full_cid_and_copies_with_feedback_and_fallback() {
    assert!(
        UI_JS.contains("data-copy-text=\"${esc(contentId)}\""),
        "content_id 全量复制站点（64 hex 全量，替代「抄 DevTools」）"
    );
    assert!(
        UI_JS.contains("data-copy-text=\"${esc(c.path)}\""),
        "副本路径复制站点（每条副本路径一枚）"
    );
    assert!(
        UI_JS.matches("class=\"btn ghost copy-btn\"").count() >= 2,
        "两处复制按钮站点（content_id + 副本路径）"
    );
    assert!(
        UI_JS.contains("if (navigator.clipboard?.writeText) {")
            && UI_JS.contains(
                "navigator.clipboard.writeText(text).then(done).catch(() => detailCopyFallback(text, done))"
            ),
        "主路径 writeText（可用性检查 + 调用）+ 回落接线（SPEC §7-R2 实现期拍板）"
    );
    let at = UI_JS
        .find("function detailCopyFallback(text, done) {")
        .expect("execCommand 回落必须存在");
    let seg = &UI_JS[at..];
    let end = ["\nfunction ", "\nasync function "]
        .iter()
        .filter_map(|m| seg.find(m))
        .min()
        .unwrap_or(seg.len());
    assert!(
        seg[..end].contains("execCommand(\"copy\")"),
        "回落 = 隐藏 textarea + execCommand（SPEC §7-R2）"
    );
    let at = UI_JS
        .find("function detailCopy(btn, text) {")
        .expect("复制助手必须存在");
    let seg = &UI_JS[at..];
    let end = seg.find("\nfunction ").unwrap_or(seg.len());
    let body = &seg[..end];
    assert!(
        body.contains("已复制") && body.contains("1500"),
        "成功反馈 = 按钮文案瞬变「已复制」+ 1.5s（1500ms）回落"
    );
    assert!(
        body.contains("clearTimeout(Number(btn.dataset.copyTimer))")
            && body.contains("btn.dataset.copyLabel"),
        "复制反馈必须防重入：clearTimeout 旧 timer + dataset.copyLabel 持久保留原始 label"
    );
}

// ── M10-WP03-T03：浏览列表 UX 静态契约（SPEC §2.3）——表头客户端排序
// （名称/大小/修改时间三列 / 回服务端序 / 目录优先次级键 / 换目录重置）
// + mtime 相对时间（>30 天回落绝对日期 + title 完整时间 + 「—」回落）。 ──

/// 三列接线 + 三态循环 + 目录优先次级键 + 换目录重置（SPEC §2.3）：
/// 点击按当前渲染行集排序、再点反序、第三点回服务端序（children() 返回
/// 序，ipc.rs:90-96）；排序作用于 lastRows 副本（点击重渲不重发 IPC）；
/// 键相同时目录行（kind=1）恒在文件行前（kind 比较不乘 dir，双向成立）；
/// 换目录（面包屑/目录行导航）后排序态重置默认（沿 M10-WP02 T01 判例）。
#[test]
fn t03_browse_sort_cycle_dirs_first_and_path_reset() {
    // 三列接线：index.html 三列 data-sort th + th-sort 按钮（data-key 一一
    // 对应）+ JS 绑定（CSP 禁 inline onclick）；内容身份列不设 data-sort。
    for key in ["name", "size", "mtime"] {
        assert!(
            UI_HTML.contains(&format!("th data-sort=\"{key}\"")),
            "表头缺 data-sort={key} 可排序列"
        );
        assert!(
            UI_HTML.contains(&format!("class=\"th-sort\" data-key=\"{key}\"")),
            "{key} 列缺 th-sort 排序按钮"
        );
    }
    assert_eq!(
        UI_HTML.matches("th data-sort=").count(),
        3,
        "可排序列必须恰为三列（名称/大小/修改时间；内容身份列除外，SPEC §2.3）"
    );
    assert!(
        UI_JS.contains("b.onclick = () => cycleSort(b.dataset.key);"),
        "th-sort 按钮必须接线 cycleSort（CSP 禁 inline onclick）"
    );
    // 三态循环：asc → desc → null（回服务端序）；循环体零 IPC 站点。
    let at = UI_JS
        .find("function cycleSort(key) {")
        .expect("cycleSort 必须存在");
    let seg = &UI_JS[at..];
    let end = seg.find("\nfunction ").unwrap_or(seg.len());
    let body = &seg[..end];
    assert!(
        body.contains("if (sortKey !== key) { sortKey = key; sortDir = 1; }"),
        "首点新键 = 升序"
    );
    assert!(
        body.contains("else if (sortDir === 1) sortDir = -1;"),
        "再点同键 = 反序（降序）"
    );
    assert!(
        body.contains("sortKey = null; sortDir = 1; }") && body.contains("renderRows();"),
        "第三点回服务端序并重渲当前行集（SPEC §2.3）"
    );
    for banned in ["call(", "invoke("] {
        assert!(
            !body.contains(banned),
            "排序点击不得重发 IPC（纯客户端重渲）：{banned}"
        );
    }
    // 排序作用于 lastRows 副本（不 mutate 服务端序快照；null = 原样返回）。
    let at = UI_JS
        .find("function sortedRows() {")
        .expect("sortedRows 必须存在");
    let seg = &UI_JS[at..];
    let end = seg.find("\nfunction ").unwrap_or(seg.len());
    let body = &seg[..end];
    assert!(
        body.contains("if (!sortKey) return lastRows;"),
        "未排序 = 原样返回服务端序（children() 返回序）"
    );
    assert!(
        body.contains("[...lastRows].sort("),
        "排序必须作用于 lastRows 副本（服务端序快照不可变）"
    );
    assert!(
        body.contains("return (b.kind ?? 0) - (a.kind ?? 0);"),
        "排序键相同时目录行（kind=1）恒在文件行前（kind 比较不乘 dir，双向成立）"
    );
    assert!(body.contains("localeCompare"), "名称列按字典序比较");
    assert!(
        body.contains("(a.size ?? 0) - (b.size ?? 0)")
            && body.contains("(a.mtime_ns ?? 0) - (b.mtime_ns ?? 0)")
            && body.contains("c * dir"),
        "大小/修改时间数值比较 + 方向乘子"
    );
    // 换目录重置：browse 内路径变化才重置（同目录轮询刷新保留排序态）。
    let at = UI_JS
        .find("async function browse(path) {")
        .expect("browse 必须存在");
    let seg = &UI_JS[at..];
    let end = seg.find("\nfunction ").unwrap_or(seg.len());
    let body = &seg[..end];
    assert!(
        body.contains("if (next !== curPath) { sortKey = null; sortDir = 1; renderSortArrows(); }"),
        "换目录（面包屑/目录行导航）必须重置排序态为默认（SPEC §2.3）"
    );
    assert!(
        body.contains("lastRows = rows;"),
        "list 结果必须快照进 lastRows（排序点击重渲不重发 IPC）"
    );
}

/// 箭头指示（SPEC §2.3）：renderSortArrows = 全清后再设（切换键/回默认
/// 不残留旧箭头）；当前键 ▲/▼ 标方向 + th aria-sort 同步（可访问语义）。
#[test]
fn t03_browse_sort_arrow_indicators() {
    let at = UI_JS
        .find("function renderSortArrows() {")
        .expect("renderSortArrows 必须存在");
    let seg = &UI_JS[at..];
    let end = seg.find("\nfunction ").unwrap_or(seg.len());
    let body = &seg[..end];
    assert!(
        body.contains("textContent = \"\";"),
        "先全清箭头再设（键切换/回默认不残留旧指示）"
    );
    assert!(
        body.contains("\"▲\" : \"▼\""),
        "箭头指示当前排序键与方向（SPEC §2.3）"
    );
    assert!(
        body.contains("ascending") && body.contains("descending"),
        "th aria-sort 同步排序方向（可访问语义）"
    );
}

/// mtime 相对时间（SPEC §2.3）：mtimeDisp 复用 relTime；>30 天回落绝对
/// 日期（本卡自含实现，不依赖 M10-WP02-T03 落地顺序）；缺失/0 → 既有
/// 「—」回落；行模板 title 悬浮 = timeFmt 完整本地时间（不回退）。
#[test]
fn t03_mtime_relative_30day_absolute_fallback_and_title() {
    let at = UI_JS
        .find("function mtimeDisp(ns) {")
        .expect("mtimeDisp 必须存在");
    let seg = &UI_JS[at..];
    let end = seg.find("\nfunction ").unwrap_or(seg.len());
    let body = &seg[..end];
    assert!(
        body.contains("if (!ns) return \"—\";"),
        "mtime_ns 缺失/0 → 「—」回落（timeFmt 既有判据不得回退）"
    );
    assert!(
        body.contains("30 * 86400"),
        ">30 天回落档判据（SPEC §2.3 字面）"
    );
    assert!(
        body.contains("relTime(ns)"),
        "≤30 天复用既有 relTime 相对显示"
    );
    assert!(body.contains("toLocaleDateString("), ">30 天回落绝对日期");
    assert!(
        UI_JS.contains(
            "<td class=\"mtime\" title=\"${esc(timeFmt(e.mtime_ns))}\">${mtimeDisp(e.mtime_ns)}</td>"
        ),
        "mtime 列 = title 完整 timeFmt 本地时间（不回退）+ mtimeDisp 相对显示"
    );
}

/// 空态不回退（SPEC §2.3）：browse 空态文案（D6 动作邀请修复）原文不动。
#[test]
fn t03_browse_empty_state_copy_untouched() {
    assert!(
        UI_JS.contains("本机还没有索引文件——运行 <b>partisync index &lt;路径&gt;</b> 开始建立索引"),
        "根目录空态文案必须维持原文（D6 动作邀请）"
    );
    assert!(
        UI_JS.contains(": \"空目录\""),
        "非根目录空态文案必须维持原文"
    );
}

/// 排序 DOM/样式对账（沿 chips 判例）：th-sort 三按钮 + 箭头 span 在
/// index.html；.th-sort / .sort-arrow 样式在 styles-v3.css 落地。
#[test]
fn t03_sort_dom_and_style_parity() {
    assert_eq!(
        UI_HTML.matches("class=\"th-sort\"").count(),
        3,
        "th-sort 按钮必须恰为三枚"
    );
    assert_eq!(
        UI_HTML.matches("class=\"sort-arrow\"").count(),
        3,
        "箭头指示 span 必须每列一枚"
    );
    for needle in [".th-sort {", ".sort-arrow {"] {
        assert!(
            UI_CSS.contains(needle),
            "styles-v3.css 缺排序指示样式：{needle}"
        );
    }
}
