# Task: M12-WP01-T01 GA 先行件——release.yml 去 rc 段 + CHANGELOG [0.2.0] 节 + rc-feedback 渠道登记

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M12-WP01-T01 |
| **类型** | docs-only + CI 工作流口径修订（cadence §3.1 三轴已确认项的执行件；非门禁阈值面） |
| **来源** | M12-WP00-T01 charter WP01-T01 + M11-WP01-ga-cadence.md v1.1 §3.1（tag/产物名/CHANGELOG/渠道推荐案，2026-10-11 用户三轴确认） |
| **创建日期** | 2026-10-11 |

## 交付

①`.github/workflows/release.yml`：产物名 7 处 `0.2.0-rc.1` → `0.2.0`（cadence §3.1「模板去 -rc.N 段」原案）；头部注释 rc 口径 → GA 口径注记（触发器 `v*` 通配不变，零门禁语义改动）；②`docs/release/CHANGELOG.md`：新增 `[0.2.0]` 草稿节（rc.1 → GA 增量：WP06 向量检索/WP04 stored 深化/LCSTS 导入器/AGPL-0034/中文高亮修复；**rc 已知限制清单逐项复核** = 清偿 2/延续 9/T08 新增 4，不假绿）；rc.1 节状态行由「草稿待 publish」修正为已发布事实（2026-10-10 + 发布报告链接）；③GitHub 仓库面：创建 `rc-feedback` issue 标签（cadence §2 反馈渠道登记，本次三轴确认项）。

## 涉及文件清单（Iron Rule 9）

.github/workflows/release.yml · docs/release/CHANGELOG.md · docs/tasks/M12-WP01-T01-ga-prep.md（本卡）

## 验收

- [x] release.yml 仅产物名与注释口径改动（jobs/步骤/签名/SBOM/repro 逻辑零触碰；触发器不变）；
- [x] CHANGELOG [0.2.0] 节增量与 `git log v0.2.0-rc.1..main` 实测对账（代码面 5 笔 feat/fix + 许可面全数登记）；rc 清单逐项清偿/延续/新增三态标注；
- [x] `rc-feedback` 标签已创建（GitHub 仓库面）；
- [x] 不 tag、不建 Release、不 publish（J1 计满 + J4 复跑 + 用户终审门，cadence v1.1 红线）；
- [x] 提交挂 Task-ID `M12-WP01-T01`，CI 全绿后 squash 合入。
