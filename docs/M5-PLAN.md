# M5 里程碑全景看板与自动化执行计划 (M5-PLAN)

> 本文档是 M5 里程碑全生命周期的自动化任务调度中枢。
> 遵循 [AGENTS.md](../AGENTS.md) 铁律：规格先行、原子交付、测试契约完备、零无聊依赖。

---

## 1. M5 工作包总览与推进状态

| 工作包 | 主题 | 负责人/模式 | 核心目标 | 状态 |
|---|---|---|---|---|
| **WP01** | 设备侧 UploadAck 协议与可靠传输 | AI 自动执行 | 解决 iroh push fire-and-forget，保证 Hub CAS 落库前不丢数据 | **已完成 (6/6) ✅** |
| **WP02** | 联邦路由协议 (Multi-Hub Federation) | 规范先行 + AI | 基于 space 前缀的多 Hub 路由协商与 Raft 视图同步 | **已完成 (7/7) ✅** |
| **WP03** | 分布式扫描调度器 | AI 自动执行 | 前缀分片并行 LIST、动态负载均衡与扫描断点恢复 | **已完成 (6/6) ✅** |
| **WP04** | 云事件流摄取引擎 | AI 自动执行 | SQS / Webhook / Kafka 增量事件流适配器与去重流水线 | **已完成 (5/5) ✅** |
| **WP05** | 真实规模性能基准 (10⁶ - 10¹² 仿真) | AI 压测评估 | 替代合成评估，真实多模态/图谱元数据规模化时延与内存评估 | **已完成 (3/3) ✅** |
| **WP06** | 夜间混沌测试套件 (Chaos Suite) | AI 自动执行 | 覆盖网络分区、断网重连、宕机恢复、并发竞争混沌测试 | **进行中 (1/4)** |

---

## 2. WP01 自动化任务卡分解 (Execution Contract)

WP01 总体目标：提供生产级端到端可靠 ACK 流协议，消除传输层数据丢失盲区。

### ✅ 已完成任务
- **M5-WP01-T01** (`3c7f8b0`): Hub 端 35B 定长帧编码与推送（`Accepted` 语义）。
- **M5-WP01-T02** (`9d50980`): Device 端 `UploadAcker` 接收器与状态机同步等待。
- **M5-WP01-T03** (`dde26ee`): Loopback 端到端基础、批量、超时 3 例测试全绿。
- **M5-WP01-T00** (`10b80f8`): WP01 规范规格书与 D2 OSCP 采购 RFP 落档。

### 🚀 待自动执行流水线任务

#### [Task] M5-WP01-T04: Duplicate 与 Rejected 状态支持与测试覆盖
- **目标**:
  1. Hub 端在接收 blob 前进行校验：若 CAS 中已有该 Hash（幂等命中），发送 `UploadAckStatus::Duplicate` 帧；若数据损坏或校验失败，发送 `UploadAckStatus::Rejected` 帧。
  2. Device 端 `UploadAcker` 接收并解析 `Duplicate` 与 `Rejected` 状态，对上层提供清晰分类枚举。
- **约束文件清单**:
  - `crates/partisync-hub/src/iroh_channel.rs`
  - `crates/partisync-hub/src/upload_acker.rs`
  - `crates/partisync-hub/tests/m5_wp01.rs`
- **DoD (验收契约)**:
  - 新增 `upload_ack_duplicate` 集成测试：重复 push 同一 Hash 时，断言返回 `UploadAckStatus::Duplicate`，CAS refcount 保持幂等。
  - 新增 `upload_ack_rejected` 集成测试：模拟传输内容校验失败时，断言返回 `UploadAckStatus::Rejected`。
  - `cargo test -p partisync-hub --test m5_wp01` 全绿。

