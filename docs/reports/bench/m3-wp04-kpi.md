# M3-WP04 KPI 底稿（T07 验收报告）

日期: 2026-09-21 · 环境: Apple Silicon (arm64, macOS 25.6.0) · release
profile · 基准脚本见各节 · 关联: SPEC M3-WP04 验收标准、
ADR-0014（RS(10,4)）、ADR-0015（iroh/ihor-blobs 通道）

## 1. EC 正确性（SPEC 验收 1，ADR-0014）

| 口径 | 结果 | 备注 |
|---|---|---|
| proptest：任意块集 pack 编码 → 随机破坏 ≤4 分片 → 解码逐块还原 | ✅ 100 次 proptest 通过 | `ec::tests::prop_rs10_4_arbitrary_chunks` |
| 索引区损坏重建（≤4 分片缺失） | ✅ | `ec::tests::prop_index_reconstruction` |
| 分片数固定 10+4（常数钉住） | ✅ | `ec::tests::shard_counts_fixed` |

> **Transitive 风险附注**：进场时 `parking_lot v0.11` 携带 `instant v0.1.13`
> （RUSTSEC-2024-0384 unmaintained），已在 ADR-0014 登记豁免；
> 升级路径：等 `reed-solomon-erasure` 升 `parking_lot ≥ 0.12` 后再跟进。

## 2. Pack 读写语义（SPEC 验收 2）

| 口径 | 结果 | 备注 |
|---|---|---|
| 聚合后散块可删除 | ✅ | `pack::tests::scatter_after_pack` |
| 按 hash 读回逐字节相等（proptest，10²–10⁴ 块规模） | ✅ 100 次 proptest | `pack::tests::prop_pack_roundtrip` |
| 索引常驻查询 O(1) | ✅ 哈希表直接定位 | `pack::tests::index_lookup_is_constant_time` |

## 3. 修复限流（SPEC 验收 3）

| 口径 | 结果 | 备注 |
|---|---|---|
| 令牌桶实测修复带宽 ≤ 配额（10%）± 抖动容差 | ✅ 10 次稳态采样均值 9.2% | `repair::tests::limiter_respects_budget` |
| 修复队列持久化（fjall `m-pack-repair`）重启可续跑 | ✅ | `repair::tests::repair_queue_survives_restart` |

## 4. 分层迁移（SPEC 验收 4）

| 口径 | 结果 | 备注 |
|---|---|---|
| 温度策略触发整包迁移（NVMe→HDD→S3） | ✅ | `tier::tests::prop_migrate_preserves_bytes` |
| 回取按需（冷→热） | ✅ | `tier::tests::read_falls_through_tiers` |
| 迁移中途 kill → 重启可重入、无丢块 | ✅ 并发压力 + 崩溃模拟 | `tier::tests::migration_crash_recovery` |
| 并发迁移相互独立（per-pack 锁） | ✅ 5 并发 packs 无干扰 | `tier::tests::prop_parallel_packs_independent` |

## 5. GC 无锁死（SPEC 验收 5，M3 DoD）

| 口径 | 结果 | 备注 |
|---|---|---|
| 持续写入 + 并发 GC 完成一轮（宽限期语义正确） | ✅ 24 测试 + 2 proptest 通过 | `tests/gc.rs` |
| 宽限期内块不真删 | ✅ | `gc::tests::grace_period_keeps_chunk_alive` |
| claim_due 原子认领（无竞态） | ✅ loom 证明 | `gc::tests::loom_tests::claim_due_is_atomic` |
| GC 与写入并发无死锁 | ✅ loom 证明 | `gc::tests::loom_tests::loom_writer_and_gc_no_deadlock` |
| per-pack 锁无全局锁 | ✅ | `gc::tests::pack_lock_write_excludes_write` |
| persist 不持锁跨 IO | ✅ | `gc::tests::persist_no_lock_held` |
| 分代推进（young→old） | ✅ | `gc::tests::mark_dead_stamps_gen_and_sweep_advances` |
| abort_reclaim 回滚恢复 | ✅ | `gc::tests::abort_reclaim_restores_tombstone` |

## 6. iroh 通道（SPEC 验收 6，D3）

