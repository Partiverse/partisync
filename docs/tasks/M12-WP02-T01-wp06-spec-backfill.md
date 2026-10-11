# Task: M12-WP02-T01 WP06 SPEC §3 十一项验收逐证据勾闭

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M12-WP02-T01 |
| **类型** | docs-only（SPEC 验收回填，M11-report §5 债表行 1 清账） |
| **来源** | M12-WP00-T01 charter WP02-T01 + M11-report §5 行 1（「实施/实测 PR #236–#241 已合入、T06 证据件在库，勾闭为纯回填」） |
| **创建日期** | 2026-10-11 |

## 交付

docs/specs/M11-WP06.md：①§3 十一项验收框全勾（`- [ ]`→`- [x]`，零文本改动）；②头部批准日期落记（待合入 → 2026-10-11，#235 合入即批准）；③§3 尾新增「勾闭依据」块——11/11 逐项证据锚点（子代理独立核验：properties.md P25 / memory_vector.rs 维度契约与 key 派生 / wp06_memory_vector.rs 探针四臂 / mcp.rs 契约面 / T06 证据件 ⑤⑥⑦ 实测数字 / models.rs MANIFEST 钉版 / deny 六 PR 零触碰），实跑 `wp06_memory_vector` 12 passed + `wp06_surface` 1 passed；诚实边界沿证据件 §8 如实维持。

## 涉及文件清单（Iron Rule 9）

docs/specs/M11-WP06.md · docs/tasks/M12-WP02-T01-wp06-spec-backfill.md（本卡）

## 验收

- [x] 11/11 逐项有硬证据锚点方可勾（不可勾项留白并登记——实际无缺口）；
- [x] 勾选零断言放宽（仅 `- [ ]`→`- [x]`，SPEC 契约文本零改动）；
- [x] 批准日期落记沿 WP02 0.4 判例（合入即批准）；
- [x] docs-only；纯两文件；零代码 diff；
- [x] 提交挂 Task-ID `M12-WP02-T01`，CI 全绿后 squash 合入。
