// PartiSync Desktop 前端 v3（设计语言 v4.3 落地——旧薄面弃用重写）。
// M8-WP05-T01：语义检索旗舰（search_hybrid + 三态）。
// 绑定面：index.html（v4.3 结构：tally 仪表 / hit 卡 / section-tag）。
// 数据走 12 个 Tauri command（src/ipc.rs）：
//   get_stats / list / search / search_hybrid / index_stats / asset_detail /
//   cas_stats / duplicates / jobs / sync_stats / sync_recent / mcp_call
// 文件名 app-core-v3.js：#39 判例（WKWebView 缓存击穿靠改名）。

const __tauriCore = window.__TAURI__?.core;
if (!__tauriCore) {
  document.body.innerHTML =
    '<div style="padding:40px;font-family:var(--mono);color:#e06c55">' +
    "Tauri API 未注入：桌面壳 IPC 不可用（请确认以桌面壳方式启动，" +
    "且 tauri.conf.json 的 app.withGlobalTauri 为 true）。</div>";
  throw new Error("Tauri IPC bridge unavailable");
}
const { invoke } = __tauriCore;

// 全局 JS 异常 → #ext-debug（T04 GUI 诊断；保留为轻量错误面）。
window.addEventListener("error", (e) => {
  const d = document.getElementById("ext-debug");
  if (d) d.textContent = `[JS-ERR] ${e.message} @${e.filename}:${e.lineno}\n` + d.textContent;
});

let curPath = "/";
let searchMode = "bm25"; // bm25 = 关键词；hybrid = 语义（T01 旗舰）；transcript = 含转写（M9-WP03-T01 接线）
const $ = (id) => document.getElementById(id);

// M9-WP03-T01（SPEC §2.1）：innerHTML 动态插值一律经 esc()——CSP 已挡
// inline 脚本执行，转义防 markup 破格与属性逃逸（data-path/data-cid 等）。
function esc(s) {
  return String(s ?? "")
    .replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;").replaceAll("'", "&#39;");
}

function sizeFmt(n) {
  const u = ["B","KB","MB","GB","TB"]; let v = n, i = 0;
  while (v >= 1024 && i < 4) { v /= 1024; i++; }
  return i === 0 ? `${n} B` : `${v.toFixed(1)} ${u[i]}`;
}

function timeFmt(ns) {
  if (!ns) return "—";
  const d = new Date(ns / 1e6);
  return d.toLocaleString("zh-CN", { hour12: false });
}

async function call(cmd, args) {
  try {
    return await invoke(cmd, args === undefined ? {} : { args });
  } catch (e) {
    const kind = e?.kind ?? "Internal";
    const msg = e?.msg ?? String(e);
    showError(`[${kind}] ${msg}`);
    throw e;
  }
}

function showError(text) {
  const el = $("error-region");
  el.textContent = text;
  el.style.display = "";
  setTimeout(() => { el.style.display = "none"; el.textContent = ""; }, 5000);
}

// ── A. 统计仪表 ──
async function loadStats() {
  const [s, cas] = await Promise.all([call("get_stats"), call("cas_stats")]);
  $("st-files").textContent = s.files;
  $("st-dirs").textContent = s.dirs;
  $("st-bytes").textContent = sizeFmt(s.total_bytes);
  $("st-unique").textContent = s.unique_contents;
  $("st-saved").textContent = sizeFmt(s.saved_bytes);
  $("st-chunks").textContent = cas.chunks;
}

// ── B. 浏览 ──
function breadcrumbFor(path) {
  if (path === "/") return [{ name: "~", path: "/" }];
  const parts = path.split("/").filter(Boolean);
  const out = [{ name: "~", path: "/" }];
  let acc = "";
  for (const p of parts) {
    acc += "/" + p;
    out.push({ name: p, path: acc });
  }
  return out;
}

function fpOf(contentId) {
  // 指纹色派生（设计 §2 tokens：H = 首字节 × 137.508° 黄金角散布）
  if (!contentId || contentId.length < 2) return "var(--green-dim)";
  const b = parseInt(contentId.slice(0, 2), 16);
  const h = Math.round((b * 137.508) % 360);
  return `hsl(${h} 55% 55%)`;
}

async function browse(path) {
  curPath = path || "/";
  const rows = await call("list", { prefix: curPath });
  const crumbs = breadcrumbFor(curPath);
  $("crumbs").innerHTML = crumbs.map((e, i) =>
    i === crumbs.length - 1
      ? `<b>${esc(e.name)}</b>`
      : `<a href="#" data-path="${esc(e.path)}">${esc(e.name)}</a>`
  ).join(`<span>▸</span>`);
  $("crumbs").querySelectorAll("a[data-path]").forEach(a => {
    a.onclick = () => browse(a.dataset.path);
  });
  $("rows").innerHTML = rows.length ? rows.map(e => {
    const dir = e.kind === 1;
    return `<tr class="${dir ? "row-dir" : "row-file"}"${dir ? ` data-path="${esc(e.path)}" style="cursor:pointer"` : ` data-cid="${esc(e.content_id)}" data-name="${esc(e.name)}" style="cursor:pointer"`}${e.content_id ? ` style="--fp: ${fpOf(e.content_id)}"` : ""}>
      <td class="icon" aria-hidden="true">${dir ? "▸" : "·"}</td>
      <td>${esc(e.name)}</td><td class="size">${dir ? "—" : sizeFmt(e.size)}</td>
      <td class="mtime">${timeFmt(e.mtime_ns)}</td>
      <td class="fp"${e.content_id ? ` style="--fp: ${fpOf(e.content_id)}"` : ""}>${e.content_id ? "<i></i>" + esc(e.content_id.slice(0, 8)) : "—"}</td></tr>`;
  }).join("") : `<tr><td colspan="5" class="empty">${curPath === "/" ? "本机还没有索引文件——运行 <b>partisync index &lt;路径&gt;</b> 开始建立索引" : "空目录"}</td></tr>`;
  $("rows").querySelectorAll("tr[data-path]").forEach(tr => {
    tr.onclick = () => browse(tr.dataset.path);
  });
  // 文件行点击 → 详情面板（M8-WP05-T02；dir 行保持进目录）
  $("rows").querySelectorAll("tr.row-file[data-cid]").forEach(tr => {
    tr.onclick = () => showDetail(tr.dataset.cid, tr.dataset.name);
  });
}

