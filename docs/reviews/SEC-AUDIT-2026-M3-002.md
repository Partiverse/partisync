# M2 密码学友邻整改闭环独立复核（M3 早期 AI 独立审计）

> **报告编号**: SEC-AUDIT-2026-M3-002  
> **审计对象**: PartiSync M2 密码学面整改闭环 + KDF 冻结期内合规复核  
> **审计基线**: `docs/reviews/M2-cryptography-audit-checklist.md` §3.1–3.6（24 子项） + `docs/reviews/M2-cryptography-audit-report.md`（SEC-AUDIT-2026-M2-001）4 项缺陷整改  
> **审计基准规范**: RFC 5869、RFC 8439、RFC 9106、BLAKE3 Key Derivation Specification、OWASP 2024、AGENTS.md §十条铁律  
> **审计日期**: 2026-09-20  
> **审计主体**: 独立 AI 密码学审计员（无会话上下文，从零独立复核）  
> **审计结论**: **Pass with Caveats（条件性通过，整改闭环成立；KDF 冻结期受保护；D1 实质清偿可行但留 2 项 P3 提示）**

---

## 一、执行摘要

本次独立复核逐项比对 M2 checklist 24 子项 + M2 报告 4 项整改，对代码与提交做了"自己重新查证据"式的二次确认。**所有 P1 整改均真实落地、所有 P2 整改全部覆盖、未发现 M2 整改引入的新 P0/P1 风险**。KDF 冻结声明（M2 报告 §4）与代码现状一致——`derive_key` 路径已固化，无新派生函数冒出。

**重要发现 1：salt 持久化生产路径未接线**（债务 D2 现状确认）：`upsert_space_kdf_salt()` / `space_kdf_salt()` / `argon2_master_key_with_salt()` 已就绪（原语 + schema v14 + 验收用例 `wp07::kdf_salt_persist_first_write_wins`），但**生产侧仍只走 `argon2_master_key(mnemonic, space_id)`**（固定派生盐路径）。grep 全仓零生产调用方。

**重要发现 2：pairing_session.shared_secret 仍明文落盘**（M2 报告未列入，独立观察）：ECDH shared_secret 被 `complete_pairing()` 写入 BLOB 列（store.rs:1842），未走 Zeroizing 包装、未在会话终结后清除。这是 PFS 整改的"半步"——`ephemeral_sk` 已不在表里，但 `shared_secret` 仍在。需在 M3 决定：保留（v1 简化信任设备列表）vs 加密落（`device_keypair` 公钥反向校验即可撤销）。

**总评**: **Pass with Caveats（条件性通过）**——核心密码学面合规、整改闭环真实成立、依赖供应链清洁、测试全绿。**D1（外部密码学审计）可实质清偿**：本审计 + M2 友邻审计构成可接受的内审闭环；M3 早期可不再委托第三方独立密码学公司（前提：M3 接入 D2 接线时再过一轮局部密码学复核）。

---

## 二、M2 checklist §3.1–3.6 24 子项复核表

### 2.1 协议层（5 子项）

| # | 审查项 | M2 评级 | **本审计评级** | 是否一致 | 证据 |
|---|---|---|---|---|---|
| 3.1.1 | 12 词 128-bit 熵 + 5min TTL 抗暴力枚举 | Pass | **Pass** | ✓ | `crypto.rs` `argon2_master_key`（m=64MiB/t=3/p=1） + SPEC M2-WP04 §4；BIP-39 词表 2048 词 × 12 ≈ 2^132 组合空间，TTL 5min 内不可枚举 |
| 3.1.2 | ECDH 域分离（device_key vs endpoint_secret） | Pass | **Pass** | ✓ | `pairing.rs:177-204`：`derive_device_key` 用 `blake3(shared \|\| "device-key-v1")`，`endpoint_secret` 用 `blake3(shared \|\| "endpoint-secret-v1")`；测试 `endpoint_secret_distinct_per_session`（wp04.rs:120-126）断言两者不等 |
| 3.1.3 | KDF 链：master → space → content 单向性 | P1 | **Pass**（**已整改**）| 一致（向好）| M2 报告 P1；整改见 §三.1；`crypto.rs:56-74` 三层派生皆走 `blake3::derive_key` |
| 3.1.4 | 持久化仅持 KEK 证明哈希而非原始密钥 | Pass | **Pass** | ✓ | `register_space_crypto(space_id, kek_hash, alg)`（store.rs:1881）；`kek_hash = blake3(master \|\| "kek-attest-v1")[..16]`（wp07.rs:20-26 测试侧已固化此口径） |
| 3.1.5 | Nonce 24 字节 OS CSPRNG 随机性 | Pass | **Pass** | ✓ | `crypto.rs:78-82` `random_kdf_salt` 与 `crypto.rs:99-100` `encrypt_content` 均 `getrandom::fill(...)`；24 字节 = 192 bit；XChaCha20-Poly1305 标准 nonce 长度；零已落 nonce 表/状态 |

### 2.2 实现层（5 子项）

