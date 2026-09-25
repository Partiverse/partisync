# M5-WP06 收官报告 —— 夜间混沌测试套件（Chaos Suite）

任务: M5-WP06-T04 · 日期: 2026-09-25 · 规格:
[specs/M5-WP06.md](../../specs/M5-WP06.md)

## 1. 交付总览

| 任务 | 主题 | Commit |
|---|---|---|
| T01 | 规格与任务卡 | `a490071` |
| T02 | hub 混沌补缺（并发 claim race / 崩溃分区叠加） | `5ec74ed` |
| T03 | sync 混沌补缺（双 failpoint 叠加 / 环形分区对账收口） | `f1b8661` |
| T04 | nightly workflow + 本报告 | 本提交 |

## 2. 混沌场景矩阵（场景 × 来源 × 测试）

| 场景 | 来源 | 测试 |
|---|---|---|
| UploadAck 传输超时 / FlakySink 恢复 | M5-WP01 既有 | `hub::m5_wp01`（7 测） |
| raft leader crash 演练 / failover | M3-WP02 既有 | `hub::wp02`（19 测） |
| 联邦连接分区：陈旧视图照答 + 重连收敛 | M5-WP02 既有 | `hub::m5_wp02::t04` |
| scan 崩溃断点续传矩阵（5 场景） | M5-WP03 既有 | `sync::m5_wp03::t04*` |
| oplog failpoint + journal 重放 | M2-WP09 既有 | `sync::wp09` |
| **联邦并发 claim race（20 轮真实并发）** | **WP06 新增** | `hub::m5_wp06::t02_concurrent_claim_race_converges` |
| **崩溃分区叠加（raft 停机 × 连接分区 × 重启收敛）** | **WP06 新增** | `hub::m5_wp06::t02_leader_crash_federation_query_and_recover` |
| **HubService 路由账本重开持久** | **WP06 新增** | `hub::m5_wp06::t02_hub_service_route_for_survives_registry_reopen` |
| **scan + event 双 failpoint 并发启用独立恢复** | **WP06 新增** | `sync::m5_wp06::t03_layered_failpoints_recover_independently` |
| **环形三节点分区 → push 漏行 → reconcile 收口** | **WP06 新增** | `sync::m5_wp06::t03_ring_partition_relay_then_full_convergence` |

## 3. 关键语义发现（T03 环形分区）

M2-WP09 push 语义实证：oplog 行应用后**全局 trim**（`push_opts` 内
`src.trim_oplog`）——**无中继转发**。环形分区（A↔B 断、B↔C 通）下 A
漏看的行在恢复后经 push **不可达**（行已被 trim）。漏行由 `reconcile`
全量对账收口——这正是「WP03 对账兜底」设计裁定的混沌实证：push =
低延迟增量通道，reconcile = 正确性最终保障，两者缺一不可。

并发 claim race 的不变量（裁定 2 修订）：epoch 并发竞争使 winner 可在
hub 间轮转（知情更高 epoch = 合法归属转移），**不变量 = 双侧视图收敛 +
epoch 单调不减**——SPEC 原文「同 epoch tie-break hub_id 小者」仅在双方
提案同 epoch 时成立，测试已按此校准。

## 4. Nightly 入口

`.github/workflows/nightly.yml`：cron `0 18 * * *`（UTC，北京 02:00 低峰）
+ `workflow_dispatch` 手动触发。Linux 单 job：全量 `cargo test --workspace`
+ ignored 冒烟（WP03 worker 扩展、WP05 元数据仿真、WP04 吞吐）。
**不触碰 ci.yml 门禁**（铁律红线）；红灯 = 报告性质（artifact 留痕 14 天）。

模糊测试口径（裁定 6）：cargo-fuzz 需 nightly 工具链 + 新依赖授权（铁律 8）
不立项；随机化口径由既有 proptest 资产代表（M3 投影一致性 proptest、
HLC 属性测试等），随全量套件进夜间。

## 5. M5 里程碑全景（收官时点）

| WP | 状态 |
|---|---|
| WP01 UploadAck | ✅ 6/6 |
| WP02 联邦路由 | ✅ 7/7 |
| WP03 扫描调度器 | ✅ 6/6 |
| WP04 云事件流 | ✅ 5/5 |
| WP05 真实规模基准 | ✅ 3/3（D5 清偿；D6/D7 gated） |
| **WP06 混沌套件** | **✅ 4/4（本报告）** |

遗留跨 WP 后续卡：usearch add 吞吐（WP05 阻塞项）、SQS/Kafka SDK ADR +
HMAC（WP04）、graph apply 接线（WP04）、联邦批量合入（WP02）、
semaphore 协调（WP03）、D6/D7 解锁补录。