// ── B2. 条目详情面板（M8-WP05-T02；SPEC §2.2） ──
async function showDetail(contentId, name) {
  const panel = $("detail-panel");
  panel.style.display = "";
  panel.innerHTML = `<div class="section-tag">detail</div>
    <div class="empty">加载中…</div>`;
  let d;
  try {
    d = await call("asset_detail", { prefix: contentId });
  } catch (e) {
    panel.innerHTML = `<div class="empty">详情加载失败（${esc(e?.kind ?? "?")}）</div>`;
    return;
  }
  const fp = fpOf(contentId);
  const copies = d.copies.map(c =>
    `<li>${esc(c.path)} <span class="dim">· ${sizeFmt(c.size)}</span></li>`).join("");
  panel.innerHTML = `
    <h2>${esc(name)}</h2>
    <dl>
      <dt>大小</dt><dd>${sizeFmt(d.size)}</dd>
      <dt>副本</dt><dd>${d.copies.length} 处</dd>
    </dl>
    <div class="fingerprint" style="--fp: ${fp}">
      <div class="label">内容身份（blake3）——指纹色由此派生</div>
      <div class="strip">${Array.from({length: 8}, (_, i) =>
        `<i style="background: hsl(${(i * 47 + parseInt(contentId.slice(0, 2), 16) * 137.508) % 360} 55% 55%)"></i>`).join("")}</div>
      <div class="hash">${contentId.slice(0, 16)}…</div>
    </div>
    <div class="copies"><div class="label">副本路径</div><ul>${copies || "<li>—</li>"}</ul></div>`;
}

// ── C. 检索（旗舰；三态全覆盖——设计审计硬约束） ──
function searchCommand() {
  // M9-WP03-T01（SPEC §2.1 N4 开关化）：「含转写文本」radio 显式传
  // include_transcript=true；关键词/语义不传（None = 后端常开现状语义）。
  if (searchMode === "hybrid") return "search_hybrid";
  return "search"; // bm25 与 transcript 同走 BM25 通道
}

function searchArgs(q) {
  const args = { q, limit: 50 };
  if (searchMode === "transcript") args.include_transcript = true;
  return args;
}

function searchSkeleton(n) {
  return Array.from({ length: n }, (_, i) =>
    `<article class="hit skel-row"><div class="skel skel-fp"></div>
     <div class="col"><div class="skel" style="height:13px;width:${45 - i * 5}%"></div>
     <div class="skel" style="height:10px;width:${70 - i * 4}%;margin-top:7px"></div></div></article>`
  ).join("");
}

// ── C3. 空态引导（M10-WP01-T05；SPEC §2.5）——index_stats 一次拉取缓存
// （含失败态，不轮询）；approx_count 为 reader 快照近似值，UI 只做 0/>0
// 粗分支 + 规模徽标，不承诺精确计数（§6-R5）。拉取失败 → 徽标隐藏，
// 检索不受阻。
let indexDocs = null; // null = 未拉取或拉取失败（徽标隐藏）
let indexStatsDone = false;

async function loadIndexStats() {
  if (indexStatsDone) return;
  indexStatsDone = true;
  try { indexDocs = (await call("index_stats")).docs; } catch { indexDocs = null; }
}

// 检索 tab 初始态（未输入查询）：输入引导 + 全文索引规模徽标。已有查询
// 结果（lastQ 非空）不覆盖——切 tab 回来检索结果原样保留。徽标样式内联
// （styles-v3.css 不在本卡文件清单；JS 内联样式沿 skeleton/error 判例）。
async function renderSearchIdle() {
  await loadIndexStats();
  if (lastQ) return;
  $("srows").innerHTML = `<div class="empty">输入关键词或自然语言问题开始检索${
    indexDocs !== null ? `<div class="idx-badge" style="display:inline-block;margin-top:12px;font-family:var(--mono);font-size:11px;line-height:1;padding:4px 9px;border:1px solid var(--green-dim);border-radius:999px;color:var(--green);background:var(--green-dark)" title="全文索引近似规模（reader 快照）">全文索引 ${indexDocs} docs</div>` : ""
  }</div>`;
}

// ── C2. 检索过滤面（M10-WP01-T04；SPEC §2.4）——纯客户端维度：chips
// 每次结果渲染后从命中集 filename 派生（大小写归一；无扩展名/孤儿行归
// 「(无)」）；点击 chip 仅过滤已渲染命中（不重发查询、不触后端）；meta
// 同步「显示 n / 共 m」；全不选 = 不过滤。诚实边界：tags 生产恒空、
// mime 未入索引 schema——不做空维度过滤面（SPEC §4 非目标登记）。
let lastQ = "";
let lastHits = []; // 最近一次资产检索命中（原始序、未过滤快照）
let lastMem = null; // 记忆通道快照（{rows, error} | null）
let lastModeLabel = "";
let selectedExts = new Set();

function extOf(filename) {
  const base = String(filename ?? "").split("/").pop() || "";
  const dot = base.lastIndexOf(".");
  if (dot <= 0 || dot === base.length - 1) return "(无)";
  return base.slice(dot + 1).toLowerCase();
}

