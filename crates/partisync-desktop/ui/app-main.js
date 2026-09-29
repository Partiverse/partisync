// PartiSync Desktop 前端（T03 IPC 调用层 + M7-WP01-T04 扩展面板）。
//
// 数据走 7 个 Tauri command（src/ipc.rs）：
//   get_stats / list / search / cas_stats / duplicates / jobs / mcp_call
//
// 错误形状： IPC 返回 `{kind: string, msg: string}`（见 error.rs）
// → 捕获 invoke 抛错， 按 kind 分支处理（toast / 降级 / 重试）。

// Tauri 全局注入（tauri.conf.json app.withGlobalTauri = true）。
// 显式守卫：未注入时给出可读提示而非整页 JS 静默失效（此前
// `window.__TAURI__.core` 解构在首行抛错，表现与静态 HTML 无异）。
const __tauriCore = window.__TAURI__?.core;
if (!__tauriCore) {
  document.body.innerHTML =
    '<div style="padding:40px;font-family:system-ui;color:#f85149">' +
    "Tauri API 未注入：桌面壳 IPC 不可用（请确认以桌面壳方式启动，" +
    "且 tauri.conf.json 的 app.withGlobalTauri 为 true）。</div>";
  throw new Error("Tauri IPC bridge unavailable");
}
const { invoke } = __tauriCore;

let curPath = "/";
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

/** 包装 invoke： 统一捕获 IPC 错误并显示在 #error-region。
 *
 * Tauri 2 参数绑定：command 的 Rust 参数名 = invoke 键。本项目所有
 * 带参 command 的参数名统一为 `args`（ListArgs/SearchArgs/DuplicatesArgs/
 * McpCallArgs），故此处统一包一层 `{ args }`——调用方保持平铺语义
 * （`call("list", { prefix })`）。此前平铺直传触发
 * 「missing required key args」（UI 修复前 IPC 从未真正执行过，
 * 该绑定错误潜伏至 withGlobalTauri 修复后才暴露）。
 */
