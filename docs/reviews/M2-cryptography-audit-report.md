# M2 密码学友邻审计报告（Independent Peer Review & Security Assessment）

> **报告编号**: SEC-AUDIT-2026-M2-001  
> **审计对象**: PartiSync M2 密码学与端到端加密栈（M2-WP04 设备配对网络 + M2-WP07 E2EE 加密层次）  
> **审计基准**: docs/reviews/M2-cryptography-audit-checklist.md  
> **关联规范与决策**: SPEC M2-WP04, SPEC M2-WP07, ADR-0008, ADR-0010, RFC 5869, RFC 9106, RFC 8439, OWASP 2024  
> **审计日期**: 2026-09-19  
> **审计主体**: 独立密码学友邻审计组（Peer Reviewer）  
> **审计结论**: **Conditional Pass（条件性通过，存在中高优待整改项，不阻断内部演进，但需在进入外部正式审计前完成修复）**

---

## 一、执行摘要（Executive Summary）

本次独立密码学审计依照 `docs/reviews/M2-cryptography-audit-checklist.md` 所列条目，对 `partisync-sync` 与 `partisync-graph` 中的密码学实现及协议设计进行了全面审查。审查覆盖了密钥生成、协商、派生、认证加密（AEAD）、内存安全与擦除、时序攻击面、依赖供应链及测试断言。

### 核心结论速览：**是否符合行业规范要求？**
- **总体结论**: **核心算法选型与参数强度基本符合行业主流高安全标准**（如 Argon2id m=64MiB/t=3, XChaCha20-Poly1305 24-byte Nonce, Ed25519/X25519 dalek 3.0 等）；
- **存在差距与违规点**: 
  1. **[P1] KDF 协议实现偏离行业标准**: `hkdf_blake3` 采用非标准的自制 Keyed-Hash 两阶段派生结构，而非 BLAKE3 官方形式化规范推荐的 Key Derivation Mode（`blake3::derive_key`）或 RFC 5869 标准 HKDF；
  2. **[P1] 临时私钥持久化违背前向安全原则**: 发起方临时私钥 `ephemeral_sk` 明文持久化到 SQLite 数据库中，即便后续状态关闭，未擦除的 SQLite 存储页与 WAL 依然留有泄漏风险；
  3. **[P2] Salt 确定性弱点**: `argon2_master_key` 的 Salt 仅绑定 `space_id`，未引入不可预测的密码学随机 Salt，在固定 space_id（如 `"default"`）下削弱了抗彩虹表攻击能力；
  4. **[P2] 内存敏感数据擦除未完整防御编译器逃逸**: 关键中间密钥裸露为 `[u8; 32]` 并穿透到异步 Future 帧与返回值，缺乏类型级常时清零（ZeroizeOnDrop）封装。

---

## 二、审计范围与核查矩阵（Checklist Verification）

本次审计逐项核实了 6 大维度共 24 个子项：

### 2.1 协议层（Protocol Level）

| 条目 | 审查项 | 审计结论 | 评级 | 发现与详细评估 |
|---|---|---|---|---|
| **3.1.1** | WP04 配对码 128-bit 熵在 5 min TTL 窗口内的暴力抗性 | **Pass** | Pass | 12 词对应 128 位熵（搜索空间 $2^{128}$），在 5 分钟窗口配合协议端速率限制，穷举完全不可行。 |
| **3.1.2** | WP04 ECDH 派生域分离（`device_key` vs `endpoint_secret`） | **Pass** | Pass | 派生分别采用固定命名空间上下文 `b"device-key-v1"` 和 `b"endpoint-secret-v1"`，严格杜绝会话凭证跨用途复用。 |
| **3.1.3** | WP07 KDF 层次（master → space → content/meta）单向性 | **Pass with Caveats** | **P1** | 层次结构清晰，但在派生算法底层采用了未经验证的自建两阶段 keyed hash，建议统一到标准接口。 |
| **3.1.4** | WP07 持久化仅持有 KEK 证明哈希而非原始密钥 | **Pass** | Pass | `kek_hash` 使用 `blake3(master \| kek-attest-v1)[..16]` 截断存储，不反向泄露 master_key。 |
| **3.1.5** | WP07 Nonce 生成随机性与碰撞概率 | **Pass** | Pass | XChaCha20 采用 24 字节（192-bit）OS 密码学随机数，碰撞界限约为 $2^{-96}$，抗碰撞与无状态并发满足工业级标准。 |

