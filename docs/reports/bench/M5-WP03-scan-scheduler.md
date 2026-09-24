# M5-WP03 基准与验收报告 —— 分布式扫描调度器（前缀分片并行 LIST + 断点恢复）

任务: M5-WP03-T06 · 日期: 2026-09-24 · 环境: darwin arm64（debug 构建，
fake source 延迟模型 + 本地 fs 真实树）· 规格:
[specs/M5-WP03.md](../../specs/M5-WP03.md)

## 1. 交付总览

| 任务 | 主题 | Commit |
|---|---|---|
| T01 | 规格与任务卡落档 | `f3ae024` |
| T02 | 分片模型与清单源（`ListSource`/`EntrySink` + Provider 接线） | `42e0a7b` |
| T03 | 调度器核心（worker 池 + BFS 分片自取 + 失败语义） | `29df932` |
| T04 | 断点恢复（`ScanJournal`/`FileJournal` + failpoint 崩溃矩阵） | `f19a91c` |
| T05 | CLI `scan-plan` 子命令（dry-run + `--journal` 续扫） | `3eadfdc` |
| T06 | 本报告 + 收官 | 本提交 |

## 2. 基准结果（`t06_bench_worker_scaling_and_fs_throughput`）

### A) 延迟模型（60 分片 × 20ms 模拟 LIST RTT——网络后端代理口径）

| workers | 实测 | 加速比 |
|---|---|---|
| 1 | 1.363 s | 1.00× |
| 2 | 0.754 s | 1.81× |
| **4** | **0.524 s** | **2.60×**（验收线 ≥2.5× ✅） |
| 8 | 0.570 s | 2.39×（无增益） |

- 理论下限 4 workers = 340ms；实测 524ms 的差值来自 50ms 终止轮询 tick 的
  尾相延迟 + debug 构建。**8 workers 无增益**：尾部排队 + tick 协调开销
  主导——生产建议 `concurrency ≤ 4`（网络后端 LIST 并发受远端 QPS 限制，
  4 即常见甜点）。
- 微优化后续卡：semaphore 许可队列取代 tick（尾延迟 ≤50ms → ~0）；规模
  上去后（10⁵ 分片）再评估。

### B) 真实本地 fs（60 目录 × 100 文件 = 6000 项）

| workers | 实测 | 吞吐 |
|---|---|---|
| 1 | 192 ms | ≈31,300 项/s |
| 4 | 243 ms | ≈24,700 项/s（无增益） |

**结论**：本地 fs 元数据是单点，并行无益（1 worker 已 31k 项/s）——并行
LIST 的价值目标自始就是**网络对象存储**（S3 等，RTT/QPS 绑定，调研方案
§4.1 云 LIST 数学），延迟模型 A 即其代理口径。fs 后端走单 worker 即可。

### C) 断点恢复正确性（failpoint 崩溃矩阵，非时序敏感）

| 断言 | 测试 | 结果 |
|---|---|---|
| 崩溃后 done 分片零重扫 | `t04_crash_resume`（source 调用计数：根 0 次/待扫 1 次） | ✅ |
| failed 分片重入队补扫 | `t04_failed_shard_requeued`（二轮 failed 清零、全集一致） | ✅ |
| 全 done 账本幂等返回 | `t04_resume_fully_done`（空前沿 <2s 立即返回） | ✅ |
| 账本原子性与并发安全 | `t04_file_journal_roundtrip` + 唯一 tmp 名（并发 rename 竞态修复） | ✅ |
| 半写损坏报错不 panic | `t04_corrupt_journal`（删除账本即可全新恢复） | ✅ |

CLI 实测（fs 树 15 项）：首轮 163ms 完成（4 分片）；同账本二轮
**0 新增条目、1.5ms** 返回——续扫幂等 DoD ✅。

## 3. 验收标准映射（SPEC §验收）

| 验收项 | 证据 | 结果 |
|---|---|---|
| 并行加速 ≥2.5× 且条目集一致 | §2-A（2.60×）+ `t03_parallel_speedup_and_set_equality` | ✅ |
| BFS 分裂无饥饿 + 巨型分片告警 | `t03_warning_on_huge_flat_shard` + 全树收敛测试 | ✅ |
| 失败隔离 / fail-fast | `t03_source_fail_isolation_and_retry` / `t03_sink_fail_fast_stops_pool` | ✅ |
| 断点恢复矩阵 | §2-C 五项 | ✅ |
| Provider 接线（fs tmpdir） | `t02_provider_list_dir_*`（3 测） | ✅ |
| CLI scan-plan + 续扫幂等 | §2-C 末行实测 | ✅ |
| 回归全绿 | fmt/clippy/test 全绿；M0–M4 + M5-WP01/02 套件原样通过（CI） | ✅ |
| 基准登记 | 本报告 | ✅ |

测试合计：`m5_wp03` 12 通过（+1 ignored 基准）；sync crate 全量绿。

## 4. T04→T05 期间缺陷修复留痕（任务卡外文件）

- **空恢复前沿悬挂**：全 done 账本续扫时 pending 误记 1，worker 永等
  tick——修正为 `pending = queue.len()`，补 `t04_resume_fully_done` 回归
  （随 T05 提交）。
- **协调协议重构**（T04 内）：`Notify::notify_waiters` 在 waiter 注册间隙
  丢通知导致挂死；改为 mpsc + pending 计数 + 50ms tick 兜底重查（正确性
  不依赖时序）；worker panic 经监控者任务即时置 abort，杜绝顺序 join
  悬挂窗口。
- **FileJournal 并发竞态**：共享 `.tmp` 名被对手 rename 抽走 → 唯一序号
  tmp 名。

## 5. 后续卡建议（不阻塞收官）

1. **graph 写入面 sink 化**：`index_provider` 并行化（分片账本取代字典序
   路径划界）——扫描收益直接落到远端索引入库。
2. **range 分片**：单层扁平巨目录（10⁵+ keys）按字典序 range 再分裂
   （OpenDAL start-after 语义），触发条件 = 告警回调频发。
3. **跨 hub 扫描委派**：M5-WP02 路由视图 + 联邦协议承载工单分发（需先定
   hub 间工作单语义与信用限流）。
4. **semaphore 协调**：tick 尾延迟消除（§2-A 微优化）。
