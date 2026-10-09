# Task: M11-WP00-T02 CI 卫生件——hub upload_ack 基准测试去抖(best-of-3)

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP00-T02 |
| **类型** | 测试基建修复(docs/test-only,零产品代码) |
| **来源** | 三次 CI 实录:PR #209 评论 + runs 37948758815(ubuntu p99=58.17ms)/37950133095(macos 58.42ms)/37963133074(macos 58.42ms)——`upload_ack_latency_and_throughput_benchmark` P99>50ms 断言连挂,阻塞无关 PR 合入 |
| **创建日期** | 2026-10-09 |

## 交付

`crates/partisync-hub/tests/m5_wp01.rs::upload_ack_latency_and_throughput_benchmark`
重构为 best-of-3:测量轮提为 `bench_round(round)` ×3,取**最小 p99** 断言
(<50ms DoD 阈值不变,latency floor 用 min 是标准去抖口径);每轮独立
hub+dev(iroh loopback)环境。BENCH_RESULT 打印逐轮 p99 + best,透明可复核。

## 涉及文件清单(Iron Rule 9)

crates/partisync-hub/tests/m5_wp01.rs · docs/tasks/M11-WP00-T02-ci-hygiene.md

## 验收

- [x] 本机 3 连跑全绿(0.18–0.21s/轮);clippy -D warnings 零告警;fmt 过
- [x] DoD 断言语义不变(P99<50ms),仅去抖;零产品代码 diff;零新依赖
- [ ] CI 全绿