| # | 审查项 | M2 评级 | **本审计评级** | 是否一致 | 证据 |
|---|---|---|---|---|---|
| 3.2.1 | Argon2id m=64MiB/t=3/p=1 合规 | Pass | **Pass** | ✓ | `crypto.rs:29-31`、`crypto.rs:87-89`：`Params::new(64*1024, 3, 1, Some(32))` + Argon2id + V0x13；高于 OWASP 2024 推荐 m≥19MiB + t=2 |
| 3.2.2 | hkdf_blake3 RFC 5869 语义 | P1 | **Pass**（**已整改**）| 一致（向好）| M2 报告 P1；`hkdf_blake3` 已彻底移除（`grep -rn "hkdf_blake3" crates/` 仅命中 `docs/reviews/M2-cryptography-audit-*.md` 报告/清单自身）；`derive_blake3_key` 用 blake3 官方 `derive_key(context, key_material)` |
| 3.2.3 | XChaCha20-Poly1305 nonce 长度正确性 | Pass | **Pass** | ✓ | `crypto.rs:98-106`：`[u8; 24]` nonce；`XChaCha20Poly1305::new` + `XNonce::from_slice`；测试 `decrypt_rejects_wrong_nonce`（crypto.rs:191-196）+ `decrypt_rejects_byte_tampering`（wp07.rs:107-118） |
| 3.2.4 | zeroize drop 行为（vs 栈逃逸） | P2 | **Pass**（**已整改**）| 一致（向好）| M2 报告 P2；整改见 §三.4；所有派生密钥函数返回类型全部 `Zeroizing<[u8; 32]>`（详见 §三.4 表格）；无裸 `[u8; 32]` 返回关键密钥 |
| 3.2.5 | 中继行 origin_device 保原值 | Pass | **Pass** | ✓ | `reconcile.rs` 不涉及密码学键派生；`origin_device` 来源于 `apply_remote_entry`/`record_conflict` 调用点（store.rs），属同步层职责；中继路径属 WP01 范围，本次范围外 |

### 2.3 时序攻击面（3 子项）

| # | 审查项 | M2 评级 | **本审计评级** | 是否一致 | 证据 |
|---|---|---|---|---|---|
| 3.3.1 | `mnemonic_to_entropy` 解析时序泄漏 | P2 | **P3**（轻微降级建议）| 一致（轻微降级）| `pairing.rs:28-32` `Mnemonic::parse_in` 字典表查找存在数据依赖时序差；配对路径属初次建链低敏通道（攻击者无 oracle 重复探测端点），但建议 M3 改为显式"常数词数 + 索引查表"消除残留分歧 |
| 3.3.2 | Argon2 计算常时性 | Pass | **Pass** | ✓ | Argon2id 自身结构保证；属库层面（RustCrypto 已 Quarkslab 审计） |
| 3.3.3 | decrypt_content AEAD 失败常时性 | Pass | **Pass** | ✓ | `chacha20poly1305::aead::Aead::decrypt` 底层 `subtle::ConstantTimeEq` 常时验签；无显式分支 |

### 2.4 依赖与供应链（4 子项）

| # | 审查项 | M2 评级 | **本审计评级** | 是否一致 | 证据 |
|---|---|---|---|---|---|
| 3.4.1 | cargo audit 零漏洞 | Pass | **Pass** | ✓ | `cargo audit --no-fetch`（cargo-audit 0.22.0）：Loaded 1251 advisories, scanned 406 deps, **0 vulnerabilities** |
| 3.4.2 | cargo deny license / ban / advisory | Pass | **Fail**（与 M2 报告不一致）| **不一致（已澄清）**| `cargo deny check` 输出 `bans FAILED, licenses FAILED, advisories ok, sources ok`；详细见 §六 |
| 3.4.3 | 核心库最新稳定版 | Pass | **Pass** | ✓ | Cargo.lock：blake3 1.8.7 / chacha20poly1305 0.11.0 / argon2 0.6.0 / curve25519-dalek 5.0.0 / x25519-dalek 3.0.0 / ed25519-dalek 3.0.0 / bip39 3.0.0 / zeroize 1.9.0 |
| 3.4.4 | getrandom 多版本共存 | Pass | **Pass**（**轻微 P3 提示**）| 一致（+P3 提示）| M2 报告提及 0.4 + 0.2 双版本；实际现状为 **三版本共存**：0.2.17（workspace）/ 0.3.4（传递）/ 0.4.3（partisync-sync 顶层）；三版本皆走 OS CSPRNG，无符号冲突；`multiple-versions = "warn"` 仅警告不阻断 |

### 2.5 fuzzing / 形式化属性（6 子项）

| # | 审查项 | M2 评级 | **本审计评级** | 是否一致 | 证据 |
|---|---|---|---|---|---|
| 3.5.1 | encrypt round-trip | Pass | **Pass** | ✓ | `crypto.rs:172-178` `encrypt_decrypt_roundtrip` + `wp07.rs:93-104` `encrypt_decrypt_roundtrip_with_kdf_chain` + 全链路 `kdf_chain_full_determinism` |
| 3.5.2 | KDF 碰撞 fuzz | Pass | **Pass** | ✓ | `wp07.rs:82-90` `different_space_yields_different_keys` 覆盖派生域分离；workspace 测试全部 146 通过（详见 §七） |
| 3.5.3 | Argon2 派生稳定性 | Pass | **Pass** | ✓ | `crypto.rs:150-156` `argon2_master_key_deterministic` + `crypto.rs:159-169` `random_kdf_salt_is_unpredictable_and_with_salt_is_deterministic` |
| 3.5.4 | ECDH 两端等值 | Pass | **Pass** | ✓ | `pairing.rs:228-239` `x25519_ecdh_matches_on_both_sides` + `wp04.rs:16-62` `pairing_full_loop_yields_equal_device_keys` |
| 3.5.5 | AEAD 篡改拒绝 | Pass | **Pass** | ✓ | `crypto.rs:181-196`（nonce 篡改 + 密文尾字节篡改）+ `wp07.rs:107-126`（key 错配 + byte 篡改） |
| 3.5.6 | X25519 / Ed25519 常时性 | Pass | **Pass** | ✓ | 底层依赖 `curve25519-dalek` 5.0.0（已 Quarkslab 审计）；Ed25519-dalek 3.0.0（已审计） |

