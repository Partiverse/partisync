# M3-WP03 KPI 底稿（T08 验收报告）

日期: 2026-09-21 · 环境: Apple Silicon (arm64, macOS 25.6.0) · debug 构建
（对账口径）+ release（对账吞吐沿用 m3-wp02-kpi 口径注记）· 基准/测试:
`tests/wp03.rs`（18 测）、`benches/wp02.rs` · 关联: m3-wp02-kpi.md、
RFC M3-WP02 §7/§8（移交项闭环）、SPEC M3-WP03

## 1. 验收对账（SPEC M3-WP03 验收标准）

| 验收 | 状态 | 证据 |
|---|---|---|
| Hub 门面复制语义（单节点组回归） | **满足** | wp03.rs 18/18（T02 语义镜像 9 + T03 注册表 4 + T05 鉴权 2 + T07 矩阵 2 + T06 e2e 1）；WP01 直连 19/19 存档回归；WP02 21/21 |
| 角色强制 | **满足** | t03_role_matrix（read/write/admin × owner/editor/viewer/非成员）+ 随机 30 步 × 参考模型（Forbidden 无副作用逐核验）；SM 侧 Owner 强制 + 最后 Owner 防锁定 |
| D2 闭环 | **满足** | create_space CSPRNG 盐持久化 + 重开可读 + argon2_with_salt 派生一致（M2 债务表 D2 清偿） |
| 对账收敛 | **满足** | t06 e2e：设备真实写 → 全量修复收敛 → 影子叶集==设备叶集 → 增量 delta → 水位快路径；M2-WP03 全测原样过（LeafSource 泛化回归线） |
| 分裂×复制矩阵 M3-M6 | **部分满足** | M1/M2 组拓扑形态 + M3 kill-during-split → 重开恢复（t07 两测）；M4-M6 保持规格态（新分区组接线归 WP04+，见下） |
| 对账吞吐口径 | **初测满足** | t06 6 叶全量+增量对账秒级内收敛（debug）；10⁷ 差异 <5min 为 M3 DoD 预演——大规模压测归 WP06（规格登记） |
| 回归门禁 | **满足** | fmt/clippy(-D warnings)/workspace test 全绿；deny 全绿（+partisync-sync/partisync-graph 路径依赖、ed25519-dalek "3" 同版本，零新增白名单） |

## 2. 实测记录

- **分裂×复制（组拓扑）**：阈值 50 写 600 条 → apply 侧多轮确定性分裂
  （分区数>1、无 splitting 残留）+ 全量 keyset 跨分区有序完整（t07_m1m2）。
  分裂元数据批提交后 kill（failpoint 命中于组内 apply 线程 → core fatal
  = leader 死亡形态）→ 重开恢复路径收尾 splitting → 50 键集完整 +
  服务恢复（t07_m3）。
- **对账端到端**：三轮会话（全量修复 → 增量 delta → 水位快路径
  fast_path=true 零修复）——`t06_device_to_hub_reconcile_e2e`。
- **鉴权**：ed25519 签名信封授权矩阵全过；篡改/重放/回退/伪造身份
  全拒绝（t05 两测）。

## 3. 已知边界（移交 WP04+）

- 新分区组 bootstrap / 控制面断点续写 / 双读合并视图（RFC M4-M6）
  —— 分裂×复制显式日志接线随 WP04 数据面落地；
- hub 影子为镜像语义（LWW 覆盖）——P11 冲突决胜在设备侧；
- follower 线性一致读 / 并发写单调 proptest（RFC F1 移交延续）；
- nonce 防重放进程内追踪——持久化防重放归 T06' 会话层。

## 4. 移交项闭环对照（RFC M3-WP02 §8）

| RFC §8 移交 | WP03 闭环 |
|---|---|
| Hub 门面接线 raft（契约 2） | ✅ T02（单节点组） |
| 单节点组拓扑回归 | ✅ wp03 镜像 18 测 |
| 业务平面经 raft apply 接线 | ✅ T02（entry/children = SM 业务节） |
| 组注册表持久化 | ✅ T03（pid=0 组 + r-space） |
| 请求 id 幂等 | ◐ rename no-op+标志位；通用请求 id 归 WP04 |
| follower 线性读 / 并发 proptest | ◐ 移交 WP04+（RFC F1） |
| 分裂×复制 M3-M6 | ✅ M1/M2/M3；M4-M6 规格态→WP04 |
| 10⁶ 修正后复测 | ◐ bench 已修；全量复测归 WP06 |