#### [Task] M5-WP01-T05: UploadAcker 指数退避重试 (Backoff) 与容错机制
- **目标**:
  1. 在 `UploadAcker` 中集成生产级重试驱动器 `UploadRetryPolicy`。
  2. 当接收到 `Retrying` 状态码或单次等待超时（Timeout）时，自动触发指数退避策略（初始 100ms → 200ms → 400ms，最大 5 次），直至成功或抛出 `ExhaustedRetries`。
- **约束文件清单**:
  - `crates/partisync-hub/src/upload_acker.rs`
  - `crates/partisync-hub/tests/m5_wp01.rs`
- **DoD (验收契约)**:
  - 属性/单元测试验证退避时延计算正确性。
  - 集成测试 `upload_ack_retry_success`：前 N 次返回 Retrying，后续成功返回 Accepted，验证状态机可自动恢复。
  - 严禁引入任何未审批的第三方外部重试 crate（无聊依赖铁律）。

#### [Task] M5-WP01-T06: UploadAck 基准压测与性能评估报告
- **目标**:
  1. 编写微基准或压力测试，度量开启 UploadAck 机制与 fire-and-forget 原生推送的吞吐量差异与 P99 往返时延。
  2. 输出性能评测分析报告至 `docs/reports/bench/M5-WP01-upload-ack.md`。
- **约束文件清单**:
  - `crates/partisync-hub/benches/upload_ack_bench.rs` 或 `tests/m5_wp01_bench.rs`
  - `docs/reports/bench/M5-WP01-upload-ack.md`
- **DoD (验收契约)**:
  - 确认在 loopback 网络下，ACK 帧通信引入的吞吐量衰减 < 5%，P99 单帧等待时延 < 15ms。

---

## 2.5 WP02 自动化任务卡分解 (Execution Contract)

WP02 总体目标：最小可用多 Hub 联邦——空间归属经协商产生、经反熵同步收敛、
路由视图经各 hub 注册表 raft 组复制（规格见 [specs/M5-WP02.md](specs/M5-WP02.md)，
已批准）。

> **✅ 已收官（7/7，2026-09-24）**：T01 `fa283b6` / T02 `5a262de` / T03
> `83e9dfd` / T04 `cf5a284` / T05 `daae767` / T06 `3ca0109` / T07 本报告
> 提交。基准与验收映射见
> [reports/bench/M5-WP02-federation.md](reports/bench/M5-WP02-federation.md)
> （RouteQuery P50 146µs / P99 298µs；批量合入后续卡触发条件已评估）。

#### [Task] M5-WP02-T01: 规格与任务卡落档
- **目标**: `docs/specs/M5-WP02.md`（裁定 1-9 + 线协议 + 验收标准）+ 本看板任务卡。
- **约束文件清单**: `docs/specs/M5-WP02.md`、`docs/M5-PLAN.md`
- **DoD**: 规格含可执行验收标准与合并/协商确定性语义；看板状态更新。

#### [Task] M5-WP02-T02: 路由视图 raft 化
- **目标**:
  1. `RegistryCmd::SetRoute`（upsert 单行）+ `r-route` keyspace（注册表组 0 业务节）。
  2. `RouteRow{space_id, hub_id, addr, epoch}` 与 `FederationView` 只读面（线性一致读）。
- **约束文件清单**: `crates/partisync-hub/src/registry.rs`、`crates/partisync-hub/src/lib.rs`、`crates/partisync-hub/tests/m5_wp02.rs`
- **DoD**:
  - `set_route` 经组 0 日志落盘；crash 后重开不丢；读经 `ensure_linearizable`。
  - 合并规则单元测试：epoch 大者胜、同 epoch hub_id 小者胜、相等 no-op。
  - `cargo test -p partisync-hub --test m5_wp02` 全绿；wp03 既有套件原样通过。

#### [Task] M5-WP02-T03: 联邦线协议与客户端
- **目标**: `FedFrame` 信封（Hello/HelloAck/RouteQuery/RouteAnswer/RouteClaim/ClaimVerdict）+ `FedClient` 长连接（懒建、串行复用、错误重置），复用 `net.rs` 帧原语。
- **约束文件清单**: `crates/partisync-hub/src/federation.rs`（新建）、`crates/partisync-hub/src/lib.rs`、`crates/partisync-hub/tests/m5_wp02.rs`
- **DoD**: loopback 双 hub 客户端往返测试全变体；变体不匹配连接重置；零新依赖。

