# Task: M10-WP03-T03 浏览列表 UX（表头客户端排序 + mtime 相对时间）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP03-T03 |
| **类型** | 实施（R0：前端增强，纯客户端） |
| **范围** | desktop ui/app-core-v3.js + ui/index.html + ui/styles-v3.css + tests/ui_hardening.rs + 截图 + 本卡（零 IPC / 零引擎 / 零 gateway） |
| **创建日期** | 2026-10-06 |
| **SPEC** | docs/specs/M10-WP03.md §2.3 / §3 |

## 根因（代码现状）

浏览表头四列静态 th（index.html:44），名称/大小/修改时间不可排序；
mtime 列只有绝对本地时间（`timeFmt`，app-core-v3.js:43-48）——与
M10-WP02 指认的记忆面板时间问题同型。`list` → `children()` 全量返
回无分页（ipc.rs:90-96），客户端全集排序无窗口语义问题。

## 修复

1. **表头排序（客户端）**：名称 / 大小 / 修改时间三列可排（内容身份
   列除外）；点击排序当前行集、再点反序、第三点回服务端序（children
   返回序）；箭头指示键与方向；**换目录（面包屑/目录行）重置默认序**
   （沿 M10-WP02 T01 判例）；键相同时目录行（kind=1）恒在文件行前
   （次级键 kind 降序）。
2. **mtime 相对时间**：复用 `relTime`（app-core-v3.js:473）+ **>30 天
   回落绝对日期**（relTime 现无回落档——本卡自含实现，不依赖
   M10-WP02-T03 顺序；其先合入则直接复用）；title = timeFmt 完整
   本地时间；mtime_ns 缺失/0 → 既有「—」。
3. **空态不回退**：browse 空态文案（D6 动作邀请，app-core-v3.js:118）
   维持原文。

## 验收

- [x] 静态探针：三列点击排序/反序/回默认 + 箭头指示 + 换目录重置 +
      目录优先次级键 + relTime 分档与 >30 天回落 + title + 「—」回落
      + 空态文案不回退 + 无未转义插值。✅ ui_hardening 5 新探针先红
      后绿（4 新探针初跑红 → 实现 → 28/28 全绿）；裸插值禁列增补
      `${e.mtime_ns`；
- [x] GUI 实操截图 `docs/screenshots/M10-WP03-T03-*.png`（8 帧）：
      sort-before / sort-name-asc（▲+目录优先）/ sort-name-desc（▼+
      目录仍恒前）/ sort-back-to-server-order（第三点回服务端序）/
      sort-mtime-desc / sort-size-asc / sort-reset-after-nav（进 d0
      后箭头消失，列表与 `partisync-cli ls /d0` 逐行对账）/
      mtime-fallback-30d（临时 `--data-dir` 演示库三档同框：刚刚 /
      26 天前 / 2026/8/20 回落绝对）。诚实口径：title 悬浮 tooltip
      视觉帧不可稳定捕获（cliclick 合成 hover 不触发 WKWebView title
      tooltip + 5s 轮询 innerHTML 重渲打断 hover 链，六轮重渲后即时
      hover 仍未得；与 M8-WP05 §6 合成事件限制同源）——title =
      timeFmt 接线以真实实例 AX 对账为证（mtime 列 18 枚 AXCell
      description = 完整本地时间戳 `2026/9/18 15:35:12` 形态）；
- [x] fmt/clippy/test 绿；改动仅限本卡范围；零新增顶层依赖。✅
      fmt 一次 rediff 经 `cargo fmt --all` 修正；clippy -D warnings
      0 warning；`cargo test --workspace` exit 0（ui_hardening
      28/28，其余套件不回归）。
