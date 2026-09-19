# ADR-0010: E2EE 加密栈以 x25519-dalek + chacha20poly1305 + blake3 + argon2 为底座

状态: 提议 · 日期: 2026-09-19 · 决策人: @lead（AI 代理起草，人工签核待补）
关联: SPEC M2-WP07、调研方案 §5.11（E2EE + 检索张力）、执行方案 §6.3（外部密码学审计为 G3 硬门禁）

## 背景
M2-WP07 端到端加密：空间级密钥层次 + 助记词配对 → 派生主密钥 → 派生空间密钥
→ XChaCha20-Poly1305 加密内容 + 元数据密钥（共享域 Tag/UserMetadata）。
外部密码学审计是 G3 硬门禁——栈选型需审计友好。

## 决策
1. 依赖栈（均为 Rust 生态事实标准；版本待 crates.io 引入时核验）：
   - `x25519-dalek`：X25519 ECDH（密钥协商）
   - `ed25519-dalek`：Ed25519（配对挑战签名 + 设备身份）
   - `chacha20poly1305`：XChaCha20-Poly1305 AEAD（内容/元数据加密）
   - `blake3`：KDF 与哈希（已 ADR-0002 同源）
   - `argon2`：助记词 → 主密钥（Argon2id，内存硬型）
   - `zeroize`：密钥内存擦除
2. 空间级密钥层次：主密钥（助记词派生）→ 空间密钥（HKDF-blake3：
   `space_id | context = "space-key"`）→ 内容密钥（per-blob nonce 派生）。
3. 不可信设备预加密接收：接收方在密钥不可达时返回 sealed envelope 头 + 内容哈希；
   接收方上线后凭密钥开信封。共享域 tag/link 元数据密钥独立于内容密钥——
   「加密空间端侧检索」性能取舍由 §5.11 既定（v1：tag 名/链接路径明文哈希索引）。
4. 外部审计 G3 门禁：选型优先"RustCrypto 生态"——理由：① 独立审计面已覆盖
   chacha20poly1305 / x25519-dalek；② RustCrypto 维护者是 audit-ready 协作方；
   ③ RustCrypto crates 与 RustCrypto/Rust 组织统一审计节奏。

## 备选
- aws-lc-rs / ring：性能更高但依赖 C/asm，审计面更大；可作为 KDF 引擎备选
  （Argon2 在 RustCrypto 中尚不主流，aws-lc-rs 提供），决定性选型在 WP07-T01
  实施前确认（已投 P9 测试门禁——选型即规格，规格变更需独立 PR）；
- OpenSSL bindings：FFI 成本 + 平台二进制膨胀，否决；
- 手写 KDF/AEAD：零审计友好，否决（外部审计必不通过）。

## 后果
- 助记词 → 主密钥 Argon2id 默认参数（m=64MiB, t=3, p=1）——WP07 KPI 评估
  启动时延，必要时调参；
- RustCrypto 升级节奏由我们把控（cargo deny + dependabot），无外部供应链风险；
- 外部密码学审计至少 +2 季度排期——执行方案 §6.3 已明确 WP07 与 hub E2EE 同
  走同一审计门禁，WP07 完成 ≠ M2 关门（关门依赖审计报告）。

## 重新评估条件
- 外部审计发现关键算法不合规 → 评估 RustCrypto vs aws-lc-rs 切换；
- Argon2 启动时延阻塞冷启动场景 → 评估 m/t 参数降级 + 内存硬化参数固化；
- 移动端 M6 内存约束下 Argon2id 不可行 → 评估 scrypt 或 PBKDF2 兜底。