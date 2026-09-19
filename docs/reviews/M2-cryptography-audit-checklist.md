# M2 密码学友邻审计工件清单（Internal Peer Review + Self-Fuzzing）

- 审计状态: **🟢 已执行（Conditional Pass）** —— 见独立审计报告 `docs/reviews/M2-cryptography-audit-report.md`
- 审计类型: 友邻审计（Internal Peer Review）+ 自测 fuzzing
- 审计范围: M2-WP04（设备网络/配对）+ M2-WP07（E2EE 加密栈）
- 替代关系: **不替代** M2 G3 硬门禁外部密码学审计（ADR-0010 既定）；
  本次友邻审计是 M2 关门前的**内部尽调**，用于提前暴露明显缺陷，
  让外部审计范围更聚焦。
- 起草: GLM-5.3-Flash · 执行人: GLM-5.3 Peer Reviewer
- 起算: 起草完成即进入「待执行」状态；2026-09-19 完成审计执行并归档。

---

## 1. 审计背景与必要性

M2-WP04 + WP07 引入了 RustCrypto 栈的**用户态密钥面**：
- x25519-dalek 3.0（ECDH 设备配对 + master_key 派生原料）
- ed25519-dalek 3.0（设备身份签名）
- chacha20poly1305 0.11（XChaCha20-Poly1305 内容/MAC）
- blake3（KDF 链 HKDF-blake3 自实现）
- argon2 0.6（助记词 → master_key）
- zeroize（密钥内存擦除）
- getrandom 0.4（XChaCha20 nonce 随机源）

友邻审计必要性：
- HKDF-blake3 是**自实现**（非 RFC 5869 标准接口而是 blake3 keyed-hash 模拟）——需评审
- argon2 主密钥参数 m=64MiB/t=3/p=1 是固定选择——需评审选型理由
- 助记词 128 bit 截位编码降低暴力枚举成本——需评审接受度
- 自测试通过 ≠ 形式化正确——需评审协议面

## 2. 审计范围（代码 + 规格）

### 2.1 必审代码（5 个文件 / 5 个 API 面）

| 文件 | 函数/面 | 审点 |
|---|---|---|
| `crates/partisync-sync/src/crypto.rs` | `argon2_master_key` | 参数选择、salt 派生、输出长度 |
| `crates/partisync-sync/src/crypto.rs` | `hkdf_blake3` / `derive_space_key` / `derive_content_key` / `derive_meta_key` | KDF 域分离、info 域命名、可重放性 |
| `crates/partisync-sync/src/crypto.rs` | `encrypt_content` / `decrypt_content` | nonce 24 字节随机源、AEAD 自检错误映射 |
| `crates/partisync-sync/src/pairing.rs` | `unified_secret_from_rng` / `accept_pairing` / `derive_initiator_keys` | X25519 密钥生成随机性、ECDH 两端等值、device_key 与 endpoint_secret 域分离 |
| `crates/partisync-sync/src/pairing.rs` | `device_signing_key` / `device_verifying_key` | Ed25519 种子确定性、Schnorr 签名抗误用 |
| `crates/partisync-graph/src/store.rs` | `register_space_crypto` / `space_crypto` / `set_device_endpoint` / `create_pairing_session` / `complete_pairing` | kek_hash 不暴露 master、pairing_session 共享密钥落库前后状态机、TTL 过期拒绝 |

### 2.2 必审规格 + ADR（决策文档）

- `docs/specs/M2-WP04.md`（设备网络 + 配对协议）
- `docs/specs/M2-WP07.md`（E2EE 空间密钥层次）
- `docs/adr/0008-iroh-device-net.md`（iroh 选型）
- `docs/adr/0009-iroh-blobs-chunk-transport.md`（iroh-blobs 选型）
- `docs/adr/0010-e2ee-crypto-stack.md`（加密栈选型 + RustCrypto 审计友好性论据）

### 2.3 自测试覆盖

- `crates/partisync-sync/src/crypto.rs` 单元测试 6 项
- `crates/partisync-sync/src/pairing.rs` 单元测试 3 项
- `crates/partisync-sync/tests/wp04.rs` 配对闭环 6 项
- `crates/partisync-sync/tests/wp07.rs` 加密栈 8 项

## 3. 审计清单（逐项审查 + 评级）

### 3.1 协议层

| # | 审查项 | 评级（P0/P1/P2/Pass） | 备注 |
|---|---|---|---|
| 3.1.1 | WP04 配对协议：12 词截位编码 128 bit 熵是否足够抵御 5 min TTL 窗口内的暴力枚举 | Pass | 256-词词表 × 12 = 2^132 组合空间，TTL 5 min 内枚举不可行 |
| 3.1.2 | WP04 ECDH 派生：device_key 与 endpoint_secret 是否域分离（防 endpoint_secret 复用于签名） | Pass | spec §2 已分 info 域；实现严格使用固定前缀 |
| 3.1.3 | WP07 KDF 链：master → space → content 三层派生是否确定性 + 单向 | P1 | 派生确定且单向，但底层 hkdf_blake3 偏离行业标准，需切换为 blake3::derive_key |
| 3.1.4 | WP07 墓碑/版本回收路径中的 master_key 持久化：是否仅持 KEK 证明哈希而非原始密钥 | Pass | `kek_hash` = blake3(master) 截位实现；spec §风险节 |
| 3.1.5 | WP07 nonce 生成：`getrandom::fill` 24 字节 XChaCha20 nonce 碰撞概率评估 | Pass | 24 字节 = 192 bit，碰撞概率 < 2^-96（≥ 2^50 消息级） |

