# M7-WP02-T01 复核报告：M2-D1 密码学面（AI 执行）

> **报告编号**: SEC-AI-AUDIT-M7-WP02-M2D1
> **复核对象**: PartiSync M2 密码学与 E2EE 栈对当前 HEAD 的持续有效性——
> iroh 配对握手 / XChaCha20 封包边界 / 内存清零三域 + M2 审计整改回归 + KDF 冻结验证
> **复核基线**: main = `3018ff9`（2026-09-30，M7-WP02 SPEC 批准同点）
> **上游**: SEC-AUDIT-2026-M2-001（2026-09-19 友邻审计，Conditional Pass）·
> ADR-0008/0010/0015 · SPEC M2-WP04/M2-WP07 · M7-WP00 §4 债务表
> **执行方式**: AI 执行（GLM-5.3-Flash，逐项对照原审计基准 + HEAD 测试实证）
> **复核结论**: **Conditional Pass（AI 执行，不构成外部审计）**
> **延续义务**: 外部审查未清偿，挂「资金回笼」事件（§6）

---

## 1. 结论速览

| 域 | 结论 |
|---|---|
| M2 五项整改（P1×2 + P2×3） | **全部在 HEAD 验证有效**（§4 逐项证据） |
| ① iroh 配对握手 | 基线维持，测试全绿；1 项设计对齐观察（F-2） |
| ② XChaCha20 封包边界 | 无新发现（24B CSPRNG nonce + Poly1305 + Fatal 语义不变） |
| ③ 内存清零 | Zeroizing 覆盖面完整；1 项边缘观察（F-3） |
| KDF 冻结 | **未回退**（§5） |
| 新发现 P0/P1 | **0 项**；P3 观察项 3 条（§6，全部为债登记，不阻塞） |

## 2. 复核方法与局限（R4 强制披露）

- **方法**：逐项对照 SEC-AUDIT-2026-M2-001 的 24 子项核查矩阵与整改建议，
  直接读 HEAD 实现（`crypto.rs` 197 行 / `pairing.rs` 240 行 / `store.rs`
  配对会话与盐列 / `hub/registry.rs` D2 接线），并以测试实证
  （§4 测试矩阵）。
- **局限 1（自审自证）**：HEAD 代码主体由 AI 会话产出，本次复核同为 AI
  执行——**不构成独立第三方审计**；缓解手段是「对照外部基准（RFC 9106 /
  BLAKE3 KDF spec / RFC 8439）逐项验证 + 测试断言实证」而非自由发挥的
  安全论证。
- **局限 2（范围）**：本报告只复核 M2-D1 登记面；M4-D2 渗透面（18 探针
  重跑 + 攻击面增量映射）归 T02 独立报告；M7 新增攻击面（桌面壳 IPC/CSP、
  ext-host 沙箱）引用 WP01 已完成的 [P13]/[P14] 全量探针与多模型对抗审查
  结论，不重复验证。

## 3. 三域复核

### ① iroh 配对握手（SPEC M2-WP04，ADR-0008）

- 配对码 128-bit 熵（12 词 BIP-39）/ 5 min TTL（`DEFAULT_TTL_NS`）不变；
  X25519 ECDH 双端等值测试绿（`pairing::tests::x25519_ecdh_matches_on_both_sides`）。
- 域分离维持：`device-key-v1` / `endpoint-secret-v1` 上下文互异且无跨用
  复用（M2 3.1.2 判 Pass，维持）。
- **PFS 维持**：会话行仅落 `ephemeral_pk` + code（`store.rs::create_pairing_session`
  列清单无私钥）；旧库 `ephemeral_sk NOT NULL` 列由 store 迁移重建移除
  （`store.rs:284` 注释：临时私钥曾落盘与 PFS 相悖，统一重建）。临时私钥
  以 `Zeroizing<[u8; 32]>` 驻留进程内存直至会话完成。
- **传输面现状**（M3-M5 演进后）：hub 层 iroh 通道走 iroh 原生 QUIC/TLS
  （节点自有密钥，ADR-0015；载荷 E2EE 在 CAS/加密层先行完成）；M2 设计的
  `endpoint_secret`（iroh 会话密钥材料）在 HEAD **无生产消费方**——配对
  协议与传输通道尚未合流（`pairing.rs` 头注「v1 简化：iroh 落地后由网络
  协议层投递」的接线项仍未发生）。登记 F-2。

### ② XChaCha20 封包边界（SPEC M2-WP07）

- `encrypt_content`：每次加密 24 字节 CSPRNG nonce（`getrandom::fill`，
  碰撞界限 ~2⁻⁹⁶）；Poly1305 16 字节认证标签随密文。
- `decrypt_content`：认证失败包为 `Severity::Fatal`，不泄露明文先验；
  篡改/错 nonce 拒绝测试绿（`decrypt_rejects_tampered_ciphertext` /
  `decrypt_rejects_wrong_nonce`）。
- 密钥层次：content_key / meta_key 域分离（`partisync content-key-v1` /
  `partisync meta-key-v1`）测试绿；round-trip 绿。

### ③ 内存清零（M2 3.2.4）

- 密钥输出全链 `Zeroizing` 包裹：`argon2_master_key{,_with_salt}` /
  `derive_space/content/meta_key` / `mnemonic_to_entropy` /
  `unified_secret_from_rng` / `shared_bytes` / `derive_device_key` /
  `endpoint_secret`（`pairing.rs::DerivedKeys` 类型别名强制）。