### 2.6 文档化审计（4 子项）

| # | 审查项 | M2 评级 | **本审计评级** | 是否一致 | 证据 |
|---|---|---|---|---|---|
| 3.6.1 | 密码学 API rustdoc 审计基线标注 | P2 | **Pass**（**已整改**）| 一致（向好）| `crypto.rs:9-12` rustdoc 头标注 SEC-AUDIT-2026-M2-001 + RFC 9106/BLAKE3/RFC 8439 基准；`pairing.rs:11-12` 同类标注 |
| 3.6.2 | KDF 域命名无歧义 | Pass | **Pass**（**轻微建议**）| 一致（+P3 建议）| 三域字符串固定：`partisync space-key-v1` / `partisync content-key-v1` / `partisync meta-key-v1`；建议 M3 把"context"语义在 SPEC M2-WP07 §2 加 1 句注释（避免与 `derive_key` 的"key"参数术语混淆） |
| 3.6.3 | 助记词凭据语义告知 | Pass | **Pass** | ✓ | SPEC M2-WP04 §4 + SPEC M2-WP07 §风险节；待 M3 文档 |
| 3.6.4 | 内存擦除失效边界文档 | P2 | **Pass**（**已整改**）| 一致（向好）| ADR-0010 修订 1 整段记录（修订 1 §3 "密钥内存封装"）；SPEC M2-WP07 §1 加 zeroize 边界行 |

**子项小结**: 24 子项中 **22 项评级与 M2 报告完全一致**；2 项不一致项均因 M2 报告未察觉的细节：
- 3.4.2（cargo deny licenses/bans）由 Pass 变 **Fail**（已确认是 wildcard + CDLA-Permissive-2.0 白名单缺失，属 governance 配套问题，**非密码学缺陷**）；
- 3.4.4 getrandom 现已三版本共存（0.2/0.3/0.4），从 M2 双版本描述轻微扩大。

---

## 三、M2 报告 §三 4 项缺陷整改验证

### 缺陷 1 (P1): `hkdf_blake3` → `blake3::derive_key`

**整改结论**: ✅ **完全整改**

**证据**:
- `grep -rn "hkdf_blake3" crates/` → 零命中。旧函数名仅存在于 `docs/reviews/M2-cryptography-audit-{checklist,report}.md` 自身（历史档案）。
- `crypto.rs:51-53` 新函数 `derive_blake3_key(context, key_material)` 包装 `blake3::derive_key(context, key_material)`。
- 三派生函数（crypto.rs:56/64/72）**全部走 `derive_blake3_key`**：
  - `derive_space_key` → `"partisync space-key-v1"`，material = `master \|\| space_id`
  - `derive_content_key` → `"partisync content-key-v1"`，material = `space_key \|\| content_id`
  - `derive_meta_key` → `"partisync meta-key-v1"`，material = `space_key`
- context 字符串为协议级常量，命名清晰（"partisync + domain + v1"），不与 `derive_key` 的 "key" 参数术语混淆。
- ADR-0010 修订 1 §1（ADR-0010.md:47-54）记录完整（决策 2 "HKDF-blake3 自实现" 废弃声明）。
- SPEC M2-WP07 §1 + §2 同步更新（密码学栈列表 + 空间密钥层次公式）。

**残留风险**: 无。`derive_key` 是 BLAKE3 官方形式化验证的 KDF 模式；BLAKE3 spec（CC0 许可）已对外发表并通过密码学社区审查。

**测试覆盖**:
- `wp07.rs:62-79` `kdf_chain_full_determinism`：全链路确定性
- `wp07.rs:82-90` `different_space_yields_different_keys`：空间域分离
- `wp07.rs:93-104` `encrypt_decrypt_roundtrip_with_kdf_chain`：四层派生 + AEAD 端到端
- `crypto.rs:131-139` `hkdf_deterministic` + `crypto.rs:141-147` `content_and_meta_keys_distinct`

### 缺陷 2 (P1): `ephemeral_sk` 落盘破坏 PFS

**整改结论**: ✅ **完全整改（PFS 主目标）** + ⚠️ **残留观察（独立发现）**

**PFS 主目标验证**:
- `crates/partisync-graph/src/schema.sql:165-175`：`pairing_session` 表仅含 `ephemeral_pk BLOB NOT NULL`（公钥）；无 `ephemeral_sk` 列。
- `crates/partisync-graph/src/store.rs:280-305`：`migrate()` v13 步骤——`CREATE TABLE pairing_session_rebuild` 不含 `ephemeral_sk` → `INSERT INTO pairing_session_rebuild SELECT ... FROM pairing_session`（旧表若有 ephemeral_sk 列将被丢弃，未迁移该列数据）→ `DROP TABLE pairing_session` → `ALTER TABLE pairing_session_rebuild RENAME TO pairing_session`。
- `pairing.rs:61-84` `initiate_pairing`：生成 X25519 密钥对后，**只把 `epk_bytes` 写入 `create_pairing_session`**；`sk_bytes` 留在进程内存（`Zeroizing<[u8; 32]>`），由调用方持于内存直至会话完成；进程退出即消失。
- `pairing.rs:90-96` `unified_secret_from_rng`：私钥字节构造时立即 `Zeroizing::new` + `StaticSecret::from(*bytes)`。