function filteredHits() {
  if (!selectedExts.size) return lastHits;
  return lastHits.filter((h) => selectedExts.has(extOf(h.filename)));
}

function renderChips() {
  const exts = [];
  for (const h of lastHits) {
    const e = extOf(h.filename);
    if (!exts.includes(e)) exts.push(e);
  }
  const box = $("filter-chips");
  box.innerHTML = exts.length
    ? `<span class="chips-cap">按扩展名过滤</span>` + exts.map((e) =>
        `<button type="button" class="chip${selectedExts.has(e) ? " on" : ""}" data-ext="${esc(e)}" aria-pressed="${selectedExts.has(e) ? "true" : "false"}">${esc(e)}</button>`
      ).join("")
    : "";
  box.querySelectorAll("button.chip").forEach((b) => {
    b.onclick = () => toggleExt(b.dataset.ext);
  });
}

function toggleExt(ext) {
  if (selectedExts.has(ext)) selectedExts.delete(ext);
  else selectedExts.add(ext);
  renderChips();
  renderSearchResults();
}

function renderSearchResults() {
  const meta = $("ssearch-meta");
  const rows = filteredHits();
  const total = lastHits.length;
  // meta（T04：资产侧同步「显示 n / 共 m」；记忆侧计数不受过滤影响）。
  meta.innerHTML = lastMem
    ? ((total || lastMem.rows.length)
      ? `资产 显示 <b>${rows.length}</b> / 共 ${total} · 记忆 <b>${lastMem.rows.length}</b> hits · ${lastModeLabel}+记忆`
      : "")
    : (total ? `显示 <b>${rows.length}</b> / 共 ${total} hits · ${lastModeLabel}` : "");
  // 资产分区：过滤仅影响展示；过滤致空 ≠ 无结果（给恢复引导，不清 chips）。
  const assetRows = rows.length ? rows.map(h => {
    const fp = fpOf(h.content_id);
    return `<article class="hit" style="--fp: ${fp}">
      <span class="fp-badge">${esc(h.content_id.slice(0, 8))}</span>
      <div class="body">
        <div class="name">${h.filename ? esc(h.filename) : esc(h.content_id.slice(0, 8)) + "…"}</div>
        <div class="snippet">${h.highlight ? esc(h.highlight) : "—"}</div>
      </div>
      <div class="score"><div class="bar" style="width: ${Math.min(100, Math.round(h.score * 100))}%"></div>
      <div class="num">${h.score.toFixed(2)}</div></div>
    </article>`;
  }).join("")
    : total
      ? `<div class="empty">扩展名过滤后无显示命中——点掉上方 chips 恢复全部 ${total} 条。</div>`
      // T05 空态分支：索引空（docs==0）→ reindex 引导（CLI 命令直出，
      // §4 非目标：不做 GUI 内执行按钮）；有索引无命中 → 既有建议文案。
      : (indexDocs === 0
        ? `<div class="empty">没有找到「${esc(lastQ.slice(0, 24))}」——全文索引还没有建立——运行 <b>partisync reindex</b> 建立内容索引。</div>`
        : `<div class="empty">没有找到「${esc(lastQ.slice(0, 24))}」——换个更短的关键词${
            searchMode === "hybrid" ? "" : "，或切到「语义」模式放宽匹配"
          }。</div>`);
  // 记忆分区（§2.3：分区标题标明通道名；行 = content 截断 + tags + score，
  // 全部动态插值经 esc；空态/错误态沿既有 .empty 样式）。
  let memSection = "";
  if (lastMem) {
    memSection = `<div class="section-tag">记忆通道 · memory_search</div>` + (lastMem.error
      ? `<div class="empty">记忆通道不可用——见顶部错误提示。</div>`
      : lastMem.rows.length
        ? `<table class="panel-table" aria-label="记忆命中"><thead><tr><th>内容</th><th>tags</th><th>score</th></tr></thead><tbody>${
            lastMem.rows.map((m) => `
              <tr class="mem-row"><td>${esc(trunc(m.content, 90))}</td>
              <td class="fp">${esc(memTags(m.tags)) || "—"}</td>
              <td class="size">${(m.score ?? 0).toFixed(2)}</td></tr>`).join("")}</tbody></table>`
        : `<div class="empty">记忆通道无命中。</div>`);
  }
  $("srows").innerHTML = (lastMem ? `<div class="section-tag">资产通道 · ${lastModeLabel}检索</div>` : "")
    + assetRows + memSection;
}