### 2.2 实现层（Implementation Level）

| 条目 | 审查项 | 审计结论 | 评级 | 发现与详细评估 |
|---|---|---|---|---|
| **3.2.1** | `argon2_master_key` 参数合规性 | **Pass** | Pass | 参数为 Argon2id v0x13, $m=64\text{MiB}, t=3, p=1, \text{tag}=32$ 字节，完全符合 RFC 9106 与 OWASP 2024 推荐上限。 |
| **3.2.2** | `hkdf_blake3` RFC 5869 语义符合度 | **Fail (Non-compliant)** | **P1** | 内部逻辑直接用 `blake3(master \| info \| 0x01)` 生成 key 再做 keyed-hash，非 RFC 5869，且未用官方 `blake3::derive_key`。 |
| **3.2.3** | `chacha20poly1305` Nonce 长度与 AEAD 绑定 | **Pass** | Pass | 严格遵循 XChaCha20-Poly1305 24 字节 Nonce，密文末尾携带 16 字节 Poly1305 认证标签，解密失败抛 Fatal。 |
| **3.2.4** | `zeroize` 内存擦除行为分析 | **Needs Improvement** | **P2** | 局部栈数组虽调用 `.zeroize()`，但因通过返回值 `[u8; 32]` 或传参至 `async` Future 状态机，未受 `Zeroizing<T>` 类型保护，存在逃逸风险。 |
| **3.2.5** | `record_oplog_raw` 跨端中继防回环 | **Pass** | Pass | `origin_device` 保留原值，同步会话 `dst_device` 匹配即跳过，避免回环死锁与密钥回溯问题。 |

### 2.3 时序攻击面（Side-Channel & Timing Attack）

| 条目 | 审查项 | 审计结论 | 评级 | 发现与详细评估 |
|---|---|---|---|---|
| **3.3.1** | `mnemonic_to_entropy` 字典解析时序泄漏 | **Pass with Caveats** | **P2** | `bip39::Mnemonic::parse_in` 基于单词索引循环查找，存在微小的数据依赖时序差，但在配对建立阶段属于低敏通道。 |
| **3.3.2** | `argon2_master_key` 计算常时性 | **Pass** | Pass | Argon2id 前半段数据无关（防侧信道缓存时序），后半段数据相关（防 GPU 攻击），符合密码学常时防侧信道要求。 |
| **3.3.3** | `decrypt_content` 认证失败常时性 | **Pass** | Pass | Poly1305 标签比对在底层 `chacha20poly1305` 库中采用常时比对（`subtle::ConstantTimeEq`），失败不泄露明文先验。 |

### 2.4 依赖与供应链安全（Dependency & Supply Chain）

| 条目 | 审查项 | 审计结论 | 评级 | 发现与详细评估 |
|---|---|---|---|---|
| **3.4.1** | `cargo audit` 漏洞扫描 | **Pass** | Pass | 扫描 390 个 crate 依赖，0 已知安全漏洞（Advisories Clean）。 |
| **3.4.2** | 许可证与安全策略合规 | **Pass** | Pass | 核心密码学库全为 Apache-2.0 / MIT，符合 `deny.toml` 白名单。 |
| **3.4.3** | 核心密码学库版本稳定性 | **Pass** | Pass | dalek 3.0, argon2 0.6, chacha20poly1305 0.11, blake3 1.8 均为当前最新且经受审计的主流版本。 |
| **3.4.4** | `getrandom` 双版本（0.4 与 0.2）共存 | **Pass** | Pass | 单 crate 内链接独立系统 CSPRNG（macOS `getentropy`/Linux `getrandom`），无符号冲突与 ABI 劫持风险。 |

### 2.5 形式化属性与自测试（Fuzzing & Invariants）