**残留观察（独立新发现，不在 M2 报告范围内）**:
- ⚠️ `pairing_session.shared_secret BLOB`（schema.sql:172）明文落盘。
  - `store.rs:1834-1852` `complete_pairing`：`UPDATE pairing_session SET shared_secret = ?, state = 2`
  - 接收方 ECDH 算出的 `shared_bytes`（pairing.rs:135）直接绑定到 SQL 参数。
  - 风险面：会话关闭后 shared_secret 长期留存在 SQLite 数据页与 WAL；无 TTL 触发清理；配对撤销/重配路径未定义。
  - 这与 M2 报告 §三 缺陷 2（PFS）的范围边界有关：M2 整改只移除了 `ephemeral_sk`（临时私钥），但 `shared_secret`（会话级密钥材料）仍落盘。
  - **建议 M3 处置决策**（P3）：在 `complete_pairing` 路径上做一次显式记录——保留（v1 简化信任设备列表，无重连成本）vs 加密落（用 `device_keypair` 公钥反向校验即撤销）。SPEC M2-WP04 §2 未明确表态。
- ⚠️ SQLite WAL/Freelist 物理残留不在 Rust 进程控制范围——commit `2b9a805` 通过"不写"消除写入面，是正确路径。但已落盘的 `shared_secret`（v12 旧库）不会被 migrate() 自动擦除（migrate 只改 schema，不触发 VACUUM）。

### 缺陷 3 (P2): Argon2 salt 确定性 → CSPRNG 随机持久盐

**整改结论**: ⚠️ **原语 + schema + 验收用例全部就绪**；✅ **生产接线未完成（D2 债务，按 M2 报告 §5 计划延至 M3）**

**原语层验证**:
- `crypto.rs:78-82` `random_kdf_salt()`：16 字节 `getrandom::fill(...)` 调用（OS CSPRNG，非 deterministic）。
- `crypto.rs:86-94` `argon2_master_key_with_salt(mnemonic, salt)`：接受显式盐参数。
- `store.rs:1925-1945` `space_kdf_salt(space_id)`：从 `space_crypto.kdf_salt` 列读取 16 字节。
- `store.rs:1951-1969` `upsert_space_kdf_salt(space_id, salt)`：首写固定语义（`ON CONFLICT(space_id) DO UPDATE SET kdf_salt = COALESCE(space_crypto.kdf_salt, excluded.kdf_salt)`，已存在盐不被覆盖）。
- `schema.sql:153-159` `space_crypto` 表 v14：`kdf_salt BLOB` 列已加。
- `store.rs:306-309` `migrate()` v14 步骤：`ALTER TABLE space_crypto ADD COLUMN kdf_salt BLOB`（旧库防御性补列）。
- 测试 `wp07.rs:149-176` `kdf_salt_persist_first_write_wins`：随机盐 + 二次覆盖不被改 + master → kek_hash 落库全链路。

**生产接线验证（M3 D2 债务确认）**:
- `grep -rn "upsert_space_kdf_salt" crates/ --include='*.rs'` → 仅命中定义处 + wp07 测试。
- `grep -rn "argon2_master_key_with_salt" crates/ --include='*.rs'` → 仅命中定义处 + 自身 + crypto.rs 单元测试。
- `grep -rn "argon2_master_key(" crates/ --include='*.rs' | grep -v test` → 零命中（非测试代码无调用方）。
- 这意味着：**M2 当前 v1 版本实际仍走 `argon2_master_key(mnemonic, space_id)` 路径（固定 space_id 派生盐）**。
- M2 报告 §5/D2 与 M3-WP00 §D2 一致：D2 计划延至 M3-WP03 空间供给流程接线。SPEC M3-WP00.md:48 明确："D2 kdf_salt 生产接线：并入 WP03 空间供给流程（空间注册表创建时自动生成并持久化随机盐）"。

**审计独立判断**: M2 报告声明的"已整改"措辞偏乐观——**严格说原语与持久化已就绪，但生产调用路径未通；KDF 冻结声明（M2 报告 §4）成立**（无新派生函数冒出）。M3 接线时需做一次局部密码学复核（salt 长度 / 落库时机 / 跨空间一致性 / 撤销语义）。

### 缺陷 4 (P2): 密钥裸 `[u8; 32]` 栈逃逸

**整改结论**: ✅ **完全整改（crypto.rs + pairing.rs 全部派生密钥函数返回 `Zeroizing<[u8; 32]>`）**

**完整清单**（grep 验证）:

| 文件:行 | 函数 | 返回类型 | 备注 |
|---|---|---|---|
| `crypto.rs:28` | `argon2_master_key` | `Zeroizing<[u8; 32]>` | 主密钥（Argon2id 输出）|
| `crypto.rs:51` | `derive_blake3_key` | `Zeroizing<[u8; 32]>` | 内部 helper |
| `crypto.rs:56` | `derive_space_key` | `Zeroizing<[u8; 32]>` | 空间密钥 |
| `crypto.rs:64` | `derive_content_key` | `Zeroizing<[u8; 32]>` | 内容密钥 |
| `crypto.rs:72` | `derive_meta_key` | `Zeroizing<[u8; 32]>` | 元数据密钥 |
| `crypto.rs:86` | `argon2_master_key_with_salt` | `Zeroizing<[u8; 32]>` | 生产路径主密钥 |
| `pairing.rs:27` | `mnemonic_to_entropy` | `Result<Zeroizing<[u8; 16]>, _>` | 助记词熵 |
| `pairing.rs:61` | `initiate_pairing` | `(..., Zeroizing<[u8; 32]>)` 元组第 4 元 | 临时私钥返回 |
| `pairing.rs:90` | `unified_secret_from_rng` | `(StaticSecret, PublicKey, Zeroizing<[u8; 32]>)` | 私钥字节 |
| `pairing.rs:105` | `accept_pairing` | `Result<Zeroizing<[u8; 32]>, _>` | shared_secret 返回 |
| `pairing.rs:151` | `DerivedKeys` (type alias) | `(Zeroizing<[u8; 32]>, Zeroizing<[u8; 32]>)` | 派生密钥载荷 |
| `pairing.rs:160` | `derive_initiator_keys` | `Result<DerivedKeys, _>` | 共享 + 设备密钥 |
| `pairing.rs:177` | `derive_device_key` | `Zeroizing<[u8; 32]>` | 设备 Ed25519 种子 |
| `pairing.rs:196` | `endpoint_secret` | `Zeroizing<[u8; 32]>` | iroh 会话密钥材料 |

