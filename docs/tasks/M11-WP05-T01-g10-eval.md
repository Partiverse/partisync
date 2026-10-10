# Task: M11-WP05-T01 G10 AI workflow 编排四问评估件

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP05-T01 |
| **类型** | docs-only(条件评估件,只评估不实施;沿 M10-WP04-T05 判例) |
| **来源** | M11 提案 §3-WP05 + §4-4c(已拍板默认生效)+ M10-WP00 §2 #4 欠账 + M8-roadmap §3 G10 锚点 |
| **创建日期** | 2026-10-10 |

## 交付

1. docs/reviews/M11-WP05-g10-workflow-eval.md——四问评估件:
   范围(§1 锚点管线切分/MVP 切片)、威胁面(§2 五条:内容注入链/
   规则篡改/动作升级/常驻面暴露/资源耗尽)、依赖面(§3 零新依赖 MVP
   成立 + 三条 ADR 触发点)、立项建议(§4 M12 立项候选 + 前置拍板项)
2. **T2 联动实证归档** docs/reviews/M11-WP05-t2-query-evidence.txt——
   「语义近似但字面不同」查询确定性失败实录(revenue report/会议纪要
   双语双查 0 命中 + fermentation 字面 sanity 命中;Kubuntu 本机 CLI
   只读检索,沿 D19 验证模式)——**候选 4 立项评估 T2 触发条件满足**

## 涉及文件清单(Iron Rule 9)

docs/reviews/M11-WP05-g10-workflow-eval.md(新)·
docs/reviews/M11-WP05-t2-query-evidence.txt(新)·
docs/tasks/M11-WP05-T01-g10-eval.md

## 验收

- [x] 四问齐备(范围/威胁面/依赖面/立项建议),全部结论有 path:line
      或命令实录支撑;诚实登记节齐(未跑向量对照/未做 DSL 原型)
- [x] T2 实证归档且候选 4 触发判定落记(触发评估 ≠ 实施)
- [x] fmt 不适用(docs);零代码 diff;零新依赖;提交挂 Task-ID
      `M11-WP05-T01`;三门禁绿(CI 复核)
