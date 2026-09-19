# ADR-0011: Hub L1 元数据引擎以 fjall 3.1 为底座

状态: 提议 · 日期: 2026-09-20 · 决策人: @lead（AI 代理起草，待人工签核）
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
2. keyspace 使用模型（SPEC M3-WP01 §风险 已留收敛余地）：256 哈希分片各一
   keyspace + range 分区组；若 T03 实测单实例多 keyspace 资源开销超预期，
   允许收敛为「共享 fjall 实例 + 分片键前缀」，行为契约不变（不影响本 ADR）。
3. fjall 仅作存储引擎：分片路由哈希仍用 blake3（项目自有代码，非 KDF 用途）；
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

- deny.toml 无需改白名单（许可证已覆盖）；`[bans] multiple-versions = "warn"`
  可能因 fjall 传递依赖出现新告警——进场提交时如实登记，不放宽阈值；
- fjall 3.x 仍在 1.0 前的快速演进带（2→3 有破坏性变更史）——升级跟随
  dependabot，破坏性升级按铁律 8 走独立 PR + 全量回归；
- 单实例多 keyspace 的句柄/内存开销（×256 分片）是 T03 必测项（SPEC 风险节）；
- 本 ADR 签核后，WP01-T03 起的实现方可携带该依赖进场。