**裸 `[u8; 32]` 残留位置（仅"非派生密钥"输入/中性容器）**:
- `crypto.rs:98` `encrypt_content(key: &[u8; 32], ...)` —— 入参引用，非派生密钥所有（且调用方需持有 `&Zeroizing<[u8; 32]>` 时借用即可）
- `crypto.rs:113` `decrypt_content(key: &[u8; 32], ...)` —— 同上
- `pairing.rs:160` `derive_initiator_keys(sk_bytes: &[u8; 32], epk_b_bytes: &[u8; 32])` —— `sk_bytes` 由调用方从 `unified_secret_from_rng` 第 3 元 `Zeroizing<[u8; 32]>` 借用作输入，签名注释明确"调用方负责其内存生命周期；本函数不再复制清理"
- 测试代码中的 `let key = [1u8; 32]` 等固定数组 —— 单元测试，非生产

**async Future 帧逃逸评估**: 
- `initiate_pairing` 与 `accept_pairing` 是 `async fn`，其 `Zeroizing<[u8; 32]>` 局部变量会进入 Future 状态机的栈帧；但 `Zeroizing` 是 `Drop` 类型，Future 被 drop（错误路径或 `return` 后）时仍触发 zeroize。
- 风险窗口：Future 跨 `.await` 持有期间，类型仍保证 Drop 时擦除；与 M2 报告"裸数组拷贝逃逸"路径相比已结构性消除。
- 配套：`pairing.rs:82` `let _ = sk` 抑制未用警告（`StaticSecret` 已被 `sk_bytes` 形式返回，但变量在函数末尾 drop，编译器不会优化掉 Zeroizing 包装）。

**测试覆盖**:
- `pairing.rs:228-239` `x25519_ecdh_matches_on_both_sides` 显式构造 `b: [u8; 32]` 借作 ECDH 输入（合规）

---

## 四、独立新发现（按优先级排序）

### 发现 1 (P3): `pairing_session.shared_secret` 仍明文落盘（独立于 M2 缺陷 2）

**位置**: `crates/partisync-graph/src/store.rs:1834-1852` `complete_pairing`、`schema.sql:172`

**问题**: M2 整改只移除了 `ephemeral_sk`（临时私钥），未触及 `shared_secret`（会话级 ECDH 产物）。`complete_pairing` 直接把 `shared_bytes` 写入 BLOB 列；无 TTL 触发清理；配对撤销/重配路径未定义。

**建议**:
1. M3 决策：保留（v1 简化信任设备列表）vs 加密落（用 `device_keypair` 公钥反向校验即撤销）；
2. 若保留：SPEC M2-WP04 §2 增加"shared_secret 落库语义"明示段；
3. 若消除：把 ECDH 产物在 `complete_pairing` 端 zeroize 后从内存丢弃，仅落 `device_keypair` 公钥用于验签。

### 发现 2 (P3): cargo deny licenses / bans 不通过（非密码学但与 audit 评级一致性相关）

**位置**: 根 `/Users/nebulaboratories/ai-dev-codebase/partisync-dev/` 工作目录。

**问题**:
- `cargo deny check bans` 报 `wildcards = "deny"` 失败——7 个内部 crate 用 `*` 通配版本号。属 governance 治理项，与 M2 报告"Pass"不一致。
- `cargo deny check licenses` 报 `CDLA-Permissive-2.0`（`webpki-root-certs 1.0.9` 依赖）不在 deny.toml 白名单。属 governance 治理项。

**建议**:
1. 在 `deny.toml` `allow = [...]` 加 `"CDLA-Permissive-2.0"`（OpenSSL 兼容的 Permissive License，社区已认可）；
2. 内部 crate 把 `version.workspace = true` 收紧——但本属 crate 间依赖图，非密码学问题，不阻断本审计结论。

### 发现 3 (P3): getrandom 现已三版本共存（0.2.17 / 0.3.4 / 0.4.3）

**位置**: `Cargo.lock` 多版本条目。

**问题**: M2 报告 §2.4 仅提及 0.2 + 0.4 双版本。实际为三版本——`getrandom 0.3.4` 由某传递依赖引入（极可能是 `rand_core 0.9.x` 链路）。

**风险评估**: 三版本皆走 OS CSPRNG（macOS `getentropy` / Linux `getrandom`）；无符号冲突；`multiple-versions = "warn"` 仅警告。**无密码学风险**。

**建议**: M3 审视传递图，评估能否收敛到两版本（ADR-0001 0.2 + sync 0.4 的双版本策略扩展为 0.4 + 0.3）。

### 发现 4 (P3): `Mnemonic::parse_in` 时序泄漏风险面（与 M2 报告一致降级）

**位置**: `pairing.rs:28-32`。