async function doSearch() {
  const q = $("q").value.trim();
  const meta = $("ssearch-meta");
  await loadIndexStats(); // T05：0/>0 空态分支与徽标依赖（一次拉取缓存，不轮询）
  if (!q) {
    lastQ = ""; lastHits = []; lastMem = null; selectedExts.clear();
    renderChips();
    renderSearchIdle();
    meta.innerHTML = "";
    return;
  }
  $("srows").innerHTML = searchSkeleton(4);
  meta.innerHTML = searchMode === "hybrid"
    ? "语义检索中（首次需加载嵌入模型）…"
    : "检索中…";
  const cmd = searchCommand();
  // M9-WP03-T03（SPEC §2.3）：「含记忆」勾选 → 与资产检索**并行**调
  // memory_search（payload 恰为 {query}，sidecar 服务端默认 limit/offset）；
  // 资产通道 IPC 参数不变（资产区现状不变）。双通道各自内部排序不变、
  // 不合并数组（score 不可比，§6-R2 分区展示）；记忆通道失败不拖垮资产区
  // （catch → 记忆分区 empty 错误行，error-region 走 call()/mcPayload
  // 既有链路透传）。T04 起：命中集落 lastHits 快照，chips 过滤仅重渲
  // 已渲染命中（renderSearchResults），不再触碰 IPC。
  const withMem = $("mode-memory").checked;
  const memPromise = withMem
    ? memCall("memory_search", { query: q })
        .then((r) => ({ rows: r.results ?? [], error: false }))
        .catch(() => ({ rows: [], error: true }))
    : Promise.resolve(null);
  try {
    const [rows, mem] = await Promise.all([call(cmd, searchArgs(q)), memPromise]);
    lastQ = q;
    lastHits = rows;
    lastMem = mem;
    lastModeLabel = searchMode === "hybrid" ? "语义" : searchMode === "transcript" ? "含转写" : "关键词";
    selectedExts.clear();
    renderChips();
    renderSearchResults();
  } catch (e) {
    lastQ = ""; lastHits = []; lastMem = null; selectedExts.clear();
    renderChips();
    const kind = e?.kind ?? "Internal";
    meta.innerHTML = "";
    $("srows").innerHTML = `<div class="empty">
      ${kind === "Index" && searchMode === "hybrid"
        ? "语义检索不可用（嵌入模型加载失败）——切回「关键词」模式仍可检索。"
        : `检索失败（${esc(kind)}）——检查索引目录后重试。`}
      </div>`;
  }
}

// ── D. 重复内容 ──
async function loadDups() {
  const groups = await call("duplicates", { top: 50 });
  $("view-dups").innerHTML = groups.length ? groups.map(g => `
    <div class="dup">
      <div class="head">
        <span class="hash">content:${esc(g.content_id.slice(0, 16))}…</span>
        <span class="badge">${g.copies.length} 份副本 · 每份 ${sizeFmt(g.size)}</span>
      </div>
      <ul>${g.copies.map(c => `<li>${esc(c.path)}</li>`).join("")}</ul>
    </div>`).join("") : `<div class="empty">没有发现重复内容——索引更多文件后这里会自动按内容身份聚合相同文件</div>`;
}

// ── E. 作业 ──
async function loadJobs() {
  const rows = await call("jobs");
  $("view-jobs").innerHTML = rows.length ? `
    <table class="panel-table"><thead><tr><th>ID</th><th>类型</th><th>状态</th><th>已处理</th><th>checkpoint</th></tr></thead>
    <tbody>${rows.map(r => {
      const cls = r.status === 3 ? "job-status-ok" : r.status === 2 ? "job-status-warn" : r.status === 4 ? "job-status-err" : "";
      return `<tr><td class="fp">${esc(r.id.slice(0,10))}…</td><td>${esc(r.kind)}</td>
        <td class="${cls}">${esc(r.status_name || r.status)}</td>
        <td class="size">${r.done_files}</td><td class="fp">${esc(r.checkpoint || "—")}</td></tr>`;
    }).join("")}</tbody></table>` : `<div class="empty">暂无作业——运行 <b>partisync index / watch</b> 后这里会显示作业进度</div>`;
}

// ── G. 扩展 ──
async function loadExtTools() {
  renderExtHistory();
  const dbg = (m) => { const d = $("ext-debug"); if (d) d.textContent = `[${new Date().toLocaleTimeString()}] ${m}\n` + (d.textContent || ""); };
  try {
    dbg("loadExtTools: start");
    const r = await call("mcp_call", { tool: "ext_list", args: {} });
    const tools = r?.structuredContent?.tools ?? r?.tools ?? [];
    dbg(`loadExtTools: got ${tools.length} tool(s)`);
    $("ext-list").innerHTML = tools.length ? `
    <table class="panel-table"><thead><tr><th>工具</th><th style="width:220px">capabilities</th><th style="width:90px"></th></tr></thead>
    <tbody>${tools.map(t => `
      <tr data-tool="${esc(t.name)}" style="cursor:pointer"><td class="fp">${esc(t.name)}</td>
      <td>${esc((t.capabilities || []).join(", ")) || "（无宿主能力，纯计算）"}</td>
      <td><button class="btn ghost ext-call-btn" data-tool="${esc(t.name)}">调用</button></td></tr>`).join("")}</tbody></table>`
    : `<div class="empty">暂无扩展。放置 &lt;name&gt;.wasm + &lt;name&gt;.json 到 ~/.partisync/extensions 后重启。</div>`;
    const trs = $("ext-list").querySelectorAll("tr[data-tool]");
    trs.forEach(tr => {
      tr.onclick = () => openExtCall(tr.dataset.tool);
    });
    $("ext-list").querySelectorAll("button.ext-call-btn").forEach(b => {
      b.onclick = (ev) => { ev.stopPropagation(); openExtCall(b.dataset.tool); };
    });
    dbg(`loadExtTools: bound ${tools.length} tool(s)`);
  } catch (e) {
    dbg(`loadExtTools ERROR: ${e?.msg ?? e}`);
  }
}

function openExtCall(name) {
  $("ext-call").style.display = "";
  $("ext-call-name").textContent = name;
  $("ext-input").value = "";
  $("ext-output").textContent = "";
}

async function doExtCall() {
  const tool = $("ext-call-name").textContent;
  let args = {};
  const raw = $("ext-input").value.trim();
  if (raw) {
    try { args = { input: JSON.stringify(JSON.parse(raw)) }; }
    catch { $("ext-output").textContent = "入参不是合法 JSON"; return; }
  }
  $("ext-output").textContent = "…调用中（沙箱执行，10s 超时兜底）";
  try {
    const r = await call("mcp_call", { tool, args });
    const payload = r?.structuredContent ?? r;
    const out = JSON.stringify(payload, null, 2);
    $("ext-output").textContent = out;
    extHistory.unshift({ at: Date.now(), tool, input: raw || "（空）", output: out, ok: true });
  } catch (e) {
    const msg = e?.msg ?? String(e);
    $("ext-output").textContent = "调用失败：" + msg;
    extHistory.unshift({ at: Date.now(), tool, input: raw || "（空）", output: msg, ok: false });
  }
  if (extHistory.length > 20) extHistory.length = 20;
  renderExtHistory();
}

