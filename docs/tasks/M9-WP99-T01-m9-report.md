# Task: M9 里程碑报告（M9-WP99-T01）

> **范围外**：G3 三签（架构/评审/安全）明确不在本任务范围——报告
> 「放行签字（G3）」节留三签空位标注待人工，沿 M6-WP99-T01 /
> M8-WP99-T01 判例由用户指令后另行落档。不修复任何债；台账所列项
> 均已有登记处（SPEC §4/§5 / 关账卡），本报告只汇总不处置。
> 不发布 GitHub Release、不 publish crate、不操控 GUI。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP99-T01 |
| **类型** | 里程碑关账工件（审计即工件，铁律 10） |
| **优先级** | P0（M9 关账前置） |
| **范围** | `docs/reports/M9-report.md`（本 PR 新建）+ 本任务卡 |
| **创建日期** | 2026-10-05 |
| **来源** | 用户会话指令；判例：M6-WP99-T01（PR #24）/ M8-WP99-T01（PR #113）/ M7 关账（PR #53） |

## 交付物

1. `docs/reports/M9-report.md`：沿 M8-report 八节结构（范围与结果 /
   KPI / 测试证据 / 安全 / ADR 与债务 / AI 披露 / 抽查审计 / 下一阶段
   建议）+ G3 三签空位节。骨架由 `cargo xtask report M9` 生成（骨架即
   占位约定，M8 判例），逐节数字现场取证回填：
   - 范围：`2a3ebb5..4b0f790`（M8 关账后 → WP06-T01，36 commit /
     35 merged PR：#114–#123、#126–#150；#124/#125 CLOSED 被
     #126/#127 取代重开）；
   - 数字一律以仓库工件为准（SPEC 勾选、任务卡记值、properties.md、
     deny.toml、git log 实测），性能记值注记出处，不凭记忆；
   - AI 披露：M9 全部任务提交由 GLM-5.3-Flash (ZCode) 执行；
   - 债台账含：T04 GUI 三态截图与全 tab 巡检待人工、CAS 内容重组
     缺口（WP03-T06 新债）、外部审计窗口资金回笼触发、
     scanner_enobufs 覆盖边界挂账、示例扩展断签窗口（WP04 §6-R5，
     本机 e2e 实证）、WP05-T02/WP06-T02/T03 未开工项；
2. 本任务卡落档 `docs/tasks/`。

## 验收

- [x] 报告沿 M8 结构逐节落盘，G3 节留空位标注待人工（不代签）；
- [x] 抽查审计：`cargo xtask trace` ×5（WP01-T03 / WP02-T02 /
      WP03-T06 / WP04-T02 / WP05-T01）全链 PASS，记录入报告 §7；
- [x] 现场取证留痕：fmt/clippy 本地绿；workspace 测试 1 例环境耦合
      e2e 失败（`mcp_call_real_sidecar_ext_list`，WP04 强制验签 ×
      本机存量未签名 demo 扩展，CI 无预置文件自跳过不受影响）——
      如实登记报告 §3/§5，不隐瞒不硬造；
- [x] 纯 docs 提交，不触碰代码/门禁/依赖文件；
- [x] 提交挂 Task-ID `M9-WP99-T01`，CI 全绿后 squash 合入。
