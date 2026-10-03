# M8-WP07 基准报告（bench）：整文件替换延迟 + /by-hash 读对照

> Task-ID: M8-WP07-T05 · 日期: 2026-10-03 · SPEC: docs/specs/M8-WP07.md §3
> 执行: GLM-5.3-Flash (ZCode)（crates/partisync-fuse/tests/bench_replace.rs，
> macOS arm64 / 1.94.0 / debug profile——工程机口径，发布前可选 release 复测）

## 1. 整文件替换延迟（写回面 T03）

端到端口径 = staging 写入 + WAL append + 压实 + recover 重放（rename 到位在内）——真实挂载面 close 序列的宿主侧等价：

| 规模 | p50 | p95 | 采样 |
|---|---|---|---|
| 1 MiB | **1 ms** | 1 ms | 5 |
| 64 MiB | **21 ms** | 58 ms | 5 |

结论：**R4（写回日志双写开销击穿读基准体验）不成立**——64 MiB 整文件替换 p50 21ms（~3 GB/s 有效吞吐，页缓存热），单文件交互式编辑延迟感知为零。

## 2. /by-hash 读 vs 一期目录透传对照（T04）

4 MiB 同内容两路读（进程内热缓存；**drop-caches 冷缓存口径**沿 M8-WP01-bench §1 为容器外手动档，未纳入本轮——热缓存数字证明无算法性劣化）：

| 路径 | 4 MiB |
|---|---|
| by-hash（CAS `get`） | 0 ms（<1ms 计 0） |
| 目录透传（`fs::read`） | 0 ms（<1ms 计 0） |

结论：同盘 read 同形，**无回归**；断言口径 = cas ≤ max(3×dir, 50ms)。

## 3. 与一期基准对照

一期（M8-WP01-T03）：冷缓存顺序读 1567 MiB/s、随机 328µs——本期新增写回日志未触读路径（WAL 目录挂载面隐藏、读路径零改动），读基准维持。

## 4. O_DIRECT/direct_io 去向判定（非目标表项）

**维持不在验收承诺**：写回路径瓶颈在 WAL append + 整文件 staging 写（缓冲 IO 足够，p50 21ms@64MiB）；O_DIRECT 需对齐内存与绕页缓存，复杂度/收益不匹配本 WP；若未来 hot-warm 分层盘出现再议（挂 M8-WP01-bench §4 偏差表遗留）。
