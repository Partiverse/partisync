# Task: M11-WP04-T01 检索摘要 stored 原文域代价评估(入线拍板前置)

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP04-T01 |
| **类型** | docs-only(代价评估件,实测支撑;评估红 = 诚实撤回) |
| **来源** | M11 提案 §3-WP04 + §4-5(入线但代价评估先行,默认生效 2026-10-09)+ M10-WP01 §4 碎片流债 |
| **创建日期** | 2026-10-10 |

## 交付

docs/reviews/M11-WP04-stored-field-cost-eval.md:机制修正(放大在写入
入口,STORED 落盘的已是扇出串)+ 实测(纯中文扇出 3.168x/.store
2.70~3.96x/目录 8.53~9.41x postings 主导;stored 翻倍全链增量 +1.8%)
→ **代价判定绿** → 入线建议(新增 stored-only 原文字段 + 反 fan-out
摘要,实施排期随检索深化 SPEC)

## 涉及文件清单(Iron Rule 9)

docs/reviews/M11-WP04-stored-field-cost-eval.md(新)·
docs/tasks/M11-WP04-T01-stored-cost-eval.md

## 验收

- [x] 代价判定有实测支撑(非纸面预估):存储域增量 +31% 修正 v0.1
      「~2x」高估;postings 主导域零增量
- [x] docs-only;零代码 diff;零新依赖;提交挂 Task-ID;CI 绿