#### [Task] M5-WP02-T04: 联邦服务端与反熵视图同步
- **目标**: serve 循环应答三请求；Hello/HelloAck 全量交换 + `merge` 确定性落盘（胜出行经本地 raft）；周期重放（默认 5s 可配）。
- **约束文件清单**: `crates/partisync-hub/src/federation.rs`、`crates/partisync-hub/tests/m5_wp02.rs`
- **DoD**: 双 hub 握手后两侧视图相等；重复交换幂等（视图不再变更）；断连重连一次握手收敛。

#### [Task] M5-WP02-T05: 路由协商（RouteClaim）
- **目标**: RouteQuery（只答本地视图，防环）；RouteClaim 定向单行合并（胜者接受并落 raft，败者回 winner 即时改写）；`resolve_space` 组合子（本地 → 逐 peer 查询 → Unknown）。
- **约束文件清单**: `crates/partisync-hub/src/federation.rs`、`crates/partisync-hub/tests/m5_wp02.rs`
- **DoD**: 并发 claim（同 epoch A/B）两端收敛同一 winner（hub_id 小者）；claim 败者不等周期握手即时收敛；全 miss → Unknown。

#### [Task] M5-WP02-T06: 设备面 redirect 门
- **目标**: `HubService::route_for(space_id)` → `RouteDecision::{Local, Redirect, Unknown}`（纯本地 raft 视图，不持网络；Unknown 升级由联邦层 `resolve_space` 完成）。
- **约束文件清单**: `crates/partisync-hub/src/service.rs`、`crates/partisync-hub/tests/m5_wp02.rs`
- **DoD**: 三分支语义测试；本 hub 空间操作不受影响（回归 wp03）。

#### [Task] M5-WP02-T07: 集成矩阵、基准与收官
- **目标**: 双 hub loopback 全链（claim→sync→query→redirect）+ 分区恢复 + 幂等重放矩阵；RouteQuery P50/P99 与 10⁴ 行全量握手吞吐实测；报告落档 + 看板收官。
- **约束文件清单**: `crates/partisync-hub/tests/m5_wp02.rs`、`docs/reports/bench/M5-WP02-federation.md`、`docs/M5-PLAN.md`
- **DoD**: 验收标准（规格 §验收）全项勾验；M3/M5 既有套件原样通过；报告含增量协议触发条件评估。

---

## 2.75 WP03 自动化任务卡分解 (Execution Contract)

WP03 总体目标：扫描调度内核——目录前缀即分片、worker 池自取队列（闲者
多劳）、分片账本持久化断点恢复（规格见 [specs/M5-WP03.md](specs/M5-WP03.md)，
已批准）。

> **✅ 已收官（6/6，2026-09-24）**：T01 `f3ae024` / T02 `42e0a7b` / T03
> `29df932` / T04 `f19a91c` / T05 `3eadfdc` / T06 本提交。基准与验收映射见
> [reports/bench/M5-WP03-scan-scheduler.md](reports/bench/M5-WP03-scan-scheduler.md)
> （延迟模型 4 workers 2.60×；本地 fs 单 worker 31k 项/s 饱和；断点恢复
> 矩阵 + CLI 续扫幂等实测；后续卡触发条件已评估）。

#### [Task] M5-WP03-T01: 规格与任务卡落档
- **目标**: `docs/specs/M5-WP03.md`（裁定 1-8 + 契约 + 验收标准）+ 本看板任务卡。
- **约束文件清单**: `docs/specs/M5-WP03.md`、`docs/M5-PLAN.md`
- **DoD**: 规格含可执行验收标准；扫描调度位置/分片粒度/失败语义裁定明确。

