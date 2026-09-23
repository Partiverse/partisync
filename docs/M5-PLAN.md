# M5 里程碑全景看板与自动化执行计划 (M5-PLAN)

> 本文档是 M5 里程碑全生命周期的自动化任务调度中枢。
> 遵循 [AGENTS.md](../AGENTS.md) 铁律：规格先行、原子交付、测试契约完备、零无聊依赖。

---

## 1. M5 工作包总览与推进状态

| 工作包 | 主题 | 负责人/模式 | 核心目标 | 状态 |
|---|---|---|---|---|
| **WP01** | 设备侧 UploadAck 协议与可靠传输 | AI 自动执行 | 解决 iroh push fire-and-forget，保证 Hub CAS 落库前不丢数据 | **已完成 (6/6) ✅** |
| **WP02** | 联邦路由协议 (Multi-Hub Federation) | 规范先行 + AI | 基于 space 前缀的多 Hub 路由协商与 Raft 视图同步 | **已完成 (7/7) ✅** |
| **WP03** | 分布式扫描调度器 | AI 自动执行 | 前缀分片并行 LIST、动态负载均衡与扫描断点恢复 | 待排期 |
| **WP04** | 云事件流摄取引擎 | AI 自动执行 | SQS / Webhook / Kafka 增量事件流适配器与去重流水线 | 待排期 |
| **WP05** | 真实规模性能基准 (10⁶ - 10¹² 仿真) | AI 压测评估 | 替代合成评估，真实多模态/图谱元数据规模化时延与内存评估 | 待排期 |
| **WP06** | 夜间混沌测试套件 (Chaos Suite) | AI 自动执行 | 覆盖网络分区、断网重连、宕机恢复、并发竞争混沌测试 | 待排期 |

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

## 3. 自动化任务推进执行铁律
1. **一任务一提交**：每次任务严格按照 `[M5-WP01-T0x]` 格式单独生成 Conventional Commit。
2. **测试门禁保障**：每次 commit 前必须完成本地 `cargo test` 及关键 targets 验证。
3. **禁止代码越界**：任务执行过程不得改动与当前任务卡清单无关的文件。