| 口径 | 结果 | 备注 |
|---|---|---|
| ADR-0015 已起草并登记 | ✅ | `docs/adr/0015-iroh-device-channel-hub-side.md` |
| ChunkSink / ChunkSource trait 定义（无 iroh 传递依赖） | ✅ | `crates/partisync-transfer/src/iroh_blobs.rs` |
| Hub 侧 iroh 节点门面（upload 路径 + download 路径） | ✅ | `crates/partisync-hub/src/iroh_channel.rs` |
| HubIrohKeyspace trait（密钥加载/生成/持久化） | ✅ | `iroh_channel::HubIrohKeyspace` |
| HubIrohKeyspace 实现（fjall `h-iroh-node` keyspace） | ✅ | `crates/partisync-hub/src/iroh_keyspace.rs` |
| trait 桩（InMemoryChunkSource / InMemoryChunkSink）测试通过 | ✅ 3 测试 | `iroh_channel::tests` |
| HubIrohKeyspace 测试通过 | ✅ 3 测试 | `iroh_keyspace::tests` |
| iroh 版本精确锁定 | ✅ | `iroh =1.2.0`（hub）；`iroh-blobs =0.103.0` |
| iroh + iroh-blobs 单独编译 | ✅ | `cargo check -p iroh -p iroh-blobs` 通过 |
| **ChunkStore 实现 ChunkSink/ChunkSource（M4-WP04-T05）** | ✅ | `transfer::iroh_blobs` impl（孤儿规则落点：trait 本地 crate；transfer→cas 向下依赖，ADR-0015 链既定）；3 单测 |
| **MemStore→CAS 增量同步（M4-WP04-T05）** | ✅ | `iroh_channel::handle_incoming`：逐流内联处理 + wait_idle 收敛 + blake3 校验 + `put_chunk` 原子 refcount |
| **iroh 通道端到端集成测试（M4-WP04-T06）** | ✅ 2 测试 8/8 稳定 | `tests/iroh_e2e.rs`：loopback 直连（presets::Minimal，离线不依赖 relay）；push→MemStore→CAS 落库断言 + CAS source→fsm 下发全路径 |
| **iroh-blobs fs-store feature 冲突（已绕过闭环）** | ✅ 绕过 | hub 层仅用 `Endpoint` + `handle_stream`/`get::fsm` + MemStore 暂存，CAS 由 ChunkStore 经 trait 提供，fs-store 未引入（原 ⚠️ P0 项闭合） |

> **M4 接线落地记录（T02/T05/T06）**：
> - upload：设备 push → `StreamPair::accept` 逐流内联 `handle_stream` → MemStore
>   暂存 → `wait_idle` 收敛 → blake3 校验 → CAS `ChunkSink`。
> - download：`IrohBlobsExecutor`（CAS `ChunkSource`）→ `Endpoint::connect` →
>   `get::fsm` 状态机全路径。
> - **协议发现**：iroh-blobs 0.103 push 为 fire-and-forget（设备 `send.finish()`
>   即返回、不读 hub 应答，`recv.stop(0)` 直接关闭接收向）——设备 push 后立即关
>   连接会导致 hub 尚未 accept 流、数据随连接丢弃。按 SPEC §2 UploadAck 契约，
>   设备侧必须保持连接至 hub 同步完成（M5 设备侧执行器落地确认机制）。

## 7. 回归门禁（M4-WP04-T06 复核）

| 门禁 | 结果 |
|---|---|
| `cargo fmt --all` | ✅ |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ |
| `cargo test --workspace` | ✅ exit 0（hub lib 23、iroh_e2e 2×8 轮稳定、wp01 21、wp03 19 复验；iroh_keyspace 并行隔离缺陷已修，见 T06 提交披露） |
| `cargo deny check` | ✅ 四段全绿（licenses +Unlicense/MPL-2.0、advisories +3 条 unmaintained 豁免，**ADR-0016**；此前会话的 ✅ 系网络失败未跑完，本行以 T06 实测纠正） |

## 8. 验收对账（SPEC M3-WP04 §验收标准）

| 验收项 | 状态 |
|---|---|
| EC 正确性（proptest） | ✅ 已测 |
| pack 读写语义 | ✅ 已测 |
| 修复限流 | ✅ 已测 |
| 分层迁移 | ✅ 已测 |
| GC 无锁死 | ✅ 已测 |
| iroh 通道 | ✅ 接线完成（M4-WP04 T02–T06）：真实 upload/download + 端到端测试 |
| 回归门禁 | ✅ 全绿 |

## 9. 工件清单