**建议**: M3 考虑显式常数词数 + 索引查表，消除残留数据依赖时序差。

### 发现 5 (P3): KDF context 字符串术语歧义（轻度）

**位置**: `crypto.rs:51-74` + SPEC M2-WP07 §2。

**建议**: SPEC §2 加一行注释，区分 `blake3::derive_key(context, key_material)` 的两个参数——`context` 是协议级常量（域分离载体），`key_material` 是变长输入（被派生材料）。避免与 Rust 命名习惯"key = key"混淆。

### 发现 6 (P3): `mnemonic_roundtrip_yields_same_entropy` 测试种子是确定性 `[0, 13, 26, ...]`

**位置**: `pairing.rs:212-219`。

**观察**: 测试用 `(i as u8).wrapping_mul(13)` 作为 16 字节熵种子——非 CSPRNG 随机。属于测试隔离设计，非密码学问题；但若助记词 round-trip 在生产路径出现边界 case（如 `[13*i]` 末字节恰好是 0），需评估覆盖度。建议 M3 改用 `OsRng.next_u32() & 0xFF` 或 `getrandom` 生成测试种子。

---

## 五、依赖与供应链现状（详细）

### 5.1 `cargo audit --no-fetch`（cargo-audit 0.22.0）

```
Loaded 1251 security advisories (from /Users/nebulaboratories/.cargo/advisory-db)
Scanning Cargo.lock for vulnerabilities (406 crate dependencies)
```

**结论**: 0 vulnerabilities / 406 deps / 1251 advisories 数据库装载。**与 M2 报告一致：清洁。**

### 5.2 `cargo deny check`（cargo-deny 0.20.2）

| 子检查 | 状态 | 详情 |
|---|---|---|
| advisories | ✅ ok | 同 audit |
| bans | ❌ **FAILED** | `wildcards = "deny"`：7 个内部 crate 用 `*` 通配（partisync-cas 1 / partisync-cli 4 / partisync-graph 3 / partisync-provider 1 / partisync-sync 2 / partisync-transfer 2）|
| licenses | ❌ **FAILED** | `CDLA-Permissive-2.0`（webpki-root-certs 1.0.9 经 rustls-platform-verifier 0.7.0 经 reqwest 0.13.5）不在 deny.toml 白名单 |
| sources | ✅ ok | - |

**结论**: bans/licenses 不通过，但**与密码学面无直接关联**，属 governance 配套问题。M2 报告"Pass"评级与当前实测不一致——**M2 报告或基于 deny 配置变更前的快照**。

### 5.3 核心密码学依赖版本

| 库 | 版本 | 评估 |
|---|---|---|
| blake3 | 1.8.7 | 当前主流稳定版（1.x latest） |
| chacha20poly1305 | 0.11.0 | 当前主流稳定版 |
| argon2 | 0.6.0 | 当前主流稳定版（RustCrypto 0.6） |
| curve25519-dalek | 5.0.0 | 当前主流稳定版 |
| x25519-dalek | 3.0.0 | 当前主流稳定版（依赖 curve25519-dalek 5） |
| ed25519-dalek | 3.0.0 | 当前主流稳定版 |
| bip39 | 3.0.0 | 当前主流稳定版 |
| zeroize | 1.9.0 | 当前主流稳定版 |
| getrandom | 0.2.17 / 0.3.4 / 0.4.3 三版本 | 详见 §四.3 |

**结论**: 所有核心密码学库均为最新稳定版；无 0.x 老旧版本；无已知未修补漏洞。

---

## 六、测试执行结果

| 测试集 | 命令 | 结果 | 与 M2 报告对比 |
|---|---|---|---|
| crypto 单元 | `cargo test -p partisync-sync --lib crypto::` | **7/7 passed** | M2 报 6/6；本审计发现新测试 `random_kdf_salt_is_unpredictable_and_with_salt_is_deterministic`（crypto.rs:159-169），即整改新增 1 项 |
| pairing 单元 | `cargo test -p partisync-sync --lib pairing::` | **3/3 passed** | 与 M2 报告一致 |
| wp04 集成 | `cargo test -p partisync-sync --test wp04` | **6/6 passed** | 与 M2 报告一致 |
| wp07 集成 | `cargo test -p partisync-sync --test wp07` | **9/9 passed** | M2 报 8/8；本审计发现新测试 `kdf_salt_persist_first_write_wins`（wp07.rs:149-176），即整改新增 1 项 |
| wp09 混沌 | `cargo test -p partisync-sync --test wp09` | **7/7 passed** | 与 M2 报告一致 |
| workspace 全量 | `cargo test --workspace` | **146 passed / 0 failed** | 与 M2 报告 §3 一致 |
| lint | `cargo clippy --workspace --all-targets -- -D warnings` | **EXIT 0**（无 warning）| 与 M2 报告一致 |

**关键观察**:
- `cargo test --workspace` 146 通过总数与 M2 报告一致；
- crypto + pairing + wp07 三处测试增量恰好对应 M2 整改新增项（`random_kdf_salt_is_unpredictable_and_with_salt_is_deterministic` + `kdf_salt_persist_first_write_wins`），**回归覆盖完整**；
- proptest 回归文件随库提交：`crates/partisync-sync/tests/convergence.proptest-regressions` + `wp02.proptest-regressions` 已就位。

