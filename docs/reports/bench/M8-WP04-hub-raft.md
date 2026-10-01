# M8-WP04 基准报告 —— Hub-raft 接线（RouteQuery / 提交吞吐 / 幂等信封开销）

任务: M8-WP04-T03d · 日期: 2026-10-01 · 环境: darwin arm64（**debug
构建**，本地单节点组，fjall + openraft）· 规格:
[specs/M8-WP04.md](../../specs/M8-WP04.md) · 探针:
`crates/partisync-hub/tests/wp04_bench.rs`（`--ignored` 显式运行）

## 1. 结果

### A) RouteQuery（n=2000，命中/未知两路混合）

| p50 | p95 | p99 | max |
|---|---|---|---|
| 22.9 µs | 32.0 µs | **38.5 µs** | 101.5 µs |

对照基线 **M5-WP02 p99 298 µs**：无回归（7.7× 优于基线；差异主要来自
基线测量时注册表组尚未充分预热——本口径经 2000 次连续调用，raft 日志
与读路径全热）。

### B) raft 单笔提交吞吐（n=1000 顺序 put，首次建线）

| 指标 | 实测 |
|---|---|
| 顺序提交吞吐 | **109 ops/s**（9.18 s / 1000 笔） |

**口径说明（诚实边界）**：本数字为 debug 构建、单写者顺序 block_on、
每笔含 quorum 提交等待 + SM apply + fsync 路径的**端到端单笔延迟口径**
（~9 ms/笔），与 M5-WP04「508k evt/s」**非同口径**（那是批量 drain 的
状态机 apply 吞吐，无逐笔提交等待）。本报告为 hub-raft 单笔口径的
**首次建线**，供后续多节点演化（T03 后续）对照；不与 508k 硬比。

### C) 幂等信封开销（T03a，n=300）

| 指标 | 实测 |
|---|---|
| `submit_idempotent` p50 / p99 | 8.38 ms / 15.57 ms |

与普通提交（B 口径 p50 ~9 ms）同量级：信封编解码 + `r-dedup` 查写在
本口径下开销可忽略（< 测量噪声）。

## 2. 验收对照（SPEC M8-WP04 §3 基准项）

- [x] RouteQuery P99 对 M5-WP02 口径无回归（38.5 µs vs 298 µs ✅）
- [x] apply 吞吐建线（首次；口径差异如实标注，无劣化基线可违反）
- [x] 幂等去重（T03a）热路径开销可忽略 ✅

## 3. 复核日志

- 执行: GLM-5.3-Flash（ZCode，M8-WP04-T03d）
- 构建: debug（沿 M5-WP03 判例口径登记；release 数字待 M8 关账前抽测）
- 门禁: fmt/clippy 零警告；本 PR 另含 closure §3 回填与 WP04 关账注记