#### [Task] M5-WP03-T02: 分片模型与清单源
- **目标**: `ListedNode`/`ListSource`/`EntrySink`/`ShardStatus`/`ScanStats`
  （scan.rs 新建）；`impl ListSource for Provider`（sync 侧，孤儿规则合规）；
  sync Cargo.toml + partisync-provider 依赖。
- **约束文件清单**: `crates/partisync-sync/src/scan.rs`、`crates/partisync-sync/src/lib.rs`、`crates/partisync-sync/Cargo.toml`、`crates/partisync-sync/tests/m5_wp03.rs`
- **DoD**: fs tmpdir 真实树走通 Provider→ListSource（文件/子目录混合、
  空目录、根前缀）；零新外部依赖。

#### [Task] M5-WP03-T03: 调度器核心
- **目标**: `ScanScheduler`/`ScanOpts`——固定 worker 池、BFS 分片队列自取、
  文件批（512）交 sink、原子统计 + 进度回调、失败语义（源失败重试 1 次→
  failed；sink 失败 fail-fast）。
- **约束文件清单**: `crates/partisync-sync/src/scan.rs`、`crates/partisync-sync/tests/m5_wp03.rs`
- **DoD**: fake source 下 concurrency=4 加速比 ≥2.5× 且条目集与串行基线
  一致；失败隔离（failed 不阻塞）与 fail-fast 语义测试全绿。

#### [Task] M5-WP03-T04: 断点恢复
- **目标**: `ScanJournal`/`JournalState`/`FileJournal`（临时文件+原子
  rename）；failpoint 命名点（`scan.before_sink`/`scan.shard_done`）。
- **约束文件清单**: `crates/partisync-sync/src/scan.rs`、`crates/partisync-sync/tests/m5_wp03.rs`
- **DoD**: 注入中断后 resume——done 分片不重扫（source 计数断言）、failed
  重入队、最终条目集一致；账本半写（截断）load 容错不 panic。

#### [Task] M5-WP03-T05: CLI scan-plan 子命令
- **目标**: `partisync scan-plan --scheme fs --root <dir> [--concurrency N]
  [--journal <path>]`——dry-run 统计（分片/条目/吞吐/失败清单）+ 续扫幂等。
- **约束文件清单**: `crates/partisync-cli/src/main.rs`、`crates/partisync-sync/tests/m5_wp03.rs`
- **DoD**: fs 目标 dry-run 输出统计；同 journal 二轮执行新增条目为 0；
  用法文本更新。

#### [Task] M5-WP03-T06: 基准与收官
- **目标**: 1/2/4/8 worker 加速曲线 + 恢复正确性 + 吞吐实测；报告落档
  `docs/reports/bench/M5-WP03-scan-scheduler.md`；看板收官。
- **约束文件清单**: `crates/partisync-sync/tests/m5_wp03.rs`、`docs/reports/bench/M5-WP03-scan-scheduler.md`、`docs/M5-PLAN.md`
- **DoD**: 验收标准（规格 §验收）全项勾验；全仓回归绿；报告含巨型扁平
  目录与 range 分片后续卡触发条件评估。

---

## 2.875 WP04 自动化任务卡分解 (Execution Contract)

WP04 总体目标：云事件流摄取——统一 EventSource 抽象、Webhook/SQS/Kafka
适配、journal 协调（折合 `EventKind::Created/Modified/Removed` + 断点续传
+ 空间路由闸）（规格见 [specs/M5-WP04.md](specs/M5-WP04.md)，已批准）。

> **✅ 已收官（5/5，2026-09-24）**：T01 `9b50a4d` / T02+T03 `3c096a7` /
> T04 `4196a9e` / T05 本提交。基准与验收映射见
> [reports/bench/M5-WP04-event-drain.md](reports/bench/M5-WP04-event-drain.md)
> （apply 323k evt/s、apply+journal 508k evt/s；v0.1 mock source + axum
> webhook 闭环；SQS/Kafka SDK 接线留 ADR 卡）。

