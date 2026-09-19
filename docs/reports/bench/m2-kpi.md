# M2 关门 KPI 基准：10⁵ 文件差异同步收敛

- 日期：2026-09-19 · 主机：Apple Silicon（darwin 25.6.0 arm64，本地 SQLite WAL）
- 复现：`PARTISYNC_KPI_FILES=100000 PARTISYNC_KPI_DELTA=1000 cargo test -p partisync-sync --test m2_kpi -- --ignored --nocapture`
- 口径（SPEC M2-WP00 §关门 KPI「10⁵ 文件差异同步收敛 <5min（LAN）」）：
  两节点 10⁵ 文件全量首同步（搭建，不计入 KPI）→ A 端变更 1%（1000 文件删+重写）
  → **bisync 差异同步计时**至收敛。

## 结果

| 阶段 | 耗时 | 说明 |
|---|---|---|
| 搭建：10⁵ 文件落库 + 捕获 + 全量首 push | 154.7s | 不计入 KPI |
| 变更注入（1000 删+1000 写 + 捕获） | 1.6s | 不计入 KPI |
| **差异同步 bisync（KPI）** | **14.1s** | **阈值 300s，余量 ≈21×** |

- bisync 统计：rounds=2（不动点退出）；pushed=2000（精确 = 变更集，无放大）；
  pulled=0（B 无自有变更）。
- 收敛校验：变更集抽样 content 等值通过；双方 oplog 清空（ACK 裁剪到位）。

## 观察与登记

1. **零放大**：差异同步流量 = 变更集本身（2000 行）。基准初版实测曾出现 7× 乒乓放大
   ——根因是捕获侧 oplog origin 退化为 "unknown"（add_file_batch 落库未盖 owner 列），
   对端回环防护永不命中。已修复（origin 一律取本机 device id）并固化为回归测试
   `batch_indexed_entries_capture_local_origin`（convergence.rs）。
2. **reconcile 快路径未在 2 节点非对称水位下生效**：bisync 后 A 侧无远端 origin 水位，
   `Seen_a(d)==Seen_b(d)` 不成立 → 慢路径 16 轮有界退出（结果收敛）。符合 SPEC P7
   健全性条件的保守语义；多对端水位对称化归 M3 hub（D3/D4 债务关联）。
3. 全量首同步 154.7s 为逐行 apply+relay+ACK 裁剪的 v1 简化路径（每行多次 SQL 往返）；
   批量化优化不在 M2 范围（增量同步 KPI 不受影响）。
