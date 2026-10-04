# Task: M9-WP03-T02 记忆浏览面板——桌面 UI 经 mcp_call 透传 memory_* 三工具

> **范围外**：旗舰检索「含记忆」分区随 T03；GUI 三态截图（全空/有数据/
> 篡改红徽章）随 T04 人工；语义向量检索进 memory 面 = 非目标（SPEC §4）。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP03-T02 |
| **类型** | 实装（桌面 UI 纯前端 + 测试；R0/R1——Rust 侧零改动，风险在 JS 渲染面） |
| **优先级** | P0（SPEC M9-WP03 三主线之一；α 主线核心资产「用户不可见」清偿） |
| **范围** | SPEC M9-WP03 §2.2 + §3「记忆 tab e2e」验收 + §5 清单（index.html / app-core-v3.js / styles-v3.css / commands.rs / ui_hardening.rs）+ 本卡 |
| **创建日期** | 2026-10-04 |
| **来源** | SPEC M9-WP03 批准（PR #134）+ M9-WP02-T04 MCP 三工具（memory_write/search/verify 已入 tools/list） |

## 契约落点（SPEC §2.2 逐条）

1. **列表/检索**：`mcp_call("memory_search", {query?, tag?, limit, offset})`；
   结果行 = content + tags + created_ns + origin_device + score。query/tag
   空则不带键（全量列表），limit=50 / offset=0 恒传。
2. **验证状态区**：`memory_verify`（无 id）→ 根 hex 截断展示（16 hex + …）
   + memory_count + ok 徽章（绿 OK / 红 FAIL，`.mem-badge`）。
3. **行级「验证」**：`memory_verify({memory_id})` → 展开包含证明行
   （leaf_hash 截断 / audit_path 长度 / root 截断 + ok 徽章）；再点收起。
4. **写入入口**：content 文本域 + tags（逗号分隔）+ metadata JSON 输入 →
   `memory_write`；`deduplicated=true` → 幂等命中提示（琥珀 `.mem-note`）。
5. **遥测口径**：IPC 失败走 `call()` 既有 error-region；工具级错误
   （CallToolResult `isError:true` + content text）经 `mcPayload()` 解出
   文本后同样透传 error-region（SPEC §2.2「工具级错误文本透传显示」）。
6. **Rust 侧零新增 IPC command**：全部走既有 `mcp_call`（ipc.rs:409）。

## 数据面（实读源码核实，非凭记忆）

- 真实 sidecar 返回 rmcp 3.4.0 `CallToolResult` 序列化（camelCase）：
  `{content, structuredContent?, isError?}`（model.rs:3873）——面板统一
  `r?.structuredContent ?? r` 解包（ext 面 app-core-v3.js:298 判例）。
- `memory_search` → `{results:[{memory_id, content, tags(canonical JSON
  串), metadata, created_ns, origin_device, score}], total}`
  （partisync-graph/src/memory.rs:240-266）。
- `memory_verify` 无 id → `{root, memory_count, recomputed_root, ok}`
  （gateway/src/mcp.rs:731-736）；有 id → `MemoryInclusionProof
  {memory_id, leaf_hash, audit_path[], root, ok}`（memory.rs:270-280）。
- `memory_write` → `{memory_id, deduplicated, root}`（mcp.rs:687-691）。

## 测试（SPEC §3「记忆 tab e2e」）

1. **commands.rs**：`t02_memory_panel_dataflow_via_stub_sidecar`——bash
   stub 沿 `mcp_call_on_stub_sidecar_returns_result` 模板升级：按
   `params.name` 回结构化 CallToolResult 形状（structuredContent +
   isError 分支）、回显请求 id（多调用串行不串线）、请求行旁路落盘。
   断言面板三动作（search/write/verify×2）的 JSON-RPC `params.arguments`
   与 §2.2 契约逐键一致 + 响应形状可驱动渲染。
2. **ui_hardening.rs**：静态契约探针补记忆 tab——JS 断言 mcp_call 工具名
   三具全接线 + 记忆动态字段裸插值零命中（禁列追加）+ mem* DOM id 在
   index.html 中存在（include_str 双文件对账，防 id 漂移）。

## 边界与既有决定

- GUI 实操三态截图 = T04 人工（AGENTS.md 桌面端验收规则的「待 GUI 验证」
  例外面：本任务不操控 GUI，PR 标注截图挂 T04）。
- 面板只读展示 origin_device，不做编辑/删除（SPEC §4 非目标）。
- 不自动轮询记忆 tab（防行级证明展开态被打断）；切 tab / 检索 / 写入后
  手动刷新，5s 轮询面维持 browse/sync 现状。
- tags 显示 = canonical JSON 串解析为逗号列表，解析失败原样透出（esc）。
- 零新增依赖（纯前端 + 既有测试基建）。

## 验收

- [ ] index.html：nav「记忆」tab + `#view-memory`（验证横幅/检索行/列表
      表/写入入口），无 inline 事件 handler；
- [ ] app-core-v3.js：§2.2 六条契约全接线，动态插值全部 esc()；
- [ ] commands.rs stub 数据流测试绿（payload 逐键断言）；
- [ ] ui_hardening.rs 新探针绿（工具名接线 + 裸插值禁列 + DOM id 对账）；
- [ ] fmt/clippy/test 三件套全绿；既有测试零回归（commands 19→20
     随数据流测试、探针 4→6 随本卡）；
- [ ] 提交挂 Task-ID `M9-WP03-T02`；GUI 截图挂 T04。

## 拆分登记（铁律 3：单 PR ≤400 行）

总量 ≈572 行超限，沿堆叠判例拆两 PR（同一 Task-ID）：

| PR | 内容 | 规模 |
|---|---|---|
| 1 | 本卡 + UI 三件（index.html/app-core-v3.js/styles-v3.css）+ ui_hardening.rs 探针 | ≈395 行 |
| 2 | commands.rs stub 数据流 e2e（rebase origin/main 后开） | ≈180 行 |
