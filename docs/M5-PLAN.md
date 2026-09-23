# M5 里程碑全景看板与自动化执行计划 (M5-PLAN)

> 本文档是 M5 里程碑全生命周期的自动化任务调度中枢。
> 遵循 [AGENTS.md](../AGENTS.md) 铁律：规格先行、原子交付、测试契约完备、零无聊依赖。

---

## 1. M5 工作包总览与推进状态

| 工作包 | 主题 | 负责人/模式 | 核心目标 | 状态 |
|---|---|---|---|---|
| **WP01** | 设备侧 UploadAck 协议与可靠传输 | AI 自动执行 | 解决 iroh push fire-and-forget，保证 Hub CAS 落库前不丢数据 | **执行中 (3/6)** |
| **WP02** | 联邦路由协议 (Multi-Hub Federation) | 规范先行 + AI | 基于 space/content_id 前缀的多 Hub 路由协商与 Raft 视图同步 | 待排期 |
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

## 3. 自动化任务推进执行铁律
1. **一任务一提交**：每次任务严格按照 `[M5-WP01-T0x]` 格式单独生成 Conventional Commit。
2. **测试门禁保障**：每次 commit 前必须完成本地 `cargo test` 及关键 targets 验证。
3. **禁止代码越界**：任务执行过程不得改动与当前任务卡清单无关的文件。
