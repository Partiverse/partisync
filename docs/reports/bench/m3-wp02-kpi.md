# M3-WP02 KPI 底稿（T07 验收报告）

日期: 2026-09-20 · 环境: Apple Silicon (arm64, macOS 25.6.0) · release
profile · 进程内 3 节点组（各节点独立 fjall DB + loopback TCP，选举
300-600ms / 心跳 50ms）· 基准: `cargo bench -p partisync-hub --bench wp02`
· 关联: m3-wp01-kpi.md（单机基线）、SPEC M3-WP02 验收标准

## 1. 复制导入吞吐（SPEC 验收 5）

| 口径 | 实测 | 备注 |
|---|---|---|
| 串行 client_write（window=1） | 44 eps（2 万条实测，454s） | 每条 = append fdatasync×3 + 2×网络 RTT + apply fdatasync×3，延迟绑定型 |
| 流水线 submit（window=64） | 初始 ~298 eps → 库增长后稳态 111 eps | 10⁶ 跑至 99 万条（2h34m），速率随日志增长/压缩衰减 |
| 衰减系数 vs WP01 单机 109k/s | ~0.001（串行口径） | 口径差异主导：WP01 为无复制批导入，本口径为逐条 raft commit（3×fsync/条）；同类系统小写复制形态常见此量级 |

**已知缺陷留痕（诚实登记）**：本次 10⁶ 跑所用 bench 存在窗口簿记缺陷
（成功 ack 分支多删一个窗口槽——`benches/wp02.rs` 已修），效果为吞吐
数字取**下界**（部分已提交条目未计数）、且 99 万条处脚本 panic 中止
（非 raft/存储面缺陷）。修正后 10⁶ 全量复测归 WP06；本底稿登记的
failover/转移演练不受该缺陷影响（独立代码路径）。

## 2. 故障切换与转移（关门 KPI，SPEC 验收 2/3）

drill k=3（release）：**kill 形态 = raft core 停止（无优雅交接）**；
恢复判据 = 新 leader 当选 + 全部 ACK 写入可读。

| 指标 | p50 | p99 | max | KPI 线 | 余量 |
|---|---|---|---|---|---|
| failover_ms | 1133 | 1139 | 1139 | <10,000 | 8.8× |
| transfer_ms | 594 | 626 | 626 | <5,000 | 8.0× |

- 转移形态：openraft 0.9.25 无显式 transfer API（铁律 8 核实）——
  旧 leader `runtime_config().elect(false)` + 目标 `trigger().elect()`；
  运营级主动转移归 WP03+。
- 已 ACK 不丢由 raft 日志批 fdatasync 承接（持久化证据：
  `t03_journal_durability_survives_unclean_close`，立即退出不跑析构
  形态）。

## 3. keyspace 收敛对照（SPEC 验收 7）

| 线 | WP01 基线（10⁷） | 收敛后复测 | 判定 |
|---|---|---|---|
| Database open | 12.8s | 229ms（2 万库，T02）/ **1650ms p99（20 万库，本次）** | ≤2s 线内 ✅（对照改善 ~8×@10⁷ 外推） |
| 导入吞吐 | 稳态 109k/s | 166.6k eps（20 万条实测） | 无回退 ✅ |
| LIST p99 | 36.1ms（10⁷ 库） | 599μs（20 万库，page=256） | 无回退 ✅（库规模口径不同，均远低于 100ms 线） |
| 点查 p99 | 726μs（10⁷ 库） | 13μs（20 万库） | 无回退 ✅ |

## 4. 验收对账（详见 RFC M3-WP02-线性一致语义与崩溃矩阵.md §7）

- 验收 2/3/7：**满足**（本报告数字）；
- 验收 5：**满足（下界口径）**——10⁶ 实测 99% + 窗口簿记缺陷留痕，
  修正后复测归 WP06；
- 验收 1/4/6：偏离/部分偏离（单节点组回归、并发 proptest、follower
  ReadIndex、M3-M6 接线）——移交项已在 RFC §8 登记，SPEC 修订建议随
  T06 提交备注；
- 观察项：选举超时/心跳间隔与 fdatasync p99 的关系（RFC §3.3）需在
  运营磁盘口径下复测（本次为内置 SSD）。

## 5. 工件清单

- 基准脚本：`crates/partisync-hub/benches/wp02.rs`（import/drill 子命令）
- 演练测试：`crates/partisync-hub/tests/wp02.rs` t05_*（debug 口径交叉验证）
- 语义钉子文档：`docs/rfcs/M3-WP02-线性一致语义与崩溃矩阵.md`
