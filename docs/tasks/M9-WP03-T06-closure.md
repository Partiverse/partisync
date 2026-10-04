# Task: M9-WP03-T06 WP03 关账盘点（文档关账件）

> **编号冲突注记**：Task-ID `M9-WP03-T06` 与既有实施卡
> `M9-WP03-T06-reindex.md`（PR #137 已合入）同号——关账盘点沿「挂该 WP
> 末位实施件 ID」判例（hook 要求 WP-T 形式；M9-WP02-T05 关账判例
> PR #133），本卡为纯文档关账件，与 reindex 实施卡互不覆盖、无文件交集。
> **范围外**：GUI 三态截图 + 全 tab 巡检挂 T04 人工（本任务不操控 GUI）；
> M9-WP00 §4 债表「escapeHtml F2…」承接行回填不在本卡指令范围（随 T04/G3）。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP03-T06（关账盘点沿用末位 ID 判例） |
| **类型** | 文档关账（R0——纯 docs，零代码触面） |
| **优先级** | P0（WP03 实施面关账前置件） |
| **范围** | docs/specs/M9-WP03.md §3 勾选注记 + docs/reviews/M8-WP05-ui-report.md D2/D3/D4 清账注记 + docs/specs/M9-WP00.md §1 WP03 行状态回填 + 本卡（M9-WP00.md 沿 WP01/WP02 关账 PR #121/#133 触面判例） |
| **创建日期** | 2026-10-04 |
| **来源** | WP03 实施件 T01/T02/T03/T05/T06 全合入（PR #135–#140，main=`e1eba80`）+ 用户关账指令（2026-10-04） |

## 盘点动作

1. **SPEC §3 九项验收逐条对照实际交付勾选**：证据注记 = PR 号 + 测试名
   （ui_hardening.rs 探针 8 具 / commands.rs 桌面测试实名）；**T04 GUI
   三态截图 + 全 tab 巡检 = 人工项，标注「待人工 GUI」不勾**。
2. **M8-WP05-ui-report 债表 D2/D3/D4 清账注记**（沿 D5/D6 清账行样式）：
   D2 esc() 统一已由 T01 清偿（PR #135 三探针）；D3 空库路径已有
   `t02_asset_detail_empty_db_returns_empty_copies` 覆盖（T01 验证后确认
   既有，未新增重复测试）；D4 开关已接线（IPC `Option<bool>` 显式传值 +
   bm25 `parser_no_tx` 补齐，false 真实生效）。
3. **M9-WP00 §1 WP03 行状态回填**（沿 WP01 行「已收官」样式）：实施面
   已收官（PR #135–#140），GUI 验收挂 T04。

## 验收

- [x] 三文件注记落盘，勾选与交付证据一一对应（本 PR diff 即证）；
- [x] 勾选纪律：人工项（SPEC §3 第 5 项）不勾、标注「待人工 GUI」；
- [x] `cargo fmt --all --check` / `cargo clippy --workspace --all-targets
      -- -D warnings` 本地绿（纯文档 diff 零 Rust 触面；clippy
      --all-targets 覆盖测试代码编译；workspace 测试由 CI 兜底）；
- [x] 提交挂 Task-ID `M9-WP03-T06`（冲突注记见卡首）。
