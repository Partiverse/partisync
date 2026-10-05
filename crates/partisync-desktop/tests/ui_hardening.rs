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

/// M10-WP01-T01：命中卡 name 行优先透出 graph 文件名（「GUI 搜索形同
/// 虚设」修复的 UI 面——filename 缺失才回落哈希切片）。
#[test]
fn hit_card_prefers_graph_filename() {
    assert!(
        UI_JS.contains("${h.filename ? esc(h.filename) : esc(h.content_id.slice(0, 8)) + \"…\"}"),
        "命中卡 name 行必须优先 filename（回落哈希切片）"
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

const UI_CSS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/ui/styles-v3.css"));

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

// ── M10-WP02-T01：记忆 form 防重载 + 表头客户端排序（SPEC §2.1）——
// 清偿上方 t04 注释登记的「记忆面板同名 form 遗留」债（PR #162 披露源）。 ──

/// 记忆 form 防整页重载锁（`t04_search_form_never_reloads_page` 同款扩面）：
/// CSP `script-src 'self'` 必拦 inline `onsubmit`（原 index.html 死代码，
/// 形同虚设），submit 按钮默认提交 / mem-q Enter 隐式提交 = 整页刷回
/// browse tab、列表全丢。锁：记忆 form 区间零 `onsubmit=` / 零
/// `type="submit"`；JS 侧 mem-q Enter preventDefault + 记忆 form submit
/// 兜底 preventDefault 双保险。
#[test]
fn t01_memory_form_never_reloads_page() {
    let at = UI_HTML
        .find("id=\"view-memory\"")
        .expect("记忆 section 存在");
    let seg = &UI_HTML[at..];
    let form_at = seg
        .find("<form class=\"query-row\"")
        .expect("记忆 form 存在（view-memory section 内首个 query-row）");
    let seg = &seg[form_at..];
    let end = seg.find("</form>").expect("记忆 form 闭合");
    let form = &seg[..end];
    assert!(
        !form.contains("onsubmit="),
        "记忆 form 禁 inline onsubmit（CSP script-src 'self' 必拦，形同虚设——登记债清偿）"
    );
    assert!(
        !form.contains("type=\"submit\""),
        "记忆 form 禁 submit 按钮（点击 = form 默认提交 = 整页刷回 browse tab）"
    );
    assert!(
        UI_JS.contains(
            "$(\"mem-q\").addEventListener(\"keydown\", (e) => { if (e.key === \"Enter\") { e.preventDefault(); loadMemories(); } });"
        ),
        "mem-q Enter 必须 preventDefault 后再检索（防隐式提交整页重载）"
    );
    assert!(
        UI_JS.contains(
            "document.querySelector(\"#view-memory form.query-row\").addEventListener(\"submit\", (e) => e.preventDefault());"
        ),
        "记忆 form 必须 submit 兜底 preventDefault（CSP 下 inline onsubmit 不生效；覆盖 mem-tag Enter 隐式提交）"
    );
}

/// 表头客户端排序接线（SPEC §2.1）：四可排序列（创建时间/score/tags/
/// 来源设备；内容列除外——截断展示排序意义弱）+ 点击排序当前快照
/// （slice 副本，不重发查询、不触后端）+ 三态循环（首点升序/再点反序/
/// 第三点回默认服务端序）+ tags 键经 memTags() 解析（非 canonical JSON
/// 原串）+ 箭头指示当前排序键与方向 + 新检索/失败路径重置排序态。
#[test]
fn t01_memory_table_header_sort_wired() {
    // 四可排序列接线：th data-mem-sort 恰四列 + memSortValue 四键覆盖。
    for key in ["created", "score", "tags", "device"] {
        assert!(
            UI_HTML.contains(&format!("data-mem-sort=\"{key}\"")),
            "记忆表头缺可排序列 {key}"
        );
    }
    assert_eq!(
        UI_HTML.matches("data-mem-sort=").count(),
        4,
        "可排序列必须恰四列（内容列除外，SPEC §2.1）"
    );
    assert!(
        UI_JS.contains("if (key === \"created\") return m.created_ns ?? 0;")
            && UI_JS.contains("if (key === \"score\") return m.score ?? 0;")
            && UI_JS.contains("if (key === \"tags\") return memTags(m.tags);")
            && UI_JS.contains("return String(m.origin_device ?? \"\");"),
        "memSortValue 必须覆盖四键；tags 键 = memTags() 解析后逗号列表字典序（非 JSON 原串）"
    );
    // 排序只作用于已渲染窗口快照：slice() 副本 + 客户端比较器，函数体内
    // 零 IPC 站点（沿 T04 chips「不重发查询、不触后端」判例）。
    let at = UI_JS
        .find("function sortedMemRows() {")
        .expect("sortedMemRows 必须存在");
    let seg = &UI_JS[at..];
    let end = seg.find("\nfunction ").unwrap_or(seg.len());
    let body = &seg[..end];
    assert!(
        body.contains("if (!memSort.key) return memRows;"),
        "默认态必须原样返回服务端序快照（第三点回默认语义）"
    );
    assert!(
        body.contains("memRows.slice().sort("),
        "排序必须作用于快照副本（不重排服务端原始序数组）"
    );
    for banned in ["invoke(", "call(", "fetch("] {
        assert!(
            !body.contains(banned),
            "排序不得触后端/重发查询（客户端窗口序，SPEC §2.1）：{banned}"
        );
    }
    // 三态循环：首点升序 → 再点反序 → 第三点回默认（服务端序）。
    let at = UI_JS
        .find("function memSortClick(key) {")
        .expect("memSortClick 必须存在");
    let seg = &UI_JS[at..];
    let end = seg.find("\nfunction ").unwrap_or(seg.len());
    let body = &seg[..end];
    assert!(
        body.contains("memSort = memSort.key !== key ? { key, dir: 1 }"),
        "首点必须升序（dir=1）"
    );
    assert!(
        body.contains("memSort.dir === 1 ? { key, dir: -1 }"),
        "再点必须反序（dir=-1）"
    );
    assert!(
        body.contains("{ key: null, dir: 1 };"),
        "第三点必须回默认（服务端序，key=null）"
    );
    // 箭头指示 + aria-sort（当前排序键与方向透出）+ CSS 指示样式落地。
    assert!(
        UI_JS.contains("function renderMemSortHeads() {")
            && UI_JS.contains(
                "th.querySelector(\".mem-sort-arrow\").textContent = on ? (memSort.dir === 1 ? \"↑\" : \"↓\") : \"\";"
            )
            && UI_JS.contains("aria-sort"),
        "表头必须箭头指示当前排序键与方向 + aria-sort 同步"
    );
    assert!(
        UI_CSS.contains(".mem-sort-arrow") && UI_CSS.contains("th[data-mem-sort]"),
        "styles-v3.css 缺排序指示样式（可点击 + 激活态 + 箭头）"
    );
    // 表头点击接线 + 新检索/换 query/换 tag 重置（成功与失败路径都重置）。
    assert!(
        UI_JS.contains("th.onclick = () => memSortClick(th.dataset.memSort);"),
        "表头点击必须接线 memSortClick（GUI 实操判例：漏绑 = 静态摆设）"
    );
    assert!(
        UI_JS.matches("memSort = { key: null, dir: 1 };").count() >= 2,
        "loadMemories 成功/失败路径都必须重置排序态（新检索/换 query/tag → 服务端默认序）"
    );
}
