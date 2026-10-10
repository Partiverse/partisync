# Task: M11-WP01-T06 B1 by-hash 断言处置拍板 A 落锤(重划断言口径)

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP01-T06 |
| **类型** | docs-only(拍板执行:checklist 口径重划 + 四处注记回填) |
| **来源** | M11-WP01 发布报告 §5 遗留项 1 + M10-WP06-T03 复测报告 §2 处置 A/B/C(推荐 A);用户 2026-10-10 裁决「A:重划断言口径」 |
| **创建日期** | 2026-10-10 |

## 交付

B1 by-hash 断言处置闭环:①RELEASE-CHECKLIST-beta.md §B1 拍板记录落锤
(断言口径 v2 = ①顺序读 by-hash ≥ 1/5 × 同轮透传护栏线;②≤4 MiB 对象
场景保留原断言;③>4 MiB 对象 open 整载语义豁免),checkbox 闭合;
②M10-WP06-T03 复测报告 §2/§3 补拍板结果注记;③M9-report §5 债表行
「待拍板」改已拍板;④M11-WP01-release-report-rc1 §3/§5 遗留项补后记。
处置 B(流式读改进债)不设立。

## 涉及文件清单(Iron Rule 9)

docs/release/RELEASE-CHECKLIST-beta.md ·
docs/reviews/M10-WP06-T03-fuse-cold-retest.md ·
docs/reports/M9-report.md ·
docs/reviews/M11-WP01-release-report-rc1.md ·
docs/tasks/M11-WP01-T06-b1-assertion-rescope.md(新)

## 验收

- [x] 拍板来源留痕(用户裁决 A,2026-10-10;非 AI 代裁)
- [x] 断言口径 v2 与复测实测数据自洽(顺序比 0.26/0.74 ≥ 1/5;
      64 MiB 热开 ~32ms 属豁免项;小文件口径未单测如实标注不追溯)
- [x] 四处挂账位全部回填(checklist 框闭合 + 复测报告 + M9 债表 +
      rc1 发布报告遗留项)
- [x] docs-only;零代码 diff;零新依赖;提交挂 Task-ID;CI 绿