#### [Task] M5-WP04-T01: 规格与任务卡落档
- **目标**: `docs/specs/M5-WP04.md`（裁定 1-8 + 契约 + 验收标准）+ 本看板任务卡。
- **约束文件清单**: `docs/specs/M5-WP04.md`、`docs/M5-PLAN.md`
- **DoD**: 规格含可执行验收标准；EventSource/EventJournal/适配器职责裁定明确。

#### [Task] M5-WP04-T02: EventSource 抽象 + WebhookAdapter（axum）+ SQS/Kafka stub
- **目标**: `EventRecord`/`EventSource`/`EventError`（sync/event.rs）+ Webhook 适配（axum POST + HMAC）+ SQS/Kafka stub impl（满足验收 1/2/3/5/7）；SDK 接线归后续 ADR。
- **约束文件清单**: `crates/partisync-sync/src/event.rs`、`crates/partisync-sync/src/lib.rs`、`crates/partisync-sync/tests/m5_wp04.rs`
- **DoD**: 折叠映射单元测试 + 同 cursor 重投递去重幂等测试全绿。

#### [Task] M5-WP04-T03: EventJournal + 断点续传 + 空间路由闸
- **目标**: `EventJournal`/`EventJournalState`（FileJournal 复用）+ 断点续传矩阵（failpoint 注入中断）+ 空间路由闸（非本 hub 持有不落 journal）。
- **约束文件清单**: `crates/partisync-sync/src/event.rs`、`crates/partisync-sync/tests/m5_wp04.rs`
- **DoD**: drain mock source 在 poll N 批次后中断→新 drain 续传；失败 source 计数一致；空间路由闸语义测试全绿。

#### [Task] M5-WP04-T04: CLI event-drain 子命令
- **目标**: `partisync event-drain --source <webhook|mock> ... [--journal <path>] [--cursor <token>]`——常驻拉取 + Ctrl-C 桥接 + 同 journal 二轮续跑幂等。
- **约束文件清单**: `crates/partisync-cli/src/main.rs`、`crates/partisync-sync/tests/m5_wp04.rs`
- **DoD**: fs mock + axum 集成；同 journal 二轮新增 0。

#### [Task] M5-WP04-T05: 基准与收官
- **目标**: 1k 事件/poll 吞吐 + webhook receiver 并发 POST 落盘时延；验收报告 + 看板收官。
- **约束文件清单**: `crates/partisync-sync/tests/m5_wp04.rs`、`docs/reports/bench/M5-WP04-event-drain.md`、`docs/M5-PLAN.md`
- **DoD**: 验收标准全项勾验；全仓回归绿。

---

## 2.9 WP05 自动化任务卡分解 (Execution Contract)

WP05 总体目标：D5 清偿——10⁶ 文档 BM25/向量/hybrid 检索吞吐实测
（P99 <100ms 验收线）+ 10⁶ 元数据仿真（对照 M3-WP01 底稿）+ 外推表；
D6/D7 gated 诚实划出（规格见 [specs/M5-WP05.md](specs/M5-WP05.md)，已批准）。

> **✅ 已收官（3/3，2026-09-25）**：T01 `23c10c7` / T02 `f2530e4`（含
> usearch 写路径超线性缺陷发现与修复）/ T03 本提交。报告
> [reports/bench/m5-wp05-scale.md](reports/bench/m5-wp05-scale.md)
> （BM25@10⁶ 636µs 达标；向量 16ms/条写入地板如实登记 + 后续卡）；
> M4-report §5 D5 已清偿、D6/D7 维持 gated。

#### [Task] M5-WP05-T01: 规格与任务卡落档
- **目标**: `docs/specs/M5-WP05.md`（裁定 1-6 + 验收线）+ 本看板任务卡。
- **约束文件清单**: `docs/specs/M5-WP05.md`、`docs/M5-PLAN.md`
- **DoD**: 验收线明确；D6/D7 gated 处置裁定。

