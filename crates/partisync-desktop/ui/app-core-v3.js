// PartiSync Desktop 前端 v3（设计语言 v4.3 落地——旧薄面弃用重写）。
// M8-WP05-T01：语义检索旗舰（search_hybrid + 三态）。
// 绑定面：index.html（v4.3 结构：tally 仪表 / hit 卡 / section-tag）。
// 数据走 11 个 Tauri command（src/ipc.rs）：
//   get_stats / list / search / search_hybrid / asset_detail / cas_stats /
//   duplicates / jobs / sync_stats / sync_recent / mcp_call
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
let searchMode = "bm25"; // bm25 = 关键词；hybrid = 语义（T01 旗舰）
const $ = (id) => document.getElementById(id);

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
      ? `<b>${e.name}</b>`
      : `<a href="#" data-path="${e.path}">${e.name}</a>`
  ).join(`<span>▸</span>`);
  $("crumbs").querySelectorAll("a[data-path]").forEach(a => {
    a.onclick = () => browse(a.dataset.path);
  });
  $("rows").innerHTML = rows.length ? rows.map(e => {
    const dir = e.kind === 1;
    const fp = e.content_id ? ` style="--fp: ${fpOf(e.content_id)}"` : "";
    return `<tr class="${dir ? "row-dir" : "row-file"}"${dir ? ` data-path="${e.path}" style="cursor:pointer"` : ` data-cid="${e.content_id}" data-name="${e.name}" style="cursor:pointer"`}${e.content_id ? ` style="--fp: ${fpOf(e.content_id)}"` : ""}>
      <td class="icon" aria-hidden="true">${dir ? "▸" : "·"}</td>
      <td>${e.name}</td><td class="size">${dir ? "—" : sizeFmt(e.size)}</td>
      <td class="mtime">${timeFmt(e.mtime_ns)}</td>
      <td class="fp"${e.content_id ? ` style="--fp: ${fpOf(e.content_id)}"` : ""}>${e.content_id ? "<i></i>" + e.content_id.slice(0, 8) : "—"}</td></tr>`;
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
    panel.innerHTML = `<div class="empty">详情加载失败（${e?.kind ?? "?"}）</div>`;
    return;
  }
  const fp = fpOf(contentId);
  const copies = d.copies.map(c =>
    `<li>${c.path} <span class="dim">· ${sizeFmt(c.size)}</span></li>`).join("");
  panel.innerHTML = `
    <h2>${name}</h2>
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
  // 含转写文本 = BM25 通道透传（N4：include_transcript 已在后端常开，
  // 转写命中走同一 search IPC；开关仅为语义标注）。
  if (searchMode === "hybrid") return "search_hybrid";
  return "search";
}

function searchSkeleton(n) {
  return Array.from({ length: n }, (_, i) =>
    `<article class="hit skel-row"><div class="skel skel-fp"></div>
     <div class="col"><div class="skel" style="height:13px;width:${45 - i * 5}%"></div>
     <div class="skel" style="height:10px;width:${70 - i * 4}%;margin-top:7px"></div></div></article>`
  ).join("");
}

async function doSearch() {
  const q = $("q").value.trim();
  const meta = $("ssearch-meta");
  if (!q) {
    $("srows").innerHTML = `<div class="empty">输入关键词或自然语言问题开始检索</div>`;
    meta.innerHTML = "";
    return;
  }
  $("srows").innerHTML = searchSkeleton(4);
  meta.innerHTML = searchMode === "hybrid"
    ? "语义检索中（首次需加载嵌入模型）…"
    : "检索中…";
  const cmd = searchMode === "hybrid" ? "search_hybrid" : "search";
  try {
    const rows = await call(cmd, { q, limit: 50 });
    const modeLabel = searchMode === "hybrid" ? "语义" : "关键词";
    meta.innerHTML = rows.length
      ? `<b>${rows.length}</b> hits · ${modeLabel}`
      : "";
    $("srows").innerHTML = rows.length ? rows.map(h => {
      const fp = fpOf(h.content_id);
      return `<article class="hit" style="--fp: ${fp}">
        <span class="fp-badge">${h.content_id.slice(0, 8)}</span>
        <div class="body">
          <div class="name">${h.content_id.slice(0, 8)}…</div>
          <div class="snippet">${h.highlight || "—"}</div>
        </div>
        <div class="score"><div class="bar" style="width: ${Math.min(100, Math.round(h.score * 100))}%"></div>
        <div class="num">${h.score.toFixed(2)}</div></div>
      </article>`;
    }).join("") : `<div class="empty">没有找到「${q.slice(0, 24)}」——换个更短的关键词${
      searchMode === "hybrid" ? "" : "，或切到「语义」模式放宽匹配"
    }。</div>`;
  } catch (e) {
    const kind = e?.kind ?? "Internal";
    meta.innerHTML = "";
    $("srows").innerHTML = `<div class="empty">
      ${kind === "Index" && searchMode === "hybrid"
        ? "语义检索不可用（嵌入模型加载失败）——切回「关键词」模式仍可检索。"
        : `检索失败（${kind}）——检查索引目录后重试。`}
      </div>`;
  }
}

// ── D. 重复内容 ──
async function loadDups() {
  const groups = await call("duplicates", { top: 50 });
  $("view-dups").innerHTML = groups.length ? groups.map(g => `
    <div class="dup">
      <div class="head">
        <span class="hash">content:${g.content_id.slice(0, 16)}…</span>
        <span class="badge">${g.copies.length} 份副本 · 每份 ${sizeFmt(g.size)}</span>
      </div>
      <ul>${g.copies.map(c => `<li>${c.path}</li>`).join("")}</ul>
    </div>`).join("") : `<div class="empty">没有发现重复内容——索引更多文件后这里会自动按内容身份聚合相同文件</div>`;
}

// ── E. 作业 ──
async function loadJobs() {
  const rows = await call("jobs");
  $("view-jobs").innerHTML = rows.length ? `
    <table class="panel-table"><thead><tr><th>ID</th><th>类型</th><th>状态</th><th>已处理</th><th>checkpoint</th></tr></thead>
    <tbody>${rows.map(r => {
      const cls = r.status === 3 ? "job-status-ok" : r.status === 2 ? "job-status-warn" : r.status === 4 ? "job-status-err" : "";
      return `<tr><td class="fp">${r.id.slice(0,10)}…</td><td>${r.kind}</td>
        <td class="${cls}">${r.status_name || r.status}</td>
        <td class="size">${r.done_files}</td><td class="fp">${r.checkpoint || "—"}</td></tr>`;
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
      <tr data-tool="${t.name}" style="cursor:pointer"><td class="fp">${t.name}</td>
      <td>${(t.capabilities || []).join(", ") || "（无宿主能力，纯计算）"}</td>
      <td><button class="btn ghost ext-call-btn" data-tool="${t.name}">调用</button></td></tr>`).join("")}</tbody></table>`
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
      <td class="fp">${h.tool}</td>
      <td class="${h.ok ? "hist-ok" : "hist-err"}">${h.ok ? "OK" : "ERR"}</td>
      <td class="hist-io">in ${trunc(h.input, 70)} · out ${trunc(h.output, 90)}</td></tr>`).join("")}</tbody></table>`;
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
        `已与 <b>${stats.devices} 台设备</b>保持一致${
          stats.conflicts ? ` · <span class="conflict-note">${stats.conflicts} 项冲突待处理</span>` : ""}`;
      $("sync-banner-time").textContent =
        stats.last_sync_ns ? `最近对账 ${relTime(stats.last_sync_ns)}` : "";
    } else if (total > 0) {
      $("sync-banner-text").innerHTML =
        `本机已记录 <b>${total.toLocaleString("zh-CN")}</b> 项变更 · 尚未与其他设备同步`;
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
        : `<span>来自 ${it.origin_device}${it.op === "remove" ? " · 删除" : ""}</span>`;
      html += `<div class="sync-item" style="--fp: ${fp}">
        <span class="fp-dot"></span>
        <div class="what"><b>${it.name}</b>${sub}${it.dir ? `<div class="dir">${it.dir}</div>` : ""}</div>
        <time>${new Date(it.at_ns / 1e6).toLocaleTimeString("zh-CN", { hour12: false, hour: "2-digit", minute: "2-digit" })}</time>
      </div>`;
    }
    timeline.innerHTML = html;
  } catch (e) {
    const kind = e?.kind ?? "Internal";
    timeline.innerHTML = `<div class="empty">同步状态加载失败（${kind}）——确认数据库可读后重试。</div>`;
    $("sync-banner").classList.remove("alert");
    $("sync-banner-text").textContent = "同步状态不可用";
    $("sync-banner-time").textContent = "";
  }
}

// ── 绑定（CSP 禁 inline onclick） ──
document.querySelectorAll("nav button").forEach(b => b.onclick = () => {
  document.querySelectorAll("nav button").forEach(x => x.removeAttribute("aria-current"));
  b.setAttribute("aria-current", "page");
  const t = b.dataset.tab;
  ["browse", "search", "sync", "dups", "jobs", "ext"].forEach(v => {
    const el = $(`view-${v}`);
    if (el) el.style.display = v === t ? "" : "none";
  });
  if (t === "browse") browse(curPath);
  if (t === "sync") { loadSync(!syncAnimated); syncAnimated = true; }
  if (t === "dups") loadDups();
  if (t === "jobs") loadJobs();
  if (t === "ext") loadExtTools();
});
$("q").addEventListener("keydown", e => { if (e.key === "Enter") doSearch(); });
$("btn-search").onclick = doSearch;
$("mode-bm25").addEventListener("change", () => { searchMode = "bm25"; if ($("q").value.trim()) doSearch(); });
$("mode-hybrid").addEventListener("change", () => { searchMode = "hybrid"; if ($("q").value.trim()) doSearch(); });
$("mode-transcript").addEventListener("change", () => { searchMode = "bm25"; if ($("q").value.trim()) doSearch(); });
$("btn-ext-refresh").onclick = loadExtTools;
$("btn-ext-call").onclick = doExtCall;

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