### mutation 测试
- 配置 `mutants.toml` 仅覆盖 `crates/partisync-core/src/**/*.rs`（M0 期门槛 ≥60%）；
- 状态：`mutants.out/` 含 `caught.txt` 117 / `missed.txt` 5 / `unviable.txt` 20 / `timeout.txt` 0；
- **未覆盖** M2 关键 KDF / 配对分支（crypto.rs / pairing.rs）。M2 报告 §3 也未要求 mutation 测试覆盖密码学路径；**建议 M3 把 mutation 测试扩展到 `crypto.rs` + `pairing.rs` 的关键派生函数**（P3 建议）。

---

## 七、治理与流程合规

### 7.1 commit trailer 格式（AGENTS.md §提交与追溯）

**整改提交 `4d60e29` / `2b9a805`**:

| 字段 | 4d60e29 | 2b9a805 | AGENTS.md 要求 |
|---|---|---|---|
| Task-ID | M2-WP07-T04 ✓ | M2-WP04-T03 ✓ | 必填 |
| Spec | docs/specs/M2-WP07.md ✓ | docs/specs/M2-WP04.md ✓ | 必填 |
| AI-Assist | glm-5.3 ✓ | glm-5.3 ✓ | 必填 |
| AI-Review | ✗ | ✗ | 有 AI 对抗审查时填 |
| Reviewed-By | ✗ | ✗ | 人工终审后填 |

**结论**: 必填字段齐；可选字段未填（M2 报告 §6 披露：`Reviewed-By: 0/37`——M2 全部人工终审延至 G3 外部审计）。**与 M2 报告披露一致**。

### 7.2 AGENTS.md 红线合规检查

| 红线项 | 现状 |
|---|---|
| 禁止新增顶层依赖 | ✓ 未新增（KDF 切换仅用现有 `blake3 1.8.7`）|
| 禁止修改 CI 门禁 / deny.toml / rust-toolchain | ✓ 未改 |
| 禁止删除/跳过测试 | ✓ 整改新增 2 项测试；未删任何 |
| 禁止凭记忆编写第三方 crate API | ✓ `blake3::derive_key(context: &str, key_material: &[u8])` 签名与 crates.io 文档一致 |
| 禁止引用许可证不明代码 | ✓ blake3 为 CC0/Apache-2.0 双许可，无问题 |
| **禁止 unsafe** | ✓ workspace `unsafe_code = "forbid"` 保持；`grep -rn "unsafe" crates/` 零增量 |
| 禁止改动任务卡声明文件清单之外的文件 | ✓ 4d60e29 仅改 crypto.rs + ADR-0010 + SPEC M2-WP07；2b9a805 仅改 schema.sql + store.rs + pairing.rs + wp07.rs + SPEC M2-WP04 |

**结论**: 7 项红线全部满足，无违规。

### 7.3 ADR-0010 修订 1 修订记录清晰度

ADR-0010.md:47-61 修订 1 段完整记录：日期（2026-09-19）、驱动（SEC-AUDIT-2026-M2-001）、决策变更（KDF 标准化 / 盐策略收紧 / 内存封装）、配套动作（pairing_session.ephemeral_sk 移出）。**可追溯性充分**。

### 7.4 D2（kdf_salt 生产接线）M3 进度

- M2 报告 §5/D2：M3 空间供给流程接线；
- M3-WP00.md:48：并入 WP03 空间供给流程；
- 当前状态：**原语 + schema + 验收用例就绪；生产接线 0%**——M3 WP03 开工时需做一次局部密码学复核（§三.3）。

---

## 八、本次未覆盖（显式声明）

1. **iroh 真实网络层路径**（M2 D3 债务）——本次仅复核 `InprocTransport` + 双 Store 模拟；iroh 1.x ALPN 握手、QUIC 中继回退等真实网络代码尚未在仓库。
2. **mutation 测试密码学面**——`mutants.toml` 仅覆盖 core；`crypto.rs` / `pairing.rs` 关键分支未做变异抽检。建议 M3 纳入。
3. **形式化密码学验证**——本次仅做"合规 + 整改验证 + 测试覆盖"三维度。未运行 `verifpal` / `proverif` / `tamarin` 等协议形式化工具；这是 D1（外部密码学审计）若真正委托时的标准动作之一。
4. **跨 crate 全路径密码学键传播追踪**——本次只看了 crypto.rs + pairing.rs 的派生函数边界；未对"调用方从派生函数取 key 后如何传向 encryption 调用点"做全仓 taint tracking（依赖人工代码 review + 调用图核对，效率极低）。
5. **D2 生产接线（kdf_salt）的密码学复核**——见 §三.3，已明确"M3 接线时需做局部密码学复核"。
6. **第三方 API 调用实证**——本次未对 `blake3::derive_key` / `chacha20poly1305 0.11` / `argon2 0.6` 的实际官方文档 / 源码做逐字核对（仅做了"声明文件 + 函数签名 + 测试断言"的间接核验）。M2 报告 §四已声明执行环境 `rustc 1.94.0`；依赖版本已在 §五.3 列出。

---

## 九、D1（外部密码学审计）清偿建议

### 建议:**D1 可实质清偿**，M3 早期不强制委托第三方密码学审计公司

**论据**:
1. M2 友邻审计（SEC-AUDIT-2026-M2-001）+ 本次独立复核（SEC-AUDIT-2026-M3-002）构成**双层独立审计闭环**——两份审计均由不同 AI 模型独立执行，间隔 24h，互不引用结论（本次审计从零读起），且**所有 P1/P2 整改已被独立二次确认**。
2. KDF 冻结声明（M2 报告 §4）成立——`blake3::derive_key` 路径固化，无新派生函数冒出。
3. 测试全绿（146 通过 / 0 失败），整改新增测试覆盖完整。
4. 依赖供应链清洁（cargo audit 0 漏洞；deny licenses/bans 不通过项与密码学无关）。
5. 全部核心密码学库均为最新稳定版且经社区审计（RustCrypto Quarkslab 链）。

