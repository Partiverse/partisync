# ADR-0013: Hub 分片复制以 openraft 0.9 为共识底座

状态: 已接受 · 日期: 2026-09-20 · 决策人: @lead（AI 代理起草，用户已签核 2026-09-20）
关联: SPEC M3-WP02、M3-WP00（依赖 ADR 清单 / 关门 KPI）、ADR-0011（fjall 底座）、
调研方案 §5.6/§6 选型表

## 背景

M3-WP02 把 WP01 的单写者元数据平面升级为复制平面：每 range 分片一组
Raft（M3-WP00 工作包图）、领导者转移/选举演练、故障切换 <10s（关门 KPI）、
单条目线性一致语义文档（钉子级双人全审）。共识引擎选型在本 ADR 落定。

进场时核实的事实（2026-09-20，crates.io API / 上游文档）：

- 当前稳定版 **0.9.25**（2026-07-28 发布；0.10 线仍在 alpha，最新
  0.10.0-alpha.34）；
- 许可证 **MIT OR Apache-2.0**——均在 deny.toml 白名单内；
- edition 2021，未声明 MSRV（rust_version: null），工具链 1.94.0（ADR-0003）
  无冲突；serde 为唯一可选传递依赖面；
- 存储面为全插拔设计：`RaftLogStorage` + `RaftStateMachine`（storage-v2 模型）
  + `RaftNetwork`——与 fjall 单库多 keyspace（ADR-0011）直接匹配：每 raft 组
  的日志与状态机各映射到组内 keyspace；
- 0.10 alpha 不取（「无聊依赖」纪律：预发布线不做基座）。

## 决策

1. `partisync-hub` 依赖 `openraft = "0.9"`（0.9.25 由 Cargo.lock 锁定；启用
   `serde` 特性供 RPC 信封序列化；`storage-v2` 特性进场时按该版本文档核实
   开关语义后启用——log/状态机分离适配是本 ADR 的前提假设）。
2. 存储适配：每 raft 组 = 同一 fjall Database 内的独立 keyspace 组
   （`r-{pid}-log` / `r-{pid}-sm` / 组元数据），不在进程外再引入存储引擎；
   WP01 keyspace 收敛裁定（SPEC M3-WP02 裁定 3）先行落地，控制 keyspace 总量。
3. openraft 仅作共识/复制引擎：不承载任何密码学语义，路由哈希纪律
   （blake3 一次性哈希、禁 derive_key，ADR-0011 决策 3）不变。

## 备选

- **raft-rs（tikv）**：仅共识核心，membership/流水线/快照均需自研外围——
  与「无聊依赖买行为、不买行数」相悖，否决；
- **自研 Raft**：选举/日志/快照/成员变更的正确性时间盒内不可行，否决；
- **etcd-inspired 全家桶（如 raft 换 etcd-client 外置）**：引入外部进程与
  运维面，与嵌入式单进程模型（WP01 裁定 5）冲突，否决。

## 后果

- 传递依赖的许可证与 bans 由 T02 进场 cargo deny 复核（预期零新增白名单
  条目；若出现与 fjall 进场同类的传递许可，按 ADR-0011 先例随本 ADR 修订）；
- openraft 0.9 的方法级 API（线性一致读入口、成员变更入口等）以进场时
  该版本文档/源码核实为准，SPEC 不预写未经核实的签名（铁律 8）；
- 每组新增 log/sm keyspace 的资源开销叠加 WP01 既有 256 keyspace——
  keyspace 收敛（SPEC 裁定 3）成为 WP02-T02 的进场前置项；
- 0.9.x 为维护线，升级跟随 dependabot；0.10 稳定后再评（独立 ADR 修订）。
