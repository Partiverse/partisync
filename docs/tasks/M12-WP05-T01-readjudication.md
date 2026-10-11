# Task: M12-WP05-T01 候选 4 复裁评估——前置条件核销 + 判定清账 + charter 勘误

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M12-WP05-T01 |
| **类型** | docs-only（复裁评估件 + charter 勘误） |
| **来源** | M12-WP00-T01 charter WP05-T01 + M11-WP05-t05 评估件「放行（有条件）」判定 |
| **创建日期** | 2026-10-11 |

## 交付

①docs/reviews/M12-WP05-candidate4-readjudication-eval.md（新）：t05 四项放行前置条件逐项核销（SPEC 四件落锤→M11-WP06.md §2 / ADR-0035 / R2 评审面→#240 对抗审查处置 / 依赖方向→fastembed feature 门控 + graph 核心仅 usearch pin）+ T1–T4 终态 + **判定「有条件放行 → 条件全部核销，候选 4 主体清账（已实施）」** + 剩余诚实边界转观察项（位级确定性/演示级收益/真实负载，随 rc-feedback 收集）；②docs/reviews/M12-roadmap-proposal.md：WP05 行勘误（「实施仍待 S4 窗口」系笔误——WP06 已实施，S4 仅为外部需求锚）+ 修订记录 v1.1。

## 涉及文件清单（Iron Rule 9）

docs/reviews/M12-WP05-candidate4-readjudication-eval.md（新）· docs/reviews/M12-roadmap-proposal.md（勘误修订，本卡为触发方）· docs/tasks/M12-WP05-T01-readjudication.md（本卡）

## 验收

- [x] 前置条件核销全部基于仓内 path:line / PR 号硬证据（Cargo.toml:22/:26/:32 依赖门控、#240 阻断项落锤、models.rs:58-88 钉版）；
- [x] 判定升级留痕完整（触发热纪律全链：触发→t03 暂缓→t05 有条件放行→WP06 实施→本件清账）；
- [x] charter 勘误如实（笔误承认 + 修订记录登记，不静默改写）；
- [x] docs-only；纯三文件；零代码 diff；
- [x] 提交挂 Task-ID `M12-WP05-T01`，CI 全绿后 squash 合入。
