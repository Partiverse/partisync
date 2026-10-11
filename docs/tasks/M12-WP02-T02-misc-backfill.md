# Task: M12-WP02-T02 WP02 SPEC T06 自指框 + T08 判据指针校准

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M12-WP02-T02 |
| **类型** | docs-only（回填小尾巴，M11-report §5 债表行 2/8 清账） |
| **来源** | M12-WP00-T01 charter WP02-T02 + M11-report §5 行 2/行 8 + §7 抽查发现 |
| **创建日期** | 2026-10-11 |

## 交付

①docs/specs/M11-WP02.md：§3.1 末项 T06 自指框勾闭（#214 执行项漏勾，回填注记）+ §3.0 任务表 T02–T06 状态格补勾（✅ #210/#211/#213/#213/#214——五格同为关账漏勾，与债表行 2 同类，一并清账）；②docs/tasks/M11-WP01-T08-cli-fulltest.md：任务卡表新增「判据精确定义」行（指向 ga-cadence §1 J1 门槛③；trailer 所指 M10-WP06-release-cadence.md 为谱系母本的注记——git trailer 不可改写历史，卡面补记为正确载体）。

## 涉及文件清单（Iron Rule 9）

docs/specs/M11-WP02.md · docs/tasks/M11-WP01-T08-cli-fulltest.md · docs/tasks/M12-WP02-T02-misc-backfill.md（本卡）

## 验收

- [x] T06 自指框勾闭且子项（SPEC 全勾/批准日期/WP 图状态）确证已由 #214 履行；
- [x] §3.0 状态格逐格对 PR 号核验后补勾，不假绿；
- [x] T08 判据指针落卡面（不动 git 历史）；
- [x] docs-only；纯三文件；零代码 diff；
- [x] 提交挂 Task-ID `M12-WP02-T02`，CI 全绿后 squash 合入。