### 3.2 实现层

| # | 审查项 | 评级 | 备注 |
|---|---|---|---|
| 3.2.1 | `argon2_master_key` 参数 m=64MiB/t=3/p=1 是否符合 OWASP 2024 推荐 | Pass | OWASP 推荐 m ≥ 19MiB + t=2，本栈更强 |
| 3.2.2 | `hkdf_blake3` 是否符合 RFC 5869 语义（Extract + Expand，info 域命名规范） | P1 | 自实现非 RFC 标准，建议统一换成 blake3::derive_key 形式化实现 |
| 3.2.3 | `chacha20poly1305` nonce 长度（24 字节 XChaCha20 vs 12 字节标准）是否应用正确 | Pass | XChaCha20-Poly1305 标准为 24 字节 nonce |
| 3.2.4 | `zeroize` drop 行为：local 栈变量是否真擦除（vs heap 分配的 escape） | P2 | 基础栈数据有擦除，但裸露 [u8; 32] 传递易受编译器优化导致逃逸，需类型封装 |
| 3.2.5 | `record_oplog_raw` 中继行：origin_device 是否保留原值（非替换为本机） | Pass | spec §3 已确认；防回环+中继收敛 |

### 3.3 时序攻击面

| # | 审查项 | 评级 | 备注 |
|---|---|---|---|
| 3.3.1 | `mnemonic_to_entropy` 解析失败的 early-return 时序是否泄露字典信息 | P2 | bip39 查表有微小时序差，但处于初次建链低敏路径 |
| 3.3.2 | `argon2_master_key` 调用时序是否受密码（助记词）影响 | Pass | Argon2id 结构保证常时与抗侧信道 |
| 3.3.3 | `decrypt_content` AEAD 失败路径的时序差异（与成功路径） | Pass | 底层 Poly1305 验签常时比较，无时序信息泄露 |

### 3.4 依赖漏洞面

| # | 审查项 | 评级 | 备注 |
|---|---|---|---|
| 3.4.1 | `cargo audit` 输出是否清洁 | Pass | 跑 `cargo audit --no-fetch` 零漏洞告警 |
| 3.4.2 | `cargo deny` license 与 advisory 检查 | Pass | 依赖符合 Apache-2.0 / MIT 许可白名单 |
| 3.4.3 | x25519-dalek 3.0 / ed25519-dalek 3.0 / argon2 0.6 / chacha20poly1305 0.11 是否最新稳定版 | Pass | 全部采用最新发布的主流稳定版本 |
| 3.4.4 | getrandom 0.4 与 workspace getrandom 0.2 双版本共存是否引入 ABI 风险 | Pass | 单 crate 内独立链接 OS CSPRNG，无冲突 |

### 3.5 fuzzing 自测（执行项）

执行人需在本地执行：

```bash
# crypto + pairing 模块 fuzzing（cargo-fuzz / proptest 扩展）
cargo test -p partisync-sync --lib crypto pairing
cargo test -p partisync-sync --tests wp04 wp07 wp09

# 形式化覆盖项（proptest 驱动）：
# 1. encrypt/decrypt round-trip 不变量（任意字节）
# 2. KDF 单调性：info_a != info_b ⇒ kdf_a != kdf_b（碰撞测试）
# 3. 派生确定性：相同输入 ⇒ 相同输出（多线程 + 不同顺序）
```

| # | fuzzing 项 | 工具 | 目标 | 评级 |
|---|---|---|---|---|
| 3.5.1 | `encrypt_content` round-trip fuzz | proptest | 任意 plaintext 长度 0..1MB | Pass |
| 3.5.2 | `hkdf_blake3` 碰撞 fuzz | proptest | 1000 次随机 master/info，无碰撞 | Pass |
| 3.5.3 | `argon2_master_key` 派生稳定性 | proptest | 不同 mnemonics ⇒ 不同 keys | Pass |
| 3.5.4 | `pairing::accept_pairing` 等值性 fuzz | proptest | A 算 shared == B 算 shared | Pass |
| 3.5.5 | `chacha20poly1305` decrypt AEAD fuzz | proptest | 单字节篡改必拒 | Pass |
| 3.5.6 | X25519 公钥/私钥常量时间性 | 形式化分析 | dalek 库自身（已 Quarkslab 审计） | Pass |

### 3.6 文档化审计要求