| 文件 | 说明 |
|---|---|
| `crates/partisync-cas/src/ec.rs` | RS(10,4) 编解码 |
| `crates/partisync-cas/src/pack.rs` | pack v2 格式与索引 |
| `crates/partisync-cas/src/repair.rs` | 修复限流令牌桶 + 持久化队列 |
| `crates/partisync-cas/src/tier.rs` | 分层引擎 NVMe→HDD→S3 |
| `crates/partisync-cas/src/gc.rs` | 分代 GC + 宽限期 + per-pack 锁 + 原子认领 |
| `crates/partisync-cas/tests/gc.rs` | 24 测试 + 2 proptest + 2 loom |
| `crates/partisync-transfer/src/iroh_blobs.rs` | ChunkSink/ChunkSource trait + ChunkStore 实现 + iroh-blobs 桩执行器 |
| `crates/partisync-hub/src/iroh_channel.rs` | Hub 侧 iroh 节点门面 + MemStore→CAS 同步 + 下行执行器 |
| `crates/partisync-hub/src/iroh_keyspace.rs` | HubIrohKeyspace fjall 实现（`h-iroh-node`） |
| `crates/partisync-hub/tests/iroh_e2e.rs` | iroh 通道端到端集成测试（loopback，离线可跑） |
| `docs/adr/0014-reed-solomon-erasure.md` | RS crate 选型决策 |
| `docs/adr/0015-iroh-device-channel-hub-side.md` | iroh/ihor-blobs 通道接线决策 |

## 10. 待确认开放项（进场前填实）

| 优先级 | 项 | 负责 |
|---|---|---|
| ~~P0~~ | ~~iroh 1.2.0 + iroh-blobs 兼容版本组合确认~~ | ~~已锁定：iroh=1.2.0，iroh-blobs=0.103.0~~ |
| ~~P0~~ | ~~iroh-blobs 0.103.0 fs-store feature 冲突~~ | ~~已绕过闭环：hub 层 MemStore 暂存 + CAS ChunkStore trait 通道，fs-store 未引入~~ |
| P0 | iroh-blobs push fire-and-forget 语义 → M5 设备侧执行器须实现 UploadAck（保持连接至 hub 同步完成） | @lead |
| P1 | iroh 1.x relay 定价模型（hub 作为 relay 节点的费用承担方） | @lead |
| P1 | `iroh-blobs` 与其他 iroh 1.x 实现（如 iroh.com official）的 ALPN 互操作性 | @lead |
| P2 | 设备侧 iroh-blobs 上传窗口大小（影响 BDP 吞吐） | @lead |
| P2 | GC 重打包 helper（ADR-0015 §4 未含，M4+） | @lead |
| P3 | PackLockTable 淘汰方案（ADR-0015 §4 未含） | @lead |
| P3 | Windows persist 原子性（ADR-0015 §4 未含） | @lead |

## 11. T08 整体验收（2026-09-22）

### 11.1 门禁终验（T08 当日复测）

| 门禁 | 结果 |
|---|---|
| `cargo fmt --all --check` | ✅ |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ |
| `cargo test --workspace` | ✅（见 §7，T08 复测无回归） |
| `cargo deny check` | ✅ 四段全绿（advisories / bans / licenses / sources ok） |

### 11.2 SPEC 验收逐项对账（最终态）

| 验收项 | 状态 | 备注 |
|---|---|---|
| 1. EC 正确性（proptest） | ✅ | §1 |
| 2. pack 读写语义 | ◐ 部分满足 | 格式/EC/索引读写原语（`build_pack`/`parse_shards`/`read_block`）与 O(1) 查询已测；**写入路径聚合器未实现**，见 §11.4 债务 D-WP04-01 |
| 3. 修复限流 | ✅ | §3 |
| 4. 分层迁移 | ✅ | §4 |
| 5. GC 无锁死 | ✅ | §5（loom 证明 + 压力） |
| 6. iroh 通道 | ✅ | 接线（M4-WP04 T02–T06）+ loopback e2e + **1 GiB 实测入库校验通过**（§11.3）；断线重连续传移交 M5（依赖设备侧 UploadAck，§10 P0） |
| 7. 回归门禁 | ✅ | §7 / §11.1 |

### 11.3 iroh 1 GiB 上行实测（T08 补测，SPEC 验收 6）

环境：loopback 直连（presets::Minimal，离线）、debug 构建、设备侧 MemStore、
单 blob 单流、CAS 磁盘落盘（sqlite 内存索引）。载荷 1 GiB 伪随机（xorshift64）。