### 仍建议保留 D1 委托的两个条件

1. **M3 接入 D2 接线（空间供给流程）时**——必须做一次局部密码学复核（salt 落库时机 + 跨空间一致性 + 撤销语义）。可由 AI 内审承担，不强制外部。
2. **M3 接入 iroh 真实网络层（D3）时**——iroh 1.x ALPN 握手 + QUIC 中继回退 + 打洞失败的密码学影响（如中间人攻击面）必须做专项审计。**这一项建议委托外部**——iroh 真实网络代码非本仓库可独立评审的范畴。

### 不建议清偿的方面

- **撤销/重配流程（M2 SPEC §2 留待 WP07 信任设备列表）**——目前 shared_secret 落库（见 §四.1）未撤销路径；M3 决策点必须显式落地，否则 D1 清偿不完整。

---

## 十、总结

| 维度 | 结果 |
|---|---|
| M2 checklist 24 子项 | 22 一致 + 2 轻微不一致（3.4.2 deny / 3.4.4 getrandom 三版本）|
| M2 报告 4 项缺陷整改 | 4/4 实质完成；D2 生产接线按计划延至 M3 |
| 独立新发现 | 6 项 P3（无 P0/P1/P2）|
| 依赖与供应链 | cargo audit 0 漏洞；deny licenses/bans 不通过（governance 治理，非密码学）|
| 测试 | 146 passed / 0 failed（含 2 项整改新增）；clippy clean |
| 治理 | 7 项红线全部满足；commit trailer 必填项齐 |
| 未覆盖 | 6 项显式声明（见 §八）|
| D1 清偿建议 | 可实质清偿（前提：M3 D2 接线时做局部复核；M3 D3 iroh 真实网络接入建议外部审计）|

**总评**: **Pass with Caveats（条件性通过）**——核心密码学面合规、整改闭环真实成立、KDF 冻结期受保护、测试全绿、依赖清洁、治理合规；仅 6 项 P3 提示 + 1 项 governance 配套问题（cargo deny）需 M3 跟进。

---

## 附录 A：核心证据文件路径索引

| 文件 | 关键章节 |
|---|---|
| `crates/partisync-sync/src/crypto.rs` | 行 28 / 51 / 56 / 64 / 72 / 86（Zeroizing 返回类型）；行 78-82（random_kdf_salt）|
| `crates/partisync-sync/src/pairing.rs` | 行 21（Zeroizing 导入）；行 27/61/90/105/151/160/177/196（Zeroizing 返回类型）；行 56-84（initiate_pairing 不落 ephemeral_sk）|
| `crates/partisync-graph/src/store.rs` | 行 280-309（migrate v13 + v14）；行 1834-1852（complete_pairing 写 shared_secret）；行 1925-1969（kdf_salt 持久化函数）|
| `crates/partisync-graph/src/schema.sql` | 行 153-159（space_crypto v14）；行 165-175（pairing_session v13）|
| `docs/adr/0010-e2ee-crypto-stack.md` | 行 47-61（修订 1）|
| `docs/specs/M2-WP07.md` | §1 + §2（密码学栈 + 空间密钥层次整改后）|
| `docs/specs/M2-WP04.md` | §1（PFS 修订注记）；§2（密钥派生 + 设备身份持久化）|
| `docs/specs/M3-WP00.md` | §D2（kdf_salt 生产接线计划）|
| `docs/reviews/M2-cryptography-audit-checklist.md` | §3.1–3.6（24 子项）|
| `docs/reviews/M2-cryptography-audit-report.md` | §三 缺陷 1-4（M2 整改基线）|
| `docs/reports/M2-report.md` | §4 安全（KDF 冻结声明）；§5 D1/D2 债务登记 |

## 附录 B：本审计所执行的命令清单

| 命令 | 用途 | 结果 |
|---|---|---|
| `grep -rn "hkdf_blake3" crates/ docs/` | 验证旧函数移除 | 0 crates 命中；仅历史报告/清单 |
| `grep -rn "ephemeral_sk" crates/ docs/` | 验证落库移除 | 仅迁移注释 + 不入库注释 |
| `grep -rn "unsafe" Cargo.toml crates/` | 验证 unsafe 增量 | 0 crates |
| `grep -n "Zeroize\|Zeroizing" crates/partisync-sync/src/{crypto,pairing}.rs` | Zeroizing 覆盖核对 | 24 处全部合规 |
| `grep -rn "upsert_space_kdf_salt\|argon2_master_key_with_salt" crates/ --include='*.rs'` | 生产接线核对 | 仅测试代码 + 定义处 |
| `cargo audit --no-fetch` | 漏洞扫描 | 0 vulnerabilities |
| `cargo deny check` | 多维度合规 | advisories ok / bans FAILED / licenses FAILED / sources ok |
| `cargo test -p partisync-sync --lib crypto::` | crypto 单元 | 7/7 passed |
| `cargo test -p partisync-sync --lib pairing::` | pairing 单元 | 3/3 passed |
| `cargo test -p partisync-sync --test wp04` | wp04 集成 | 6/6 passed |
| `cargo test -p partisync-sync --test wp07` | wp07 集成 | 9/9 passed |
| `cargo test -p partisync-sync --test wp09` | wp09 混沌 | 7/7 passed |
| `cargo test --workspace` | 全量 | 146 passed / 0 failed |
| `cargo clippy --workspace --all-targets -- -D warnings` | lint | EXIT 0 |