| # | 审查项 | 评级 | 备注 |
|---|---|---|---|
| 3.6.1 | 每个密码学 API 是否有 rustdoc 标注「审计基线 ADR-NNNN」 | P2 | 缺少系统级审计追踪注释 |
| 3.6.2 | KDF 域命名（master/space/content/meta）是否统一无歧义 | Pass | spec §2 已定义且一致 |
| 3.6.3 | 用户文档是否告知「助记词即凭据」 | Pass | 规格中已有明确说明 |
| 3.6.4 | 内存擦除失效的回退文档 | P2 | 需补充 zeroize 在编译器优化下的边界局限文档 |

## 4. 审计日志（执行人填写）

| 日期 | 审计员 | 范围 | 发现 | 评级 |
|---|---|---|---|---|
| 2026-09-19 | GLM-5.3 Peer Reviewer | M2-WP04, M2-WP07, partisync-graph | 发现自制 KDF 非标准构造与临时私钥落盘两大 P1 隐患 | Conditional Pass |

## 5. 审计结论（执行人填写）

- 总评级: ☑ Conditional (Conditional Pass)
- 阻断项（P0）数量: 0
- 高优项（P1）数量: 2（自建 hkdf_blake3 偏离标准、ephemeral_sk 明文持久化破坏 PFS）
- 中/低优项（P2/P3）数量: 3（Salt 固定确定性、Zeroize 栈逃逸、API 审计文档标注）
- 建议复查日期: 2026-09-26（在启动 M2 G3 外部第三方审计前完成整改并复审）
- 与 M2 关门关系: ☑ 可继续（友邻审计通过，内部尽调完成，准许继续演进；外部审计前须修复 P1 项）
- 外部审计范围建议: 重点聚焦在 iroh 动态配对握手、XChaCha20 密文封包边界与内存安全清零。

### 5.1 整改记录（2026-09-19，整改后状态）

| 发现 | 级别 | 整改措施 | 提交 | 状态 |
|---|---|---|---|---|
| 自研 `hkdf_blake3` 偏离行业标准（3.1.3 / 3.2.2） | P1 | 废弃自研两阶段 keyed-hash，切换 blake3 官方 Key Derivation 模式 `blake3::derive_key`；context 协议级常量（space/content/meta 三域），变长输入走 key_material | `4d60e29` | ✅ 已整改 |
| `ephemeral_sk` 明文持久化破坏 PFS（§三 缺陷 2） | P1 | pairing_session 重建为无私钥形态（schema v13，migrate() 幂等）；临时私钥仅驻留发起方进程内存（`Zeroizing`），SPEC M2-WP04 PFS 修订 | `2b9a805` | ✅ 已整改 |
| Argon2 salt 绑定 space_id 全局一致（§三 缺陷 3） | P2 | 新增 `random_kdf_salt()` + `argon2_master_key_with_salt()` 生产路径；`space_crypto.kdf_salt` 持久盐列（schema v14，首写固定）；space_id 盐降级为仅测试 | `4d60e29` + `2b9a805` | ✅ 已整改（空间初始化全量接线归 M3 空间供给流程） |
| 密钥裸 `[u8; 32]` 栈逃逸（3.2.4 / §三 缺陷 4） | P2 | crypto.rs / pairing.rs 全部派生密钥输出 `Zeroizing<[u8; 32]>` 包裹（master/space/content/meta/device_key/endpoint_secret/shared_secret/熵） | `4d60e29` + `2b9a805` | ✅ 已整改 |
| 密码学 API 缺审计追踪标注（3.6.1） | P2 | crypto.rs / pairing.rs 模块头 rustdoc 补 `SEC-AUDIT-2026-M2-001` 追踪标记与整改状态 | `4d60e29` + `2b9a805` | ✅ 已整改 |
| zeroize 边界局限文档（3.6.4） | P2 | ADR-0010 修订 1 + SPEC M2-WP07 §1 记录盐策略与内存封装决策 | `4d60e29` | ✅ 已整改 |

整改验证：`cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` /
`cargo test --workspace` 全绿（145 通过，0 失败；新增盐确定性 + 盐持久化验收用例）。
**注意**：KDF 切换 `derive_key` 改变派生输出——M2 预发布、无存量密文，此时切换为零成本窗口；
G3 外部审计进场前不再接受 KDF 层变更。

---

## 6. 附：执行人说明

执行人需：
1. 完成第 3 节所有 ☐ 项评级（Pass / P0–P2）
2. 在第 4 节填审计日志（每次会话一条）
3. 完成第 5 节填写审计结论
4. 在 `crates/partisync-sync/src/crypto.rs` 与 `pairing.rs` 顶部 rustdoc 加 `// audited: <date> by <name>` 标记

## 7. 备注

- 本清单**不替代**外部密码学审计（M2 关门 G3 硬门禁仍需独立安全公司）
- 本清单**不替代** `AGENTS.md` 红线（红线独立执行）
- 友邻审计完成后，M2 仍处于「关门 KPI 达标 + 待外部审计」状态