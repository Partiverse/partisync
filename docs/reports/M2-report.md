# M2 里程碑报告

生成：`cargo xtask report M2`（骨架自动统计 + 人工填写各节）· 日期：2026-09-19

## 1. 范围与结果（对照 SPEC 汇总；范围变更记录）

M2 兑现产品核心主张「多设备融合」：9 个工作包全部交付，与 SPEC M2-WP00 工作包图一致，无范围裁剪。

| WP | 交付 | 核心验收 |
|---|---|---|
| WP01 同步核 | 域分离 oplog（设备自有单写者 / 共享域 HLC LWW）+ ACK 裁剪 + 回环防护 | P6 收敛性 proptest（2/3 节点随机操作全 mesh 收敛）；单写者纪律；幂等重放（P8） |
| WP02 双向同步 | bisync 不动点循环 + 冲突「保留两者 + 血缘」（P11）+ `--max-delete` 安全阈 | 冲突后缀永不覆盖更早副本；tag 共享域 LWW 收敛 proptest；墓碑抗复活 |
| WP03 Merkle 对账 | 4/8/12-bit 分级桶树 + 水位快路径（`Seen_a(d)==Seen_b(d)` 健全性条件）+ 时钟持久化 | P7 proptest 随机漂移收敛；快路径仅在全 origin 覆盖相等时生效（无假阴性）；迟到者全量对账恢复 |
| WP04 设备网络 | BIP-39 12 词配对码 + X25519 ECDH + Ed25519 设备身份 + 配对会话状态机 | 双端独立 ECDH 等值；TTL 过期/错码/乱码拒绝；**PFS：ephemeral_sk 不落盘**（审计整改，schema v13） |
| WP05 块级传输 | 泛化 delta 协议：ChunkPlan（Need/Have/Extra）+ 降级整文件回退 + executor 路由 | 空块根降级；块清单差集分类正确 |
| WP06 占位符 | `EntryState::Placeholder` 骨架同步 + 按需 `hydrate_entry` + pin/unpin | 占位经 oplog 传播（state 标记）；path 冲突幂等；pin 幂等 |
| WP07 E2EE | 空间密钥层次（Argon2id master → space → content/meta）+ XChaCha20-Poly1305 | KDF 全链确定性 + 域分离；AEAD 篡改拒绝；**审计整改后 KDF = blake3 官方 derive_key** |
| WP08 版本回收 | staggered 滚动窗 K=5 + trash-can TTL + GC + resurrect | pin 阻断回收；版本数上界 proptest；过期 GC；复活拒绝路径冲突 |
| WP09 混沌测试床 | ChaosSim（分区/时钟偏斜/故障注入 failpoint）+ 多节点故障矩阵 | 三节点非对称分区中继收敛；混合故障下 P6 不变量保持；oplog 截断不腐坏状态 |

范围变更记录：无裁剪。WP04/WP07 的 schema（v11/v12）在审计整改中演进为 v13（PFS）/v14（kdf_salt），见 §4。

## 2. KPI 达标表（基准报告链接）

对照 SPEC M2-WP00 §关门 KPI：

| KPI | 口径 | 结果 | 证据 |
|---|---|---|---|
| 三端空间同步 + 断网 24h 重连收敛 | 混沌床时间快进（24h 等价为时钟累计推进）自动验证 | ✅ | `wp09.rs`：`partition_then_recovery_converges` / `three_node_asymmetric_partition_converges_via_relay` / `mixed_partition_and_clock_skew_keeps_p6_invariant` |
| P6/P7/P8 全绿 | proptest 属性测试 | ✅ | `convergence.rs`（P6，5 测试含 3 节点 proptest）、`wp03.rs`（P7）、`replay_is_idempotent`（P8） |
| 外部密码学审计通过 | 独立安全公司审计报告 | ⚠️ **临时放行** | 见 §4：友邻审计 SEC-AUDIT-2026-M2-001 Conditional Pass + P1/P2 全部整改闭环；外部审计由 @lead 决定**临时放行、后续补上**（2026-09-19），登记为关门条件债务（§5） |
| 端到端基准：10⁵ 文件差异同步收敛 <5min（LAN） | `cargo test -p partisync-sync --test m2_kpi -- --ignored`（差异 = 1% 变更集，bisync 增量同步计时） | ✅ **14.1s（余量 ≈21×）** | `docs/reports/bench/m2-kpi.md`（rounds=2 不动点、pushed 精确=变更集零放大、oplog 双侧清空） |

## 3. 测试证据（覆盖率/属性测试/变异分数/模糊时长/混沌/互操作）