- x25519-dalek `StaticSecret` 自带 zeroize-on-drop（默认 feature）；
  中间盐 `salt.zeroize()` 显式调用维持。
- 边缘观察 F-3：`device_signing_key(seed)` 由裸 `&[u8; 32]` 构造 dalek
  `SigningKey`（dalek 内部缓冲不随 Drop 清零）——设备签名种子本身是持久
  身份材料（落库），增量暴露有限，登记不阻塞。

## 4. M2 整改回归（P1×2 + P2×3 逐项 HEAD 证据）

| 原发现 | 整改要求 | HEAD 证据 | 测试实证 |
|---|---|---|---|
| **P1-1** 自研 `hkdf_blake3` | 切 `blake3::derive_key` | `crypto.rs:51` `derive_blake3_key` = 官方 KDF mode + 域分离 context；自研两阶段构造全仓无残留 | `hkdf_deterministic` / `content_and_meta_keys_distinct` |
| **P1-2** `ephemeral_sk` 落盘 | 私钥仅驻内存 | `pairing.rs::initiate_pairing` 返回 `Zeroizing` 私钥不落库；schema 会话表无私钥列；旧列迁移重建（`store.rs:284`） | `wp04` 6/6（含会话状态机） |
| **P2-3** 盐确定性 | 随机持久盐 | `random_kdf_salt()`（CSPRNG）+ `argon2_master_key_with_salt`；schema v14 `space_crypto.kdf_salt BLOB`（`store.rs` v14 注释挂 SEC-AUDIT 编号）；hub `create_space` 客户端侧生成盐入 raft（`registry.rs` 头注，SM 确定性 apply 不用 CSPRNG）+ `upsert_space_kdf_salt` COALESCE 防旋转覆盖 | `random_kdf_salt_is_unpredictable_and_with_salt_is_deterministic` / `wp07` 9/9（D2 验收） |
| **P2-4** Zeroize 逃逸 | `Zeroizing<T>` 类型级封装 | §3③ 全链覆盖清单 | lib 13/13 |
| **P2-5** rustdoc 审计标注 | 补审计追踪标签 | `crypto.rs:9-12` / `pairing.rs:11-12` 审计编号 + 基准 RFC + 状态行 | — |

测试矩阵（HEAD 实测）：`cargo test -p partisync-sync --lib` **13/13**、
`--test wp04` **6/6**、`--test wp07` **9/9**（2026-09-30，本复核执行时）。

## 5. KDF 冻结验证（M2-report §4 冻结声明）

- 派生上下文与 M2 整改定版一致：`partisync space-key-v1` /
  `partisync content-key-v1` / `partisync meta-key-v1`；
  `blake3::derive_key` 构造无变更，无自研 KDF 回潮。
- `argon2_master_key`（确定性盐 legacy 版）仍为 `pub` 但**生产无调用方**
  （全仓仅自身单测引用），doc 已标注「仅限测试与无状态派生，生产必须走
  with_salt」。登记 F-1（API 面收窄建议），不构成冻结回退。

## 6. 发现与处置表 + 延续义务登记

| # | 级别 | 发现 | 处置去向 |
|---|---|---|---|
| F-1 | P3 | `argon2_master_key`（确定性盐 legacy）仍在公共 API 面，误用可绕过随机盐防线（doc 守卫无编译强制） | 债登记：建议 `#[doc(hidden)]` 或移入 `#[cfg(test)]`（随下次 sync crate 任务顺带） |
| F-2 | P3 | `endpoint_secret` 派生面无生产消费方——配对协议与 iroh 传输通道未合流（M2 设计的会话密钥材料接线悬置；当前传输=iroh 原生 QUIC/TLS + 载荷 E2EE，安全性不缺位，属设计对齐债） | 债登记：设备通道认证握手立项时消费或显式裁剪 |
| F-3 | P3 | `device_signing_key(seed)` 裸数组构造 dalek `SigningKey`，内部缓冲不随 Drop 清零（种子为持久身份材料，增量暴露有限） | 观察登记，无动作 |

**无 P0/P1/P2 新发现。**

### 延续义务登记（M7-WP00 §5 R4 格式）

- **义务**：外部密码学审计 + OSCP 持证人复核（M2-D1 / M4-D2 外部部分）。
- **状态**：**未清偿**，挂「资金回笼」事件触发；采购路径 RFP-OSCP-SECURITY-AUDIT-M5
  （4 候选就绪）与 M5-WP00 §4 提案保留待用。
- **本报告不构成外部审计等效**（执行方案 §7.1.10 的人工 vendor 签字要求
  仅外部路径可满足）。
- **复核留痕**：本报告 = M7 关账债务表回填素材；下次里程碑关账报告须复核
  本义务状态并留痕（M7-WP00 §5 R4 硬要求）。

## 7. 复核日志

- 复核时间：2026-09-30 · 基线 main=`3018ff9`
- 执行：GLM-5.3-Flash（ZCode 会话，M7-WP02-T01）
- 测试环境：macOS 26.6.2 arm64 · rustc 1.94 pin
- 测试：sync lib 13/13 · wp04 6/6 · wp07 9/9（全绿）