| 条目 | 审查项 | 审计结论 | 评级 | 发现与详细评估 |
|---|---|---|---|---|
| **3.5.1** | 加解密 Round-trip 等值性 | **Pass** | Pass | 单元测试与端到端测试均验证通过。 |
| **3.5.2** | KDF 碰撞抗性与单调性 | **Pass** | Pass | 1000 轮空间/内容 ID 派生测试 0 碰撞。 |
| **3.5.3** | 主密钥派生确定性与空间隔离 | **Pass** | Pass | 跨空间即使相同助记词，派生结果严格不同；相同空间参数下 100% 确定性。 |
| **3.5.4** | X25519 ECDH 两端等值协商 | **Pass** | Pass | 双端独立计算 $sk_a \cdot pk_b == sk_b \cdot pk_a$ 严格等值。 |
| **3.5.5** | AEAD 密文单字节篡改自检 | **Pass** | Pass | 任意篡改 1 字节（含 Nonce 或密文）均被 Poly1305 拒绝并拦截。 |
| **3.5.6** | X25519 / Ed25519 常时实现 | **Pass** | Pass | 底层依赖 `curve25519-dalek` 已经过 Quarkslab 专业第三方审计并保证常时。 |

### 2.6 文档与工程规范（Documentation）

| 条目 | 审查项 | 审计结论 | 评级 | 发现与详细评估 |
|---|---|---|---|---|
| **3.6.1** | 密码学 API rustdoc 标注审计基线 | **Needs Fix** | **P2** | `crypto.rs` 与 `pairing.rs` 缺少明确的审计追踪标签，需按照规范补齐。 |
| **3.6.2** | KDF 命名空间统一无歧义 | **Pass** | Pass | `space-key-v1`, `content_id|c`, `meta-v1` 均符合 M2-WP07 规约。 |
| **3.6.3** | 敏感助记词用户风险警示 | **Pass** | Pass | 文档已明确将助记词定性为根凭据。 |

---

## 三、关键审计缺陷与风险深入分析

### 缺陷 1 (P1 - 必须整改): 自研 `hkdf_blake3` 偏离行业规范，存在自定义密码学构造风险
- **风险位置**: `crates/partisync-sync/src/crypto.rs` (第 42-60 行)
- **问题描述**:
  RFC 5869 标准 HKDF 包含基于 HMAC 的 Extract 与 Expand 步骤。当前代码自行使用 `blake3(master || info || 0x01)` 产生伪密钥，再利用 `blake3::new_keyed` 再次混合 `master` 和常量串进行哈希。
  密码学工程铁律是 **"Don't Roll Your Own Crypto"**。BLAKE3 官方库原作者已经针对密钥派生需求设计了形式化验证的专用模式：`blake3::derive_key(context: &str, key_material: &[u8])`。
  该官方 API 内部自带严格的域分离（Domain Separation）机制与上下文前缀，无需开发者手工拼接字符串或多次调用 Hasher。
- **整改建议**:
  重构 `hkdf_blake3`，直接调用 `blake3::derive_key`：
  ```rust
  pub fn hkdf_blake3(master: &[u8; 32], context: &str) -> [u8; 32] {
      blake3::derive_key(context, master)
  }
  ```

### 缺陷 2 (P1 - 必须整改): 临时私钥 `ephemeral_sk` 明文持久化破坏前向安全性（PFS）
- **风险位置**: `crates/partisync-graph/src/schema.sql` (第 165 行) 与 `store.rs`
- **问题描述**:
  在配对流程中，发起方的临时私钥 `ephemeral_sk` 被直接通过 SQL `INSERT INTO pairing_session` 存入了 SQLite 磁盘文件。
  虽然配对会话设置了 5 分钟过期以及配对成功后的状态机闭环，但 SQLite 的 `UPDATE` 或 `DELETE` 操作**绝不会在物理存储介质上即时清零或安全擦除数据页**，剩余的 WAL 日志或空闲页（Freelist Pages）会长期保留该临时私钥。如果设备遭受本地文件提取或取证分析，攻击者可还原历史配对私钥，进而打破前向安全。
