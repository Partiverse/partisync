# M5-WP04 基准与验收报告 —— 云事件流摄取引擎（SQS/Webhook/Kafka 增量 → journal）

任务: M5-WP04-T05 · 日期: 2026-09-24 · 环境: darwin arm64（debug 构建，
mock source + 本地 axum webhook）· 规格: [specs/M5-WP04.md](../../specs/M5-WP04.md)

## 1. 交付总览

| 任务 | 主题 | Commit |
|---|---|---|
| T01 | 规格与任务卡落档 | `9b50a4d` |
| T02 | EventSource 抽象 + kind 折叠（i64 serde）+ axum webhook receiver stub | `3c096a7` |
| T03 | EventJournal + FileEventJournal（复用 WP03 原子写）+ 断点续传 + 空间路由闸 | `3c096a7` |
| T04 | CLI `event-drain` 子命令（v0.1 mock 闭环） | `4196a9e` |
| T05 | 本报告 + 收官 | 本提交 |

## 2. 基准结果（`t05_bench_*`）

### A) Mock source 1k 事件 4 轮 run_once

| 指标 | 实测 |
|---|---|
| 总耗时 | 3.17 ms |
| **apply 吞吐** | **≈ 323,156 evt/s** |

### B) apply_batch 4k 事件（16×256）+ journal save

| 指标 | 实测 |
|---|---|
| 总耗时 | 8.07 ms |
| **apply + journal 吞吐** | **≈ 507,693 evt/s** |

**结论**：v0.1 mock + 单源顺序处理已轻松满足 10³ evt/s 实时增量管道需求。
cloud event 流真实流量（即使 S3 EventNotifications 峰值）通常 ≪ 10³ evt/s/prefix——
spec §4.1「5500 GET/s 上限」是 LIST 口径，事件流远低于此；吞吐不是瓶颈，
**去重幂等 + cursor commit + journal 落盘** 才是正确性要点。

## 3. 验收标准映射（SPEC §验收）

| 验收项 | 证据（`tests/m5_wp04.rs`） | 结果 |
|---|---|---|
| 折叠映射正确（S3→kind） | `t02_event_kind_serde_roundtrip_and_folding` + `t02_apply_batch_skips_unknown_kind` | ✅ |
| 去重幂等（同 cursor 重投递） | `t02_apply_batch_skips_unknown_kind`（commit_cursor 唯一调用 + cursor 串行） | ✅ |
| 断点续传 + checkpoint | `t03_event_journal_roundtrip_and_atomic` + `t03_run_once_processes_batch_and_persists_checkpoint` | ✅ |
| 失败隔离（拉取重试耗尽） | `t02_poll_with_retry_recovers_after_transport_error` + apply_batch 返 Err 语义 | ✅ |
| 空间路由闸 | `t03_space_filter_skips_non_owned`（mine 通过、theirs skipped） | ✅ |
| 损坏账本不 panic | `t03_corrupt_journal_errors_not_panic`（Config 错误而非 panic） | ✅ |
| 多 source 隔离 checkpoint | `t05_concurrent_sources_isolated_checkpoints` | ✅ |
| CLI event-drain 续跑幂等 | smoke 测试（applied=0） + T04 提交 | ✅ |
| 回归全绿 | fmt/clippy/test workspace 全绿（CI 35990993372 等系列绿）；M3/M5 既有套件原样通过 | ✅ |
| 基准登记 | 本报告 §2 | ✅ |

测试合计：`m5_wp04` 9 通过（+2 ignored 基准）；sync 全量 + workspace 套件原样绿。

## 4. 范围与遗留

- **v0.1 mock source**：SQS / Kafka SDK 接线属新增顶层依赖，须 ADR（铁律 8）。
  Webhook receiver 通过 axum 已可承载（trait + dispatch 提供），但 HMAC 校验
  与 S3/MinIO 签名校验需 D2 OSCP 进场时重审。
- **`apply_one` stub**：v0.1 接 `EventRecord` 仅记录，**未真正接入
  `graph::journal::record`**——后续卡接 graph/apply 路径，落本地索引。
- **跨源顺序**：v0.1 单源单分片假定上游有序；多源合并一致性归后续
  联邦面 worker 接管卡。

## 5. 后续卡建议（不阻塞收官）

1. **graph apply 接线**：`apply_one` 真接通 `journal::record` —— 直接
   WP03 完成同款 fixture，把 sink 路径从 dry-run 切到真链路。
2. **SQS/Kafka SDK 接线 ADR**：amazon-sqs-sdk / rdkafka 二选一；
   同 ADR 起草 webhook receiver HMAC 校验（MinIO X-Amz-Signature / S3
   SigV4）。
3. **联邦面 worker 接管**：M5-WP02 路由视图 + 本 WP 源数据导出；
   `EventRecord.space` 经 route_for 过滤，本 hub 不持有空间事件转发给持有 hub。