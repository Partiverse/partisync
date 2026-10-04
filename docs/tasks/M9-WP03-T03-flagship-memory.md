# Task: M9-WP03-T03 旗舰检索记忆通道——检索 tab「含记忆」勾选 + 双通道分区展示

> **范围外**：GUI 实操截图随 T04 人工（本任务不操控 GUI）；跨通道混合
> 排序 = 非目标（SPEC §4 / §6-R2：score 不可比，分区展示不混排）；
> 语义向量检索进 memory 面 = 非目标（SPEC §4）。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP03-T03 |
| **类型** | 实装（桌面 UI 纯前端 + 测试；R0——Rust IPC 零改动，风险在 JS 渲染面） |
| **优先级** | P0（SPEC M9-WP03 三主线之三；旗舰检索「只覆盖资产通道」的半成品清偿） |
| **范围** | SPEC M9-WP03 §2.3 + §3「分区展示测试」验收 + §5 清单（index.html / app-core-v3.js / styles-v3.css / commands.rs / ui_hardening.rs）+ 本卡 |
| **创建日期** | 2026-10-04 |
| **来源** | SPEC M9-WP03 批准（PR #134）+ M8-WP05-T01 语义旗舰（资产通道已实装）+ M9-WP02-T04 memory_search 工具（sidecar 已入 MCP 面） |

## 契约落点（SPEC §2.3 逐条）

1. **单一输入框保持**：检索 tab 输入框 + 三态 radio（关键词/语义/含转写）
   不动；其后新增「含记忆」checkbox（`#mode-memory`，**默认勾选**）。
2. **并行调用**：勾选时
   `Promise.all([call(cmd, searchArgs(q)), memPromise])`——资产通道 IPC
   参数**不变**（`searchArgs(q)` 现状，勾选只在 UI 层追加并行支路）；
   记忆通道 `memCall("memory_search", { query: q })`（payload 恰为
   `{query}`，SPEC §2.3 字面；不传 limit/offset，sidecar 服务端默认）。
3. **分区展示**：资产区行渲染现状不变（hit 卡原样），勾选开启时置
   `// 资产通道 · {mode}检索` 分区标题（section-tag）；记忆区标题
   `// 记忆通道 · memory_search`，行 = content 截断（`trunc 90` + esc）+
   tags（`memTags` + esc）+ score（`toFixed(2)`），panel-table 三列。
   未勾选时渲染路径与现状完全一致（不注入分区标题）。
4. **跨通道不混排（§6-R2）**：两区各自内部排序不变；不合并数组
   （无 `[...rows, ...mem]` / `concat`）；记忆通道失败不拖垮资产区
   （catch → 记忆分区 `.empty` 错误行；error-region 文本由
   `call()` / `mcPayload` 既有链路透传）。
5. **空态/错误态**：沿既有 `.empty` 样式（记忆无命中 / 通道不可用文案）。
6. **Rust 侧零改动**：ipc.rs 无新 command / 新参数（§5 清单中 ipc.rs
   的 include_transcript 已随 T01 合入，本任务不触及）。

## 测试（SPEC §3「分区展示测试」；R5 落锤分工）

1. **commands.rs** `t03_flagship_memory_dual_channel_via_stub_sidecar`：
   bash stub 沿 T02 `t02_memory_panel_dataflow_via_stub_sidecar` 模板
   （请求行旁路落盘 + 按 `params.name` 回 CallToolResult 形状）；
   BM25 直种资产文档（沿 T01 `t01_search_include_transcript_toggle_wiring`
   判例）+ stub 回记忆行——断言：资产通道 `search` IPC 照常命中（资产区
   不受「含记忆」影响）+ `memory_search` 请求 payload 恰为 `{query}` +
   响应行字段（content/tags/score）可驱动记忆分区渲染。
2. **ui_hardening.rs** 静态探针（R5 落锤：JS 展示面 = Rust 静态契约）：
   index.html checkbox 默认勾选 + JS `.checked` 读取 + `Promise.all`
   并行字面量 + `{ query: q }` payload 字面量 + 双通道分区标题字面量 +
   记忆行三字段 esc 覆盖 + 资产行模板区间零记忆字段（不混排）+
   合并数组模式零命中。

## 边界与既有决定

- GUI 实操截图 = T04 人工（AGENTS.md 桌面端验收规则例外面：本任务
  不操控 GUI，PR 正文标注截图挂 T04）。
- 记忆行复用 T02 `memCall` / `memTags` / `trunc` helper（function 声明
  提升，doSearch 调用点无时序问题）；tags 解析失败原样透出（esc 兜底）。
- `.mem-toggle` 样式为 styles-v3.css 新增小节（勾选视觉沿 v4.3 tokens：
  accent-color var(--green)）；不新增主题/设计语言（SPEC §4）。
- 零新增依赖（纯前端 + 既有测试基建）。

## 验收

- [x] index.html：检索 tab `#mode-memory` checkbox 默认勾选，无 inline
      事件 handler；
- [x] app-core-v3.js：§2.3 六条契约全接线，新增动态插值全部 esc()
      （数字 length/toFixed 沿判例不入禁列）；
- [x] commands.rs stub 双通道数据流测试绿（资产区不受影响 + payload
      逐键断言）；
- [x] ui_hardening.rs 新探针绿（勾选接线 + 分区字面量 + 不混排）；
- [x] fmt/clippy/test 三件套全绿；既有测试零回归；
  （`cargo fmt --all --check` exit 0；`cargo clippy --workspace
  --all-targets -- -D warnings` Finished 无告警；`cargo test
  -p partisync-desktop`：commands 21 passed + ui_hardening 8 passed，
  1 ignored = T01 hybrid 需下载模型权重判例）；
- [x] 提交挂 Task-ID `M9-WP03-T03`；GUI 截图挂 T04。