// ── G2. 扩展调用历史（M8-WP05-T04：会话内最近 20 次入参/出参，纯前端态） ──
let extHistory = [];

function trunc(s, n) {
  s = String(s ?? "");
  return s.length > n ? s.slice(0, n) + "…" : s;
}

function renderExtHistory() {
  const box = $("ext-history");
  if (!extHistory.length) {
    box.innerHTML = `<div class="empty">本次会话还没有扩展调用——在上方选工具后点「调用」</div>`;
    return;
  }
  box.innerHTML = `<table class="panel-table"><thead><tr><th style="width:90px">时间</th><th style="width:130px">工具</th><th style="width:56px">状态</th><th>入参 / 出参</th></tr></thead><tbody>${
    extHistory.map((h, i) => `
      <tr data-hist="${i}" title="点击回看本次入参/出参"><td class="size">${new Date(h.at).toLocaleTimeString("zh-CN", { hour12: false })}</td>
      <td class="fp">${esc(h.tool)}</td>
      <td class="${h.ok ? "hist-ok" : "hist-err"}">${h.ok ? "OK" : "ERR"}</td>
      <td class="hist-io">in ${esc(trunc(h.input, 70))} · out ${esc(trunc(h.output, 90))}</td></tr>`).join("")}</tbody></table>`;
  box.querySelectorAll("tr[data-hist]").forEach(tr => {
    tr.onclick = () => {
      const h = extHistory[+tr.dataset.hist];
      openExtCall(h.tool);
      $("ext-input").value = h.input === "（空）" ? "" : h.input;
      $("ext-output").textContent = h.output;
    };
  });
}

// ── F. 同步（M8-WP05-T03：sync_stats/sync_recent 只读呈现；H3 5s 轮询） ──
// 大数字 data-count 滚动只在首次入场播一次（design v4.3 tokens），轮询静默刷新。
let syncAnimated = false;

function relTime(ns) {
  if (!ns) return "";
  const s = Math.max(0, (Date.now() - ns / 1e6) / 1000);
  if (s < 60) return "刚刚";
  if (s < 3600) return `${Math.floor(s / 60)} 分钟前`;
  if (s < 86400) return `${Math.floor(s / 3600)} 小时前`;
  return `${Math.floor(s / 86400)} 天前`;
}

function dayLabel(ns) {
  const d = new Date(ns / 1e6), now = new Date();
  const day = (x) => Math.floor((x.getTime() - x.getTimezoneOffset() * 6e4) / 864e5);
  const diff = day(now) - day(d);
  if (diff <= 0) return "今天";
  if (diff === 1) return "昨天";
  return d.toLocaleDateString("zh-CN", { month: "numeric", day: "numeric" });
}

