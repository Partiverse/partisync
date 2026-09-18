# M0 KPI 达标报告（10⁶ 文件索引）

- 日期：2026-09-18 · 机器：Apple Silicon（M 系列）笔记本口径（执行方案 §6.1 DoD）
- 夹具：`gen-fixture --files 1000000 --dup-rate 0.35` → 1,000,000 文件 / 1,555 目录 / 33.2 GB
- 命令：`time partisync index /tmp/partisync-m0-kpi --db /tmp/kpi.db --cas /tmp/kpi.cas`（fresh db+cas）

## 三轮实测（优化全程留痕）

| 轮次 | 配置 | real | 结论 |
|---|---|---|---|
| 基线 | WAL + synchronous=FULL（默认）、每文件 4-5 条独立语句 | **28m33s** | ❌ 差 2.9× |
| 中间 | + synchronous=NORMAL、unchanged 检查惰性化 | 21m28s | ❌ 差 2.1× |
| **终版** | + **批处理：200 文件/批并发哈希（JoinSet）+ 单事务批量插入（ON CONFLICT RETURNING）** | **7m55.6s** | ✅ **达标（余量 17%）** |

吞吐：基线 ~580 files/s → 终版 ~2,110 files/s（**3.6×**）。

## 优化说明（均留痕于代码注释与 SPEC）

1. **synchronous=NORMAL（WAL）**：提交不再逐条 fsync（基线约 500 万次 fsync）。
   代价：OS 断电可能丢尾部提交；进程崩溃安全（WAL 语义）。
   可接受性：索引可幂等重建 + 作业 checkpoint 重放（P8）。
2. **批事务**：200 文件/事务，摊薄提交开销；`ON CONFLICT(path) DO UPDATE RETURNING id`
   保持 upsert 语义（旧 id 保留，闭包用 RETURNING 的真实 id）。
3. **并发哈希**：批内 JoinSet 并发读文件+BLAKE3（小文件 I/O 延迟被并行隐藏）；
   ≥256KiB 文件仍走流式 `content_hash_file`。
4. **字典序缺陷（写测试前抓住）**：walkdir DFS 序 ≠ 字典序（`/a.txt < /a/b`），
   checkpoint `≤` 划界会漏文件破坏 L5 → 全局字典序处理（本报告轮次均基于此）。

## 附属验收

- find（百万库）：0.88s 含进程启动 ✓；ls：9–65ms ✓（SPEC <1s 量级）
- 去重报告：文件级节省 4.42 GB（13.3%）实时可见 ✓
  注：SPEC 预估「dup_rate 0.35 ⇒ 节省 ≈30%±10%」按文件数占比外推字节占比，
  实际字节占比 13.3%（重复采样池的尺寸分布所致）——**SPEC 估计口径错误，实测修正**，
  KPI 核心（报告可见且数学正确）满足
- 变异例行门禁：core+cas 142 变异体 117 caught = **82.4%**（≥60% ✓；miss 均为已归档等价变异）

## 遗留（登记）

- 索引器全局排序内存峰值（1M 路径 ~100MB 量级）：流式外排归 M1；
- fmt_bytes 以 1024 进度标注 "MB"（应为 MiB）：显示瑕疵，M1 随 --json 输出统一。
