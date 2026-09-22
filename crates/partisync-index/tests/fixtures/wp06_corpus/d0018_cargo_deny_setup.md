title: cargo deny 配置实战
filename: cargo_deny_setup.md
tags: [rust, cargo-deny, supply-chain, security, dependency]
updated_ns: 1704412800000000000

# cargo deny 配置实战

## 四段检查

1. **advisories**：RUSTSEC 数据库漏洞
2. **bans**：版本/重复/许可冲突
3. **licenses**：许可证白名单
4. **sources**：crate 来源仓库

## deny.toml 关键字段

```toml
[advisories]
db-path = "~/.cargo/advisory-db"
ignore = [
    "RUSTSEC-2023-0071",  # rsa Marvin：仅公钥验证路径不可达
]

[licenses]
allow = [
    "MIT", "Apache-2.0", "BSD-3-Clause", "ISC",
    "Unicode-DFS-2016", "Zlib", "MPL-2.0", "CC0-1.0",
]

[bans]
multiple-versions = "warn"
wildcard = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "warn"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
```

## 坑位

- `cargo deny check X | tail` 会吞退出码——必须 `> file 2>&1; echo $?`
- GitHub 不可达时 `cargo deny --offline check advisories` 用本地缓存 DB
- `--offline` 是全局 flag 不是 check 子 flag
- ban 触发的 version missing 错误——path 依赖必须加 `version = "0.1.0"`

## PartiSync 现状

deny ignore 共 5 条（ADR-0014×1 + ADR-0016×3 + ADR-0020×1）；
ci-advisor 跑全部四段，每段失败即红线。

## 调试

```bash
# 全部
cargo deny check

# 单段
cargo deny check advisories
cargo deny check bans
cargo deny check licenses
cargo deny check sources

# 解释某条
cargo deny check advisories --filter RUSTSEC-2024-0001
```