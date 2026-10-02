// PartiSync Desktop 前端 v3（设计语言 v4.3 落地——旧薄面弃用重写）。
// M8-WP05-T01：语义检索旗舰（search_hybrid + 三态）。
// 绑定面：index.html（v4.3 结构：tally 仪表 / hit 卡 / section-tag）。
// 数据走 8 个 Tauri command（src/ipc.rs）：
//   get_stats / list / search / search_hybrid / cas_stats / duplicates /
//   jobs / mcp_call
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
    return `<tr class="${dir ? "row-dir" : "row-file"}"${dir ? ` data-path="${e.path}" style="cursor:pointer"` : ""}${fp ? ` ${fp.replace('style="', 'style="')}` : ""}>
      <td class="icon" aria-hidden="true">${dir ? "▸" : "·"}</td>
      <td>${e.name}</td><td class="size">${dir ? "—" : sizeFmt(e.size)}</td>
      <td class="mtime">${timeFmt(e.mtime_ns)}</td>
      <td class="fp"${e.content_id ? ` style="--fp: ${fpOf(e.content_id)}"` : ""}>${e.content_id ? "<i></i>" + e.content_id.slice(0, 8) : "—"}</td></tr>`;
  }).join("") : `<tr><td colspan="5" class="empty">空目录</td></tr>`;
  $("rows").querySelectorAll("tr[data-path]").forEach(tr => {
    tr.onclick = () => browse(tr.dataset.path);
  });
}

// ── C. 检索（旗舰；三态全覆盖——设计审计硬约束） ──
function searchCommand() {
  // 含转写文本 = BM25 通道透传（N4 零后端增量）；语义 = search_hybrid
  if (searchMode === "hybrid") return "search_hybrid";
  if ($("mode-transcript").checked) return "search_transcript_placeholder";
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
    </div>`).join("") : `<div class="empty">没有发现重复内容</div>`;
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
    }).join("")}</tbody></table>` : `<div class="empty">暂无作业</div>`;
}

// ── G. 扩展 ──
async function loadExtTools() {
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
    $("ext-output").textContent = JSON.stringify(payload, null, 2);
  } catch (e) {
    $("ext-output").textContent = "调用失败：" + (e?.msg ?? String(e));
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
  if (document.querySelector("nav button[aria-current]")?.dataset.tab === "browse") {
    browse(curPath);
  }
}, 5000);

loadStats();
browse("/");
