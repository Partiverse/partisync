# Task: M12-WP04-T01 CLI 交互能力强化细化提案（征求意见稿）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M12-WP04-T01 |
| **类型** | docs-only（细化提案，用户拍板后才立 SPEC——铁律 1） |
| **来源** | M12-WP00-T01 charter WP04（用户拍板主线）+ 全仓 CLI 面盘点（子代理，path:line 在案） |
| **创建日期** | 2026-10-11 |

## 交付

docs/reviews/M12-WP04-cli-enhancement-proposal.md（新）v0.1：现状快照（16 动词手写分发/零 --json 零 --help/未知 flag 静默吞/退出码不一致 ls=1 vs find=0/CLI↔partisd 完全脱节——CLI search 恒 BM25 降级 vs partisd hybrid/T08 ⑩ 微债）+ 四方向包（P1 机器可读契约层/P2 partisd 客户端打通含 ADR 前置依赖面/P3 UX 硬化/P4 ga 式 DX 反哺 doctor）+ 推荐排期（P3→P1→P2→P4，实施落 v0.2.0 tag 后）+ 待用户拍板三项（范围排序/P2 实现路径/T 轮粒度）。

## 涉及文件清单（Iron Rule 9）

docs/reviews/M12-WP04-cli-enhancement-proposal.md（新）· docs/tasks/M12-WP04-T01-cli-proposal.md（本卡）

## 验收

- [x] 现状快照全部 path:line 取证（盘点为子代理全表核验，非记忆）；
- [x] 四包均标依赖面与风险（P2 新顶层依赖 → ADR 前置显式标注，铁律 8）；
- [x] 提案不产码、不立 SPEC（待用户拍板范围后走铁律 1 流程）；
- [x] docs-only；纯两文件新增；零代码 diff；
- [x] 提交挂 Task-ID `M12-WP04-T01`，CI 全绿后 squash 合入。
