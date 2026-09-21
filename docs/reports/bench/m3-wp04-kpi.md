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
| Hub 侧 iroh 节点门面骨架 | ✅ | `crates/partisync-hub/src/iroh_channel.rs` |
| trait 桩（InMemoryChunkSource / InMemoryChunkSink）测试通过 | ✅ 3 测试 | `iroh_channel::tests` |
| iroh 版本精确锁定 | ✅ | `iroh =1.2.0`（hub）；`iroh-blobs =0.103.0`（待 M4 接入 store） |
| iroh 1.2.0 单独编译 | ✅ | `cargo check -p iroh` 通过 |
| **iroh-blobs fs-store feature 冲突（M4 进场地解决）** | ⚠️ | `iroh-blobs 0.103.0` fs-store 用 `tokio::task::block_in_place`（需 `rt-multi-thread`），workspace tokio 1.53 无 `blocking` feature；transfer 层不引 iroh-blobs（ADR-0015 裁定 1），hub 层仅用 iroh 主 crate |

> **M4 接线计划**：hub 层通过 `iroh::Endpoint` + `iroh_blobs::get::fsm` 状态机实现 download；
> upload 由 `Router::accept(ALPN, BlobsProtocol)` 后台处理；绕过 `iroh-blobs` fs-store 依赖。

## 7. 回归门禁

| 门禁 | 结果 |
|---|---|
| `cargo fmt --all` | ✅ |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ |
| `cargo test --workspace` | ✅ 全 workspace 通过 |
| `cargo deny check` | ✅（RUSTSEC-2024-0384 豁免已登记于 ADR-0014） |

## 8. 验收对账（SPEC M3-WP04 §验收标准）

| 验收项 | 状态 |
|---|---|
| EC 正确性（proptest） | ✅ 已测 |
| pack 读写语义 | ✅ 已测 |
| 修复限流 | ✅ 已测 |
| 分层迁移 | ✅ 已测 |
| GC 无锁死 | ✅ 已测 |
| iroh 通道 | ⚠️ 桩骨架完成；iroh 版本对齐待确认 |
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
| `crates/partisync-transfer/src/iroh_blobs.rs` | ChunkSink/ChunkSource trait + iroh-blobs 桩执行器 |
| `crates/partisync-hub/src/iroh_channel.rs` | Hub 侧 iroh 节点门面 + 桩实现 |
| `docs/adr/0014-reed-solomon-erasure.md` | RS crate 选型决策 |
| `docs/adr/0015-iroh-device-channel-hub-side.md` | iroh/ihor-blobs 通道接线决策 |

## 10. 待确认开放项（进场前填实）

| 优先级 | 项 | 负责 |
|---|---|---|
| ~~P0~~ | ~~iroh 1.2.0 + iroh-blobs 兼容版本组合确认~~ | ~~已锁定：iroh=1.2.0，iroh-blobs=0.103.0~~ |
| P0 | iroh-blobs 0.103.0 fs-store feature 冲突（workspace tokio 缺 blocking；M4 接入 store 时解决） | @lead |
| P1 | iroh 1.x relay 定价模型（hub 作为 relay 节点的费用承担方） | @lead |
| P1 | `iroh-blobs` 与其他 iroh 1.x 实现（如 iroh.com official）的 ALPN 互操作性 | @lead |
| P2 | 设备侧 iroh-blobs 上传窗口大小（影响 BDP 吞吐） | @lead |
| P2 | GC 重打包 helper（ADR-0015 §4 未含，M4+） | @lead |
| P3 | PackLockTable 淘汰方案（ADR-0015 §4 未含） | @lead |
| P3 | Windows persist 原子性（ADR-0015 §4 未含） | @lead |
