# ADR-0011: Hub L1 元数据引擎以 fjall 3.1 为底座

状态: 已接受 · 日期: 2026-09-20 · 决策人: @lead（AI 代理起草，用户已签核 2026-09-20）
关联: SPEC M3-WP01、M3-WP00（依赖 ADR 清单）、调研方案 §5.6（L1 Hub 行）/§6 选型表、ADR-0002（依赖治理先例）

## 背景

M3-WP01 把 hub 元数据平面抬到 10⁹ 条目：需要 LSM 型 KV（顺序写友好、压缩可控）、
多 keyspace（SPEC 裁定 5：每分片独立 keyspace，WP02 每分片套 openraft 组）、
有序 range 扫描（children 聚簇 LIST）与 100% safe Rust（workspace forbid unsafe）。
调研方案 §6 选型表已列 fjall 为 L1 首选（rust-rocksdb 为备选），本 ADR 落实进场。

进场时核实的事实（2026-09-20，crates.io / 上游 Cargo.toml）：

- 当前稳定版 **3.1.10**（3.0 于 2026-01 发布，3.1.x 维护至 2026-08+，仓库活跃）；
- 许可证 **MIT OR Apache-2.0**——均在 deny.toml 白名单内，无需新增条目；
- **纯 Rust、100% safe**（无 C/C++ FFI），rust-version 1.90.0 ≤ 工具链 1.94.0
  （ADR-0003），无需动 rust-toolchain.toml；
- 自带多 partition（keyspace）模型与原子 WriteBatch——与 SPEC M3-WP01
  分裂协议（元数据先行原子批）直接匹配。

## 决策

1. `partisync-hub` 依赖 `fjall = "3.1"`（具体 3.1.x 由 Cargo.lock 锁定，进场后
   dependabot/cargo deny 常规跟踪）。
2. keyspace 使用模型：**单 Database + N keyspace**（fjall 顶层是 `Database`，跨
   Database 无 `OwnedWriteBatch` 原子批——WP01 分裂协议的原子批主张以此为前提）。
   256 哈希分片 + range 分区组均为同一 Database 内的 keyspace；若 T03 实测单库
   多 keyspace 资源开销超预期，允许收敛为「共享 keyspace + 分片键前缀」，
   行为契约不变（不影响本 ADR）。
3. fjall 仅作存储引擎：分片路由哈希一律 `blake3::hash`（一次性哈希，项目自有
   代码，非 KDF 用途）；**禁止 `blake3::derive_key`**（KDF 冻结面，D1）；
   引擎不承载任何密码学语义——与 D1/KDF 冻结面无关。

## 备选

- **rust-rocksdb**：久经考验、写放大调参成熟；但 C++ FFI（构建/供应链/审计面
  扩大）、unsafe 绑定与 forbid-unsafe 纪律冲突、跨平台二进制成本——调研方案
  已列为备选，v0.1 否决（KPI 不达标时可按本 ADR 重审）；
- **redb**：已在 L0 设备侧使用（同源依赖面小）；但单文件 B 树模型的顺序写吞吐
  与压缩特性不及 LSM，10⁹ 量级 + 每 keyspace 独立恢复（崩溃域隔离）需求下
  fjall 更贴 SPEC 裁定 5——保留为单机小规模嵌入场景（非本 ADR 范围）；
- **sled**：维护长期停滞（0.x 未达 1.0），不满足「无聊依赖」纪律——否决；
- **自研 LSM**：M3 时间盒内不可行，否决。

## 后果

- ~~deny.toml 无需改白名单（许可证已覆盖）~~ **进场修订（2026-09-20，签核决策的
  机械后果）**：fjall 传递依赖引入两个白名单外许可证——`varint-rs 2.2.1`（0BSD）
  与 `xxhash-rust 0.8.18`（BSL-1.0，Boost Software License）。两者均为 OSI 认可的
  宽松许可，随本 ADR 一并加入 deny.toml allow（不放宽任何其他阈值）；
- fjall 3.x 仍在 1.0 前的快速演进带（2→3 有破坏性变更史）——升级跟随
  dependabot，破坏性升级按铁律 8 走独立 PR + 全量回归；
- 单实例多 keyspace 的句柄/内存开销（×256 分片）是 T03 必测项（SPEC 风险节）；
- 本 ADR 签核后，WP01-T03 起的实现方可携带该依赖进场。

## 进场记录（T02 执行结果）

- cargo-deny 0.20.2：licenses/bans 相对进场前基线**零新增失败**（新增的
  0BSD/BSL-1.0 已按上文加入 allow）；
- **基线既有失败（与本 ADR 无关，登记待另开任务）**：① workspace 内 7 组
  path 依赖被 `wildcards = "deny"` 判罚（partisync-cli/graph/transfer/sync/
  provider/cas，疑为 cargo-deny 版本演进导致基线转红）；② `webpki-root-certs
  1.0.9`（CDLA-Permissive-2.0，reqwest dev 链）不在白名单。均不在 T02 范围，
  按「顺手修另开任务」纪律登记。