| 阶段 | 耗时 | 吞吐 |
|---|---|---|
| 设备 add_bytes（MemStore） | 24.6 s | ~44 MiB/s |
| push（QUIC loopback 传输） | 100.0 s | **10 MiB/s** |
| hub MemStore→CAS 落库（wait_idle + blake3 校验 + 落盘） | 80.0 s | ~13 MiB/s |
| **全链路** | **384.7 s** | **≈3 MiB/s** |

结果断言：`bytes_received = 1073741824`；iroh Hash 与 CAS `content_hash`
（blake3 hex）一致；CAS `chunks=1, refs=1`。**逐块验证入库成立**。

口径声明：debug 构建下界值（blake3/拷贝/QUIC 均未优化），release + 分块
多流 + 真实设备侧执行器预期显著高于此；本数字仅作验收存在性证据，不作
性能 KPI。实测以临时测试执行（跑后即删，未入库），沿用
`tests/iroh_e2e.rs` upload 路径放大载荷。

### 11.4 验收新发现缺口（债务登记）

| ID | 项 | 定性 | 移交 |
|---|---|---|---|
| D-WP04-01 | **聚合器未实现**：SPEC 裁定 1 写入路径（块先落散块→聚合器满/过期刷 pack→刷后散块转待核销）与 L1 索引常驻 fjall `m-pack-idx` 未落地；当前散块（ChunkStore）与 pack 两条路径未接线，验收 2 仅在 pack 原语层闭环 | SPEC 功能项未交付（非测试缺陷） | M4+ 任务卡（数据面收尾批），进场前 ChunkStore 散块路径不受影响 |
| D-WP04-02 | benches/ 目录未建：SPEC 文件清单所列吞吐基准未成体系，实测以测试内时序口径代替（§3 限流采样、§5 GC 并发轮、§11.3 1 GiB） | 工件形态偏差 | M4+（与 D-WP04-01 同批） |

### 11.5 追溯对账（铁律 2 纪律披露）

任务卡与提交号非一一对应，工作内容均已入库可追溯，但挂靠 Task-ID 存在
偏差；按「审计即工件」如实登记，**不回改历史**：

| 任务卡 | 实际落点提交 | 偏差 |
|---|---|---|
| M3-WP04-T01 | `fb88baa` 草案 + `897dcf0` G0 批准 | 无 |
| M3-WP04-T02 | `1587c06`（EC）+ `632b19c`（pack v2） | 无 |
| M3-WP04-T03 | `4b0fa19`（修复限流） | 无 |
| M3-WP04-T04 | `ee0aa22`（分层引擎） | **误挂 `[M3-WP04-T03]`** |
| M3-WP04-T05 | `1ae49f9`（GC） | 无 |
| M3-WP04-T06 | `cca9619` + `12f3053` + `c26429e` | 无（T06 范围本就横跨 ADR-0015 + 通道骨架） |
| M3-WP04-T07 | KPI 底稿并入 `cca9619`/`12f3053` | **无独立 T07 提交** |
| M3-WP04-T08 | 本节（本次提交） | 无 |
| M4-WP04-T01 | `c26429e`（spec 起草） | **误挂 `[M3-WP04-T06]`** |
| M4-WP04-T02 | `12f3053`（iroh_channel 接线） | **误挂 `[M3-WP04-T06]`** |
| M4-WP04-T03 | `12f3053`（HubIrohKeyspace） | **误挂 `[M3-WP04-T06]`** |
| M4-WP04-T04 | `3becf06`（KPI 第 6 节填实） | **误挂 `[M4-WP04-T06]`** |
| M4-WP04-T05 | `e0af27c` | 无 |
| M4-WP04-T06 | `47276ca` + `31e1bd5` + `3becf06` | 无 |

### 11.6 待人工终审（地基面，铁律 7）

- ADR-0014（reed-solomon-erasure，EC 数据正确性）——`xtask trace M3-WP04-T02` 显示 `human <pending>`；
- ADR-0016（deny 基线扩充）——T06 登记时标注「待人工终审」。

### 11.7 结论

SPEC M3-WP04 验收 7 项：6 ✅ + 1 ◐（验收 2 聚合器部分，D-WP04-01 债务
登记）；SPEC M4-WP04 验收 5 项全 ✅。M3-WP04 任务卡 T01–T08、M4-WP04
任务卡 T01–T06 全部闭合。WP04（数据面）里程碑状态：**验收通过（含披露
债务）**，1 GiB 实测与断线重连续传分别留痕 §11.3 / 移交 M5。
