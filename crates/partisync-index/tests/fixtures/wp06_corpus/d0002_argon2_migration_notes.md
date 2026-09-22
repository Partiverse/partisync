title: Argon2id KDF 参数迁移笔记
filename: argon2_migration_notes.md
tags: [cryptography, kdf, argon2, security, rust]
updated_ns: 1704412800000000000

# Argon2id KDF 参数迁移笔记

## 背景

项目从 PBKDF2-SHA256（100k iterations）迁移到 Argon2id。原因：
PBKDF2 在 GPU 上抵抗暴力枚举能力差（ASIC 加速 100x+），Argon2id
内存硬（m=64MiB），抗 ASIC。

## 参数选择

| 参数 | 取值 | 理由 |
|---|---|---|
| m（内存） | 64 MiB | OWASP 2024 推荐 ≥19 MiB；预留 GPU 抗性 |
| t（迭代） | 3 | 平衡移动端延迟 |
| p（并行） | 1 | 单用户单设备场景 |
| 输出长度 | 32 B | 256-bit 熵足够 |
| 版本 | 0x13 | RFC 9106 标准 |

## 测试覆盖

- 派生结果长度固定 32 B
- 同输入同 salt → 同输出（确定性）
- 派生耗时 p95 <500ms（Apple M2 实测）

## 兼容性

旧 PBKDF2 派生路径保留为 `legacy_pbkdf2`，仅用于：
- 解密 2024-Q1 之前的加密空间
- 用户首次升级时一次性 re-wrap

旧路径加 `legacy-v1` 域分离标签；新路径加 `argon2-v1`。

## 决策记录

ADR-0010 留档；禁止散乱派生——所有 KDF 调用走 `crypto::derive_key` 一个入口。