- **全仓测试**：146 通过 / 0 失败（`cargo test --workspace`，2026-09-19 本地全绿；CI 同门禁）。
- **属性测试**：P6 三节点收敛、P7 随机漂移、P11 冲突血缘、占位符不变量、staggered 上界——proptest 16 cases 默认档，回归文件（*.proptest-regressions）随库提交。
- **混沌**：ChaosSim 故障矩阵 7 项全绿（分区/时钟偏斜/崩溃窗口/高速 push/混合故障）。
- **变异测试**：M0 期建立 mutant 基线（docs/reports/bench/mutants-m0-wp00.md）；M2 新增行为契约均以验收测试先行承载（测试即规格铁律）。
- **互操作**：M1 已建立 rclone S3/WebDAV 双协议认证基线（M1-WP02-interop.md），M2 未改动协议面。
- **KPI 基准**：`crates/partisync-sync/tests/m2_kpi.rs`（#[ignore]，环境变量可调规模），结果存 docs/reports/bench/m2-kpi.md。

## 4. 安全（cargo audit / deny / unsafe 增量 / 外部审计）

- **cargo audit**：0 漏洞（1251 advisories，390 依赖，2026-09-19）。
- **unsafe**：workspace `unsafe_code = "forbid"` 全程未放宽，M2 零 unsafe 增量。
- **友邻密码学审计**（内部尽调，不替代外部审计）：SEC-AUDIT-2026-M2-001
  （docs/reviews/M2-cryptography-audit-report.md）结论 **Conditional Pass**——
  P0=0 / P1=2 / P2=3；**全部 P1/P2 已整改闭环**（清单 §5.1 整改记录，提交
  `4d60e29`/`2b9a805`）：KDF 切换 blake3 官方 derive_key、ephemeral_sk 移出
  持久层（PFS）、随机持久盐原语、密钥输出 Zeroizing 全覆盖、rustdoc 审计追踪标注。
- **外部密码学审计（G3 硬门禁）**：**@lead 决定临时放行（2026-09-19），后续补上**。
  放行依据：友邻审计已达成「关门前自查排雷」目的，P1 隐患全部消除，外部审计进场面
  显著收窄（聚焦 iroh 配对握手、XChaCha20 封包边界、内存清零）。补做义务已登记（§5）。
- **KDF 冻结声明**：整改改变派生输出（预发布无存量密文，零成本窗口）；G3 外部审计
  进场前冻结 KDF 层，不再接受变更。

## 5. ADR 清单与债务登记

ADR：0008（iroh 设备网络）· 0009（iroh-blobs 块传输）· 0010（E2EE 加密栈，含修订 1：
KDF 标准化 / 盐策略收紧 / 内存封装）——均已接受。

债务登记（按优先级）：

| # | 债务 | 偿还窗口 |
|---|---|---|
| D1 | **外部密码学审计补做**（G3 硬门禁唯一未清项） | M3 早期，外部审计公司委托 |
| D2 | `kdf_salt` 全量接线：空间初始化流程自动生成/持久化随机盐（原语与存储已就绪并有验收测试） | M3 空间供给流程 |
| D3 | iroh 真实网络层（当前配对/同步为同进程双 Store 模拟，接口已抽象） | M3 hub 集群 |
| D4 | 混沌床容器化三节点（当前 ChaosSim 进程内模拟） | M3 |

## 6. AI 使用披露（自动统计）
- 挂接 M2-* 任务的提交数：37
- 任务数：34
- 工作包分布：M2-WP00–WP09 全覆盖
- AI 辅助提交（AI-Assist）：37/37
- 人工终审提交（Reviewed-By）：0/37（M2 人工终审随 G3 外部审计补签）

（明细清单见 `cargo xtask report M2` 自动生成节，随本报告同步刷新。）

## 7. 抽查审计记录（随机 5 任务，仅凭工件重建故事）

抽查口径沿用 M-1 审计演练（docs/reports/M-1-audit-rehearsal.md）：任选
`cargo xtask trace` 抽 5 个 M2 任务凭 commit+spec+tests 重建实现故事：

- M2-WP01-T02（同步核）→ P6 测试先行，session/reconcile 实现与 SPEC §3/§4 一致；
- M2-WP03-T04（快路径）→ `fast_path_eligible` 健全性条件与 SPEC §2 逐条对应；
- M2-WP04-T02（配对闭环）→ wp04.rs 验收矩阵覆盖 SPEC §2 状态机全分支；
- M2-WP07-T04（KDF 整改）→ commit + ADR-0010 修订 1 + 清单 §5.1 三方互证；
- M2-WP09-T04（故障矩阵）→ wp09.rs 7 项测试与 SPEC §3 故障矩阵逐行对应。

全部任务可凭工件独立重建，无断链。

## 8. 下一阶段建议

1. **外部密码学审计补做（D1）**——M2 关门唯一悬置硬门禁，建议 M3 开局即委托；
2. M3 主题：hub 集群（中继/多对端 ACK 集合）、EC/分层存储、空间供给流程（含 D2 盐接线）；
3. 同步核经验沉淀：捕获侧 origin 必须取本机 device（KPI 基准暴露的乒乓放大根因）——
   已修复并固化为测试口径，M3 hub 多对端场景需延续该不变量。

## 放行签字（G3）

- [ ] 架构负责人：
- [ ] 评审人：
- [x] 安全负责人（M2/M4）：**外部审计临时放行**（@lead，2026-09-19——补做义务登记 §5/D1；友邻审计 SEC-AUDIT-2026-M2-001 已闭环）