- **整改建议**:
  1. `ephemeral_sk` 原则上仅应留存与发起方进程内存中（由内存安全数据结构或缓存生命周期管理）；
  2. 若因同进程多 Store 跨实例测试需要跨表读取，必须在会话结束时执行显式物理覆盖清零，或利用临时内存数据库（In-Memory DB）维护未握手会话。

### 缺陷 3 (P2 - 建议整改): `argon2_master_key` 的 Salt 缺乏密码学随机性
- **风险位置**: `crates/partisync-sync/src/crypto.rs` (第 23-29 行)
- **问题描述**:
  `salt` 仅由 `space_id` 经哈希截取 16 字节产生。在默认配置下，大量单用户空间的 `space_id` 均为 `"default"`。这意味着针对相同空间名称的用户，其 Argon2 Salt 是全局一致的静态已知值。攻击者可针对常见助记词预先计算彩虹表或批处理攻击字典。
- **整改建议**:
  1. 规范层面强制要求 `space_id` 必须为高熵全局唯一标识符（如 ULID / UUIDv4）；
  2. 或在 `space_crypto` 表中增加独立的 16 字节随机 `salt` 字段，并在空间初次初始化时生成并存储。

### 缺陷 4 (P2 - 建议整改): 敏感密钥内存擦除未能免疫 Rust 编译器优化与栈逃逸
- **风险位置**: `crates/partisync-sync/src/pairing.rs` 与 `crypto.rs`
- **问题描述**:
  当前代码虽局部调用了 `salt.zeroize()` 与 `key.zeroize()`，但核心私钥是以纯裸数组 `[u8; 32]` 进行返回值传递的（例如 `argon2_master_key` 返回 `[u8; 32]`，`derive_initiator_keys` 返回元组）。这些裸数组在函数返回、栈帧弹出或作为参数传入异步 Future 时，编译器通常会执行内存拷贝，而残留拷贝完全无法被自动清零。
- **整改建议**:
  引入 `zeroize::Zeroizing` 包装类型：
  ```rust
  use zeroize::Zeroizing;
  pub fn argon2_master_key(...) -> Zeroizing<[u8; 32]> { ... }
  ```

---

## 四、审计日志与跟踪信息（Audit Log）

- **审计时间**: 2026-09-19
- **审计员**: GLM-5.3 Peer Review Team
- **会话范围**: M2-WP04 (Pairing) + M2-WP07 (E2EE) + partisync-graph schema/store
- **工具链环境**: rustc 1.94.0, cargo-audit 0.21.2, SQLx SQLite
- **执行结果**:
  - `cargo test -p partisync-sync --lib crypto::` -> 6/6 通过
  - `cargo test -p partisync-sync --lib pairing::` -> 3/3 通过
  - `cargo test -p partisync-sync --test wp04` -> 6/6 通过
  - `cargo test -p partisync-sync --test wp07` -> 8/8 通过
  - `cargo audit --no-fetch` -> 0 vulnerabilities

---

## 五、最终审计结论与 M2 关门准入建议

1. **总评级**: **Conditional Pass（条件性通过 / 建议外部审计前整改）**
2. **阻断项（P0）**: **0 项**（无灾难性单点破译风险，无硬编码秘钥，无弱加密算法）
3. **高优项（P1）**: **2 项**（自制 HKDF 规范偏离、临时私钥落盘破坏前向安全性）
4. **中低优项（P2/P3）**: **3 项**（Salt 确定性风险、内存 Zeroize 逃逸保护、API 审计文档标签）
5. **与 M2 关门关系**: **可继续（内部里程碑准许合入）**。本审计为友邻内审，已达到“关门前自查排雷”目的；
6. **对 M2 G3 硬门禁（外部正式密码学审计）的建议**:
   - 在外部审计公司入场前，优先按照本报告将 `hkdf_blake3` 切换为官方推荐的 `blake3::derive_key`；
   - 将 `ephemeral_sk` 改为内存态或安全信封，消除磁盘取证后门疑虑；
   - 该整改可显著缩减外部安全公司审查工时，确保 G3 门禁顺利一次性签发。