async function call(cmd, args) {
  try {
    return await invoke(cmd, args === undefined ? {} : { args });
  } catch (e) {
    // IPC 错误形状： {kind, msg}（src/error.rs DesktopError Serialize）
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

async function loadStats() {
  const [s, cas] = await Promise.all([call("get_stats"), call("cas_stats")]);
  $("cards").innerHTML = `
    <div class="card"><div class="label">文件</div><div class="value">${s.files}</div></div>
    <div class="card"><div class="label">目录</div><div class="value">${s.dirs}</div></div>
    <div class="card"><div class="label">总容量</div><div class="value">${sizeFmt(s.total_bytes)}</div></div>
    <div class="card"><div class="label">唯一内容</div><div class="value">${s.unique_contents}</div>
      <div class="hint">${sizeFmt(s.unique_bytes)}</div></div>
    <div class="card"><div class="label">去重节省</div><div class="value green">${sizeFmt(s.saved_bytes)}</div>
      <div class="hint">${s.total_bytes ? (100*s.saved_bytes/s.total_bytes).toFixed(1) : 0}% 的字节是重复的</div></div>
    <div class="card"><div class="label">重复组</div><div class="value">${s.duplicate_groups}</div></div>
    <div class="card"><div class="label">块级节省</div><div class="value green">${sizeFmt(cas.saved_bytes)}</div>
      <div class="hint">${cas.refs} 个块引用 / ${cas.chunks} 个唯一块（CDC 1MiB）</div></div>`;
}

/** 由路径派生面包屑（替代 CLI 版 /api/breadcrumb 端点）。 */
function breadcrumbFor(path) {
  if (path === "/") return [{ name: "根", path: "/" }];
  const parts = path.split("/").filter(Boolean);
  const out = [{ name: "根", path: "/" }];
  let acc = "";
  for (const p of parts) {
    acc += "/" + p;
    out.push({ name: p, path: acc });
  }
  return out.slice(1); // 尾段由 browse() 加粗显示，避免「根 › 根」重复
}

async function browse(path) {
  curPath = path || "/";
  const rows = await call("list", { prefix: curPath });
  const crumbs = breadcrumbFor(curPath);
  $("crumbs").innerHTML = crumbs.map(e =>
    `<a href="#" data-path="${e.path}">${e.name}</a>`
  ).join(`<span class="sep">›</span>`) +
    `<span class="sep">›</span><b>${curPath === "/" ? "根" : curPath.split("/").pop()}</b>`;
  // CSP（script-src 'self'）禁 inline onclick 属性——交互一律 JS 绑定
  $("crumbs").querySelectorAll("a[data-path]").forEach(a => {
    a.onclick = () => browse(a.dataset.path);
  });
  $("rows").innerHTML = rows.length ? rows.map(e => {
    const dir = e.kind === 1;
    const icon = dir ? "▸" : "·";
    const cls = dir ? "row-dir" : "row-file";
    return `<tr class="${cls}"${dir ? ` data-path="${e.path}" style="cursor:pointer"` : ""}>
      <td class="${dir ? "icon-dir" : "icon-file"}">${icon}</td>
      <td>${e.name}</td><td>${dir ? "—" : sizeFmt(e.size)}</td>
      <td>${timeFmt(e.mtime_ns)}</td>
      <td class="hash">${e.content_id ? e.content_id.slice(0, 8) + "…" : ""}</td></tr>`;
  }).join("") : `<tr><td colspan="5" class="empty">空目录</td></tr>`;
  $("rows").querySelectorAll("tr[data-path]").forEach(tr => {
    tr.onclick = () => browse(tr.dataset.path);
  });
}

async function doSearch() {
  const q = $("q").value.trim();
  const rows = q ? await call("search", { q, limit: 50 }) : [];
  $("srows").innerHTML = rows.length ? rows.map(h =>
    `<tr class="row-file"><td class="hash">${h.content_id.slice(0, 8)}…</td>
     <td>${(h.highlight || "—").slice(0, 80)}</td>
     <td>${h.score.toFixed(3)}</td></tr>`
  ).join("") : `<tr><td colspan="3" class="empty">${q ? "无结果" : "输入关键词"}</td></tr>`;
}

async function loadJobs() {
  const rows = await call("jobs");
  $("view-jobs").innerHTML = rows.length ? `
    <table><thead><tr><th>ID</th><th>类型</th><th>状态</th><th style="width:90px">已处理</th><th>checkpoint</th></tr></thead>
    <tbody>${rows.map(r => {
      const color = r.status === 3 ? "var(--green)" : r.status === 2 ? "var(--amber)" : r.status === 4 ? "#f85149" : "var(--dim)";
      return `<tr><td class="hash">${r.id.slice(0,10)}…</td><td>${r.kind}</td>
        <td style="color:${color}">${r.status_name || r.status}</td>
        <td>${r.done_files}</td><td class="hash">${r.checkpoint || "—"}</td></tr>`;
    }).join("")}</tbody></table>` : `<div class="empty">暂无作业</div>`;
}

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

// ── 扩展面板（M7-WP01-T04）：经 mcp_call 消费 ext_list / ext_<name> ──

async function loadExtTools() {
  const dbg = (m) => { const d = $("ext-debug"); if (d) d.textContent = `[${new Date().toLocaleTimeString()}] ${m}\n` + (d.textContent || ""); };
  try {
    dbg("loadExtTools: start");
    const r = await call("mcp_call", { tool: "ext_list", args: {} });
    // mcp_call 返回 CallToolResult 全对象——工具载荷在 structuredContent
    const tools = r?.structuredContent?.tools ?? r?.tools ?? [];
    dbg(`loadExtTools: got ${tools.length} tool(s)`);
    $("ext-list").innerHTML = tools.length ? `
    <table><thead><tr><th>工具</th><th style="width:220px">capabilities</th><th style="width:90px"></th></tr></thead>
    <tbody>${tools.map(t => `
      <tr data-tool="${t.name}" style="cursor:pointer"><td class="hash">${t.name}</td>
      <td>${(t.capabilities || []).join(", ") || "（无宿主能力，纯计算）"}</td>
      <td><button class="ext-call-btn" data-tool="${t.name}">调用</button></td></tr>`).join("")}</tbody></table>`
    : `<div class="empty">暂无扩展。放置 &lt;name&gt;.wasm + &lt;name&gt;.json 到 ~/.partisync/extensions 后重启。</div>`;
    // CSP 禁 inline onclick——渲染后 JS 绑定
    const trs = $("ext-list").querySelectorAll("tr[data-tool]");
    trs.forEach(tr => {
      tr.onclick = (ev) => {
        dbg(`row clicked: ${tr.dataset.tool} @ client(${ev.clientX},${ev.clientY})`);
        openExtCall(tr.dataset.tool);
      };
    });
    let btns = 0;
    $("ext-list").querySelectorAll("button.ext-call-btn").forEach(b => {
      b.onclick = (ev) => { ev.stopPropagation(); openExtCall(b.dataset.tool); };
      btns++;
    });
    dbg(`loadExtTools: bound ${trs.length} row(s), ${btns} button(s)`);
  } catch (e) {
    dbg(`loadExtTools ERROR: ${e?.msg ?? e}`);
  }
}

function openExtCall(name) {
  $("ext-call").style.display = "";
  $("ext-call-name").textContent = name;
  $("ext-input").value = "";
  $("ext-output").textContent = "（结果）";
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
    // 工具载荷优先取 structuredContent（CallToolResult 全对象时）
    const payload = r?.structuredContent ?? r;
    $("ext-output").textContent = JSON.stringify(payload, null, 2);
  } catch (e) {
    $("ext-output").textContent = "调用失败：" + (e?.msg ?? String(e));
  }
}

document.querySelectorAll("nav.tabs button").forEach(b => b.onclick = () => {
  document.querySelectorAll("nav.tabs button").forEach(x => x.classList.remove("active"));
  b.classList.add("active");
  const t = b.dataset.tab;
  ["browse", "search", "dups", "jobs", "ext"].forEach(v => $(`view-${v}`).style.display = v === t ? "" : "none");
  if (t === "browse") browse(curPath);
  if (t === "dups") loadDups();
  if (t === "jobs") loadJobs();
  if (t === "ext") loadExtTools();
});
$("q").addEventListener("keydown", e => { if (e.key === "Enter") doSearch(); });
// CSP（script-src 'self'）禁 inline onclick——静态按钮一律 JS 绑定
$("btn-search").onclick = doSearch;
$("btn-ext-refresh").onclick = loadExtTools;
$("btn-ext-call").onclick = doExtCall;

// 自动刷新： 统计与浏览视图每 5s 轮询
setInterval(async () => {
  await loadStats();
  if (document.querySelector("nav.tabs button.active").dataset.tab === "browse") {
    browse(curPath);
  }
}, 5000);

loadStats();
browse("/");