#### [Task] M5-WP05-T02: D5 检索吞吐基准
- **目标**: `benches/wp05.rs`——10⁶ 合成语料（确定性生成，落盘复用）+
  BM25/向量/hybrid 三表 criterion 实测；CI 只跑 10⁴ 冒烟（内存上限规避）。
- **约束文件清单**: `crates/partisync-index/benches/wp05.rs`、`crates/partisync-index/Cargo.toml`、`crates/partisync-index/tests/m5_wp05.rs`
- **DoD**: 三表 P50/P99 登记；验收线（BM25/向量 <100ms、hybrid <150ms）达标或如实登记不达标。

#### [Task] M5-WP05-T03: 元数据仿真与收官
- **目标**: 10⁶ entry upsert 吞吐 + 查询 P99 + RSS 峰值实测；报告
  `docs/reports/bench/m5-wp05-scale.md`（含 10⁹–10¹² 外推段）；M4-report
  §5 D5 行更新已清偿、D6/D7 标注 gated；看板收官。
- **约束文件清单**: `crates/partisync-index/tests/m5_wp05.rs`、`docs/reports/bench/m5-wp05-scale.md`、`docs/reports/M4-report.md`、`docs/M5-PLAN.md`
- **DoD**: 验收标准全项勾验；全仓回归绿。

---

## 2.95 WP06 自动化任务卡分解 (Execution Contract)

WP06 总体目标：夜间混沌套件——聚合 M5 各 WP 恢复语义测试 + 补缺场景
（并发 claim race / 崩溃叠加 / 多层故障 / 环形分区编排）+ nightly 调度
入口（规格见 [specs/M5-WP06.md](specs/M5-WP06.md)，已批准）。

#### [Task] M5-WP06-T01: 规格与任务卡落档
- **目标**: `docs/specs/M5-WP06.md`（裁定 1-6）+ 本看板任务卡。
- **约束文件清单**: `docs/specs/M5-WP06.md`、`docs/M5-PLAN.md`
- **DoD**: 聚合/补缺边界明确；nightly 不触碰 ci.yml 门禁（铁律红线）。

#### [Task] M5-WP06-T02: hub 混沌补缺
- **目标**: 联邦并发 claim race（20 轮真实并发，收敛唯一 winner）+ leader crash 期间联邦查询照答陈旧视图。
- **约束文件清单**: `crates/partisync-hub/tests/m5_wp06.rs`
- **DoD**: 20 轮全收敛；crash 期间查询正常应答、重启后反熵收敛。

#### [Task] M5-WP06-T03: sync 混沌补缺
- **目标**: scan + event failpoint 并发启用（多层故障叠加）+ chaos.rs 环形分区（A↔B 断、B↔C 通、C↔A 断）逐对恢复收敛。
- **约束文件清单**: `crates/partisync-sync/tests/m5_wp06.rs`
- **DoD**: 双 failpoint 独立恢复成立；环形分区全网收敛无数据丢失。

#### [Task] M5-WP06-T04: nightly workflow 与收官
- **目标**: `.github/workflows/nightly.yml`（cron + workflow_dispatch，全量测试 + ignored 冒烟）；报告 `docs/reports/M5-WP06-chaos.md`（场景矩阵）；看板收官。
- **约束文件清单**: `.github/workflows/nightly.yml`、`docs/reports/M5-WP06-chaos.md`、`docs/M5-PLAN.md`
- **DoD**: 验收标准全项勾验；全仓回归绿。

---

## 3. 自动化任务推进执行铁律
1. **一任务一提交**：每次任务严格按照 `[M5-WP01-T0x]` 格式单独生成 Conventional Commit。
2. **测试门禁保障**：每次 commit 前必须完成本地 `cargo test` 及关键 targets 验证。
3. **禁止代码越界**：任务执行过程不得改动与当前任务卡清单无关的文件。