function countUp(el, target, animate) {
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (reduce || !animate) { el.textContent = target.toLocaleString("zh-CN"); return; }
  const t0 = performance.now(), dur = 900;
  const tick = (t) => {
    const p = Math.min(1, (t - t0) / dur), e = 1 - Math.pow(1 - p, 3);
    el.textContent = Math.round(target * e).toLocaleString("zh-CN");
    if (p < 1) requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

async function loadSync(animate = false) {
  const timeline = $("sync-timeline");
  try {
    const [stats, recent] = await Promise.all([
      call("sync_stats"),
      call("sync_recent", { limit: 30 })
    ]);
    countUp($("sync-applied"), stats.applied, animate);
    countUp($("sync-skipped"), stats.skipped_self, animate);
    countUp($("sync-lww"), stats.skipped_lww, animate);
    countUp($("sync-conflicts"), stats.conflicts, animate);
    // 状态横幅：对账语义（devices = oplog 远端 origin 去重）
    const banner = $("sync-banner");
    banner.classList.toggle("alert", stats.conflicts > 0);
    const total = stats.applied + stats.skipped_self;
    if (stats.devices > 0) {
      $("sync-banner-text").innerHTML =
        `已与 <b>${esc(stats.devices)} 台设备</b>保持一致${
          stats.conflicts ? ` · <span class="conflict-note">${esc(stats.conflicts)} 项冲突待处理</span>` : ""}`;
      $("sync-banner-time").textContent =
        stats.last_sync_ns ? `最近对账 ${relTime(stats.last_sync_ns)}` : "";
    } else if (total > 0) {
      $("sync-banner-text").innerHTML =
        `本机已记录 <b>${esc(total.toLocaleString("zh-CN"))}</b> 项变更 · 尚未与其他设备同步`;
      $("sync-banner-time").textContent = "";
    } else {
      $("sync-banner-text").textContent = "本机还没有同步记录";
      $("sync-banner-time").textContent = "";
    }
    // 按日分组时间线（指纹色延续：upsert 按 content_id 派生，冲突琥珀）
    if (!recent.length) {
      timeline.innerHTML =
        `<div class="empty">还没有文件变更——运行 <b>partisync index &lt;路径&gt;</b> 开始建立本机索引</div>`;
      return;
    }
    let lastDay = null, html = "";
    for (const it of recent) {
      const day = dayLabel(it.at_ns);
      if (day !== lastDay) { html += `<div class="sync-day">${day}</div>`; lastDay = day; }
      const fp = it.conflict ? "var(--amber)"
        : it.content_id ? fpOf(it.content_id) : "var(--green-dim)";
      const sub = it.conflict
        ? `<span class="conflict">冲突 · 两台设备都改了此文件</span>`
        : `<span>来自 ${esc(it.origin_device)}${it.op === "remove" ? " · 删除" : ""}</span>`;
      html += `<div class="sync-item" style="--fp: ${fp}">
        <span class="fp-dot"></span>
        <div class="what"><b>${esc(it.name)}</b>${sub}${it.dir ? `<div class="dir">${esc(it.dir)}</div>` : ""}</div>
        <time>${new Date(it.at_ns / 1e6).toLocaleTimeString("zh-CN", { hour12: false, hour: "2-digit", minute: "2-digit" })}</time>
      </div>`;
    }
    timeline.innerHTML = html;
  } catch (e) {
    const kind = e?.kind ?? "Internal";
    timeline.innerHTML = `<div class="empty">同步状态加载失败（${esc(kind)}）——确认数据库可读后重试。</div>`;
    $("sync-banner").classList.remove("alert");
    $("sync-banner-text").textContent = "同步状态不可用";
    $("sync-banner-time").textContent = "";
  }
}

// ── H. 记忆浏览面板（M9-WP03-T02；SPEC §2.2）——mcp_call 透传
// memory_search / memory_write / memory_verify 三工具，桌面 Rust 侧零新增
// command。真实 sidecar 返回 rmcp CallToolResult（camelCase：content /
// structuredContent / isError）——解包沿 ext 面（structuredContent ?? r）
// 判例；工具级错误（isError=true，文本在 content[]）解出后走 error-region
// 既有链路（SPEC §2.2：工具级错误文本透传显示）。
function mcPayload(r) {
  if (r?.isError) {
    const text = (r.content ?? []).map((c) => c?.text ?? "").filter(Boolean).join(" ")
      || "memory 工具返回错误（无错误文本）";
    showError(`[Memory] ${text}`);
    throw new Error(text);
  }
  return r?.structuredContent ?? r;
}

async function memCall(tool, args) {
  // IPC 层失败由 call() 直接走 error-region；工具级错误由 mcPayload 走。
  return mcPayload(await call("mcp_call", { tool, args }));
}

// tags canonical JSON 串（memory.rs canonical_tags）→ 逗号列表；解析失败原样透出。
function memTags(s) {
  try {
    const arr = JSON.parse(s);
    return Array.isArray(arr) ? arr.join(", ") : String(s ?? "");
  } catch { return String(s ?? ""); }
}

// 最近一次 memory_search 命中快照（memory_id → 原始行）：详情展开与复制
// 的数据源（M10-WP02-T02）——不抄 DOM 展示文本（列表 content 是截断态）。
let memIndex = new Map();

// 验证状态区：memory_verify（无 id）→ 根 hex 截断 + memory_count + ok 徽章。
async function loadMemVerify() {
  try {
    const v = await memCall("memory_verify", {});
    $("mem-root").textContent = v.root ? `${v.root.slice(0, 16)}…` : "—";
    $("mem-banner-text").innerHTML = `<b>${esc(v.memory_count)}</b> 条记忆 · 承诺验证`;
    const badge = $("mem-ok");
    badge.style.display = "";
    badge.textContent = v.ok ? "OK" : "FAIL";
    badge.classList.toggle("ok", !!v.ok);
    badge.classList.toggle("bad", !v.ok);
    $("mem-banner").classList.toggle("alert", !v.ok);
  } catch (e) {
    $("mem-banner-text").textContent = "记忆承诺不可用（sidecar 未启动或库不可读）";
    $("mem-ok").style.display = "none";
    $("mem-root").textContent = "";
  }
}

// 列表/检索：query/tag 空则不带键（全量列表），limit/offset 恒传（§2.2 契约）。
async function loadMemories() {
  const q = $("mem-q").value.trim();
  const tag = $("mem-tag").value.trim();
  const args = { limit: 50, offset: 0 };
  if (q) args.query = q;
  if (tag) args.tag = tag;
  $("mem-meta").innerHTML = "检索中…";
  try {
    const r = await memCall("memory_search", args);
    const rows = r.results ?? [];
    memIndex = new Map(rows.map((x) => [x.memory_id, x]));
    $("mem-meta").innerHTML = rows.length ? `<b>${rows.length}</b> / ${r.total ?? rows.length} 条记忆` : "";
    $("mem-rows").innerHTML = rows.length ? rows.map((m) => `
      <tr class="mem-row" data-mid="${esc(m.memory_id)}">
        <td>${esc(trunc(m.content, 90))}</td>
        <td class="fp">${esc(memTags(m.tags)) || "—"}</td>
        <td class="mtime">${timeFmt(m.created_ns)}</td>
        <td class="size">${esc(m.origin_device)}</td>
        <td class="size">${(m.score ?? 0).toFixed(2)}</td>
        <td><button class="btn ghost mem-verify-btn" data-mid="${esc(m.memory_id)}">验证</button></td>
      </tr>`).join("")
      : `<tr><td colspan="6" class="empty">${q || tag ? "没有命中的记忆——换个更短的词或清空 tag 过滤" : "还没有记忆——在下方写入第一条"}</td></tr>`;
  } catch (e) {
    $("mem-meta").innerHTML = "";
    $("mem-rows").innerHTML = `<tr><td colspan="6" class="empty">记忆列表加载失败——见顶部错误提示。</td></tr>`;
  }
  $("mem-rows").querySelectorAll("button.mem-verify-btn").forEach((b) => {
    b.onclick = (ev) => { ev.stopPropagation(); memVerifyRow(b.dataset.mid, b); };
  });
  // 行点击展开/收起详情（M10-WP02-T02；验证按钮 stopPropagation 已隔离，
  // 点验证不触发行展开）。
  $("mem-rows").querySelectorAll("tr.mem-row").forEach((tr) => {
    tr.onclick = () => memToggleDetail(tr.dataset.mid);
  });
}

// 行级「验证」：memory_verify(memory_id) → 包含证明展开行（单开语义，
// 再点收起）；proof = {memory_id, leaf_hash, audit_path[], root, ok}。
async function memVerifyRow(memoryId, btn) {
  const open = $("mem-rows").querySelector(`tr.mem-proof[data-proof="${CSS.escape(memoryId)}"]`);
  if (open) { open.remove(); return; }
  $("mem-rows").querySelectorAll("tr.mem-proof").forEach((tr) => tr.remove());
  const row = $("mem-rows").querySelector(`tr.mem-row[data-mid="${CSS.escape(memoryId)}"]`);
  btn.disabled = true;
  const label = btn.textContent;
  btn.textContent = "验证中…";
  try {
    const p = await memCall("memory_verify", { memory_id: memoryId });
    const tr = document.createElement("tr");
    tr.className = "mem-proof";
    tr.dataset.proof = p.memory_id;
    tr.innerHTML = `<td colspan="6">
      <span class="mem-badge ${p.ok ? "ok" : "bad"}">${p.ok ? "OK" : "FAIL"}</span>
      包含证明 · leaf <span class="hash">${esc(p.leaf_hash.slice(0, 16))}…</span>
      · audit_path <b>${p.audit_path.length}</b> 节点
      · root <span class="hash">${esc(p.root.slice(0, 16))}…</span></td>`;
    if (row) row.after(tr); else $("mem-rows").append(tr);
  } catch {
    // 工具级错误（如 memory 不存在）已由 mcPayload → error-region 透传。
  } finally {
    btn.disabled = false;
    btn.textContent = label;
  }
}

// ── 记忆详情联动展开 + 复制（M10-WP02-T02；SPEC §2.2）——行点击展开
// tr.mem-detail（沿证明行展开判例）：完整 content / tags 全列 / metadata
// pretty JSON / 完整 memory_id / created_ns 完整本地时间 / score 口径
// 注记（FTS 全文路径 = 匹配秩（-bm25）；LIKE 兜底 / tag·id 精确路径恒
// 1.0——store.rs memory_search docstring 语义诚实透出）。展开/收起只动
// 本行详情：不重写列表、不重发检索；多行详情并存，证明行语义不受影响。

// 详情行模板（动态段全部 esc / 数字格式化——无未转义插值；content 不截断）。
function memDetailHtml(m) {
  let metaPretty;
  try { metaPretty = JSON.stringify(JSON.parse(m.metadata), null, 2); }
  catch { metaPretty = String(m.metadata ?? ""); }
  return `<td colspan="6">
    <div class="mem-detail-grid">
      <span class="lbl">content</span><div class="val">${esc(m.content)}</div>
      <span class="lbl">tags</span><div class="val">${esc(memTags(m.tags)) || "—"}</div>
      <span class="lbl">metadata</span><div class="val"><pre class="mem-detail-meta">${esc(metaPretty) || "—"}</pre></div>
      <span class="lbl">memory_id</span><div class="val hash">${esc(m.memory_id)}</div>
      <span class="lbl">created_ns</span><div class="val">${timeFmt(m.created_ns)}</div>
      <span class="lbl">score</span><div class="val">${(m.score ?? 0).toFixed(2)}<span class="score-note">口径：FTS 全文路径 = 匹配秩（-bm25）；LIKE 兜底 / tag·id 精确路径恒 1.0</span></div>
    </div>
    <div class="mem-detail-actions">
      <button type="button" class="btn ghost mem-copy-btn" data-copy="content" data-mid="${esc(m.memory_id)}">复制内容</button>
      <button type="button" class="btn ghost mem-copy-btn" data-copy="id" data-mid="${esc(m.memory_id)}">复制 ID</button>
    </div></td>`;
}

// 展开/收起切换：已开 → 只删该行详情行；未开 → 在本行后插入。既有证明
// 行与列表本体一概不触碰。
function memToggleDetail(memoryId) {
  const open = $("mem-rows").querySelector(`tr.mem-detail[data-detail="${CSS.escape(memoryId)}"]`);
  if (open) { open.remove(); return; }
  const row = $("mem-rows").querySelector(`tr.mem-row[data-mid="${CSS.escape(memoryId)}"]`);
  const m = memIndex.get(memoryId);
  if (!row || !m) return;
  const tr = document.createElement("tr");
  tr.className = "mem-detail";
  tr.dataset.detail = memoryId;
  tr.innerHTML = memDetailHtml(m);
  row.after(tr);
  tr.querySelectorAll("button.mem-copy-btn").forEach((b) => {
    b.onclick = (ev) => { ev.stopPropagation(); memCopyDetail(b, b.dataset.mid, b.dataset.copy); };
  });
}

// 复制：webview 内建 navigator.clipboard.writeText（零新增依赖）；API 不
// 可用/拒权 → 隐藏 textarea + execCommand 回落（SPEC §6-R1 实现期拍板）。
// 成功反馈 = 按钮文案瞬变「已复制」1.5s 回落。
function memCopyDetail(btn, memoryId, what) {
  const m = memIndex.get(memoryId);
  if (!m) return;
  const text = what === "content" ? String(m.content ?? "") : String(m.memory_id ?? "");
  const done = () => {
    // 防重入（PR #166 对抗评审 F3）：1.5s 窗口内二次点击——原始 label 只
    // 捕获一次（dataset 持久保存，避免把「已复制」存成回落文案），旧 timer
    // 先清再设，杜绝双 setTimeout 竞争致按钮永久停留「已复制」。
    if (!btn.dataset.copyLabel) btn.dataset.copyLabel = btn.textContent;
    clearTimeout(Number(btn.dataset.copyTimer));
    btn.textContent = "已复制";
    btn.classList.add("copied");
    btn.dataset.copyTimer = String(setTimeout(() => {
      btn.textContent = btn.dataset.copyLabel;
      btn.removeAttribute("data-copy-label");
      btn.removeAttribute("data-copy-timer");
      btn.classList.remove("copied");
    }, 1500));
  };
  if (navigator.clipboard?.writeText) {
    navigator.clipboard.writeText(text).then(done).catch(() => execCopyFallback(text, done));
  } else {
    execCopyFallback(text, done);
  }
}

function execCopyFallback(text, done) {
  const ta = document.createElement("textarea");
  ta.value = text;
  ta.setAttribute("readonly", "");
  ta.style.position = "fixed";
  ta.style.opacity = "0";
  document.body.append(ta);
  ta.select();
  let ok = false;
  try { ok = document.execCommand("copy"); } catch { ok = false; }
  ta.remove();
  if (ok) done();
}

// 写入入口：content + tags（逗号分隔）+ metadata JSON → memory_write；
// deduplicated=true → 幂等命中提示（琥珀）。metadata 预校验沿工具契约
// （非空 object），超限/非法形态由服务端工具级错误经 error-region 透传。
async function memWrite() {
  const note = $("mem-write-note");
  const fail = (msg) => { note.className = "mem-note dup"; note.textContent = msg; };
  const content = $("mem-content").value;
  if (!content.trim()) { fail("内容不能为空"); return; }
  const args = { content };
  const tags = $("mem-write-tags").value.split(",").map((t) => t.trim()).filter(Boolean);
  if (tags.length) args.tags = tags;
  const rawMeta = $("mem-metadata").value.trim();
  if (rawMeta) {
    let meta;
    try { meta = JSON.parse(rawMeta); }
    catch { fail("metadata 不是合法 JSON——留空或写成 {\"k\":\"v\"} 形态"); return; }
    if (!meta || typeof meta !== "object" || Array.isArray(meta)) {
      fail("metadata 必须为 JSON object"); return;
    }
    args.metadata = meta;
  }
  note.className = "mem-note";
  note.textContent = "写入中…";
  try {
    const r = await memCall("memory_write", args);
    if (r.deduplicated) {
      note.className = "mem-note dup";
      note.textContent = `幂等命中：同 (content, tags, metadata) 已存在，复用 ${(r.memory_id ?? "").slice(0, 12)}…`;
    } else {
      note.className = "mem-note ok";
      note.textContent = `已写入 ${(r.memory_id ?? "").slice(0, 12)}…（承诺根已刷新）`;
    }
    loadMemVerify();
    loadMemories();
  } catch (e) {
    fail("写入失败——见顶部错误提示。");
  }
}

// ── 绑定（CSP 禁 inline onclick） ──
document.querySelectorAll("nav button").forEach(b => b.onclick = () => {
  document.querySelectorAll("nav button").forEach(x => x.removeAttribute("aria-current"));
  b.setAttribute("aria-current", "page");
  const t = b.dataset.tab;
  ["browse", "search", "memory", "sync", "dups", "jobs", "ext"].forEach(v => {
    const el = $(`view-${v}`);
    if (el) el.style.display = v === t ? "" : "none";
  });
  if (t === "browse") browse(curPath);
  if (t === "search") renderSearchIdle(); // T05：初始态 = 输入引导 + 索引规模徽标
  if (t === "memory") { loadMemVerify(); loadMemories(); }
  if (t === "sync") { loadSync(!syncAnimated); syncAnimated = true; }
  if (t === "dups") loadDups();
  if (t === "jobs") loadJobs();
  if (t === "ext") loadExtTools();
});
// 检索 form 防整页重载（M10-WP01-T04 GUI 实操发现的既有 bug）：CSP
// script-src 'self' 必拦 inline onsubmit（index.html 原 onsubmit 形同
// 虚设），Enter 隐式提交 / submit 按钮都会把整页刷回 browse tab。JS 侧
// 双保险：Enter keydown preventDefault + form submit 兜底 preventDefault。
$("q").addEventListener("keydown", e => { if (e.key === "Enter") { e.preventDefault(); doSearch(); } });
document.querySelector("form.query-row").addEventListener("submit", (e) => e.preventDefault());
$("btn-search").onclick = doSearch;
$("mode-bm25").addEventListener("change", () => { searchMode = "bm25"; if ($("q").value.trim()) doSearch(); });
$("mode-hybrid").addEventListener("change", () => { searchMode = "hybrid"; if ($("q").value.trim()) doSearch(); });
$("mode-transcript").addEventListener("change", () => { searchMode = "transcript"; if ($("q").value.trim()) doSearch(); });
$("btn-ext-refresh").onclick = loadExtTools;
$("btn-ext-call").onclick = doExtCall;
// 记忆面板（M9-WP03-T02）：检索/写入/回车触发；不进 5s 轮询（防行级
// 证明展开态被打断），切 tab / 写入后刷新。
$("btn-mem-search").onclick = loadMemories;
$("mem-q").addEventListener("keydown", (e) => { if (e.key === "Enter") loadMemories(); });
$("btn-mem-write").onclick = memWrite;

setInterval(async () => {
  await loadStats();
  const cur = document.querySelector("nav button[aria-current]")?.dataset.tab;
  if (cur === "browse") {
    browse(curPath);
  }
  if (cur === "sync") {
    loadSync(false);
  }
}, 5000);

loadStats();
browse("/");
