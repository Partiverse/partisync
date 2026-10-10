# Task: M11-WP05-T03 语义向量检索二期立项评估(T2 触发后)

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP05-T03 |
| **类型** | docs-only(立项评估件,只评估不实施) |
| **来源** | memory-vector-eval §6 触发纪律 + T2 触发(M11-WP05-T02)+ 工作流 T3 冒烟实测 |
| **创建日期** | 2026-10-10 |

## 交付

docs/reviews/M11-WP05-t03-vector-initiation-eval.md:T1–T4 逐条现状
(T3 半清偿——BGEM3 冒烟实测 1024d/89.1ms/权重 blake3 备料,BGESmall
ZHV15 冒烟 3 次代理断流未完成;T1 实测未满足——全机 memory 叶量峰值
3 行;T2 满足;T4 部分)→ **实施立项暂缓(T1 硬条件)**,转准备态
清偿清单(MANIFEST blake3 填实/BGESmallZHV15 冒烟重试/维度契约随
WP04 SPEC 落锤);S4 对齐判定可达;维度契约修正登记(BGEM3=1024d
证伪 768d 直连旧假设)。

## 涉及文件清单(Iron Rule 9)

docs/reviews/M11-WP05-t03-vector-initiation-eval.md(新)·
docs/tasks/M11-WP05-T03-vector-initiation-eval.md

## 验收

- [x] T1–T4 逐条现状全部工作流实测/核验支撑(eval-inputs 汇总)
- [x] 判定诚实:实施暂缓理由(T1)明确,无伪造触发
- [x] docs-only;零代码 diff;零新依赖;CI 绿
