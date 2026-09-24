# M5-WP05 基准报告 —— 真实规模性能（D5 清偿）与 10⁹–10¹² 外推

任务: M5-WP05-T02/T03 · 日期: 2026-09-25 · 环境: darwin arm64（bench
profile = optimized；语料确定性 LCG 合成）· 规格:
[specs/M5-WP05.md](../../specs/M5-WP05.md)（含 T02 修订记录）

## 1. 交付与缺陷修复

| 项 | Commit |
|---|---|
| T01 规格 | `23c10c7` |
| T02 bench 骨架 + 2×10³ 冒烟 + **usearch remove/contains 超线性缺陷修复**（`add_new` 快路径 + `reserve`） | `f2530e4` |
| T02/T03 全量基准 + 本报告 | 本提交 |

**M4-WP02 生产缺陷发现**（D5 基准的核心产出）：
`VectorStore::upsert` 每次写入先 `remove(key)` 且后续含 `contains` 检查——
usearch HNSW 的 remove/contains 代价随规模超线性（10⁴ 规模实测 ~30ms/次），
10⁶ 写入路径不可用。修复 = fresh-key 快路径 `add_new`（bulk-load 主路径）+
预分配 `reserve`；覆盖语义保留在 `upsert`。**遗留**：usearch `add` 本身
在本环境仍 ~4–16ms/条（release，768d Cos）——10⁵ 构建需 ~27min，归后续卡
查 C 层编译/探测参数（疑 SIMD 未启用）。

## 2. 检索基准（criterion 稳态口径 = 报告口径）

| 查询 | 语料 | time（criterion 95% 区间） | 验收线 | 结果 |
|---|---|---|---|---|
| **BM25 top-10** | **10⁶ 文档** | **[626, 636, 642] µs** | <100ms | ✅（150× 余量） |
| 向量 top-10（768d Cos） | 2×10⁴ | [9.13, 9.22, 9.29] ms | <100ms | ✅ |
| hybrid RRF top-10 | 2×10⁴ | [10.05, 10.26, 10.53] ms | <150ms | ✅ |

**worst-case 长查询披露**（手测 200 样本，查询 = 整篇文档文本 20–80 词，
多 term OR）：BM25 P50 87.8ms / P99 208ms；hybrid P50 103ms / P99 229ms
（含每样本 tokio Runtime 重建开销）。真实用户查询为短词序列，criterion
口径为准；长查询超 100ms 线如实登记（tantivy 多 term OR 语义，非缺陷）。

**语料构建**：10⁶ BM25 文档 + 2×10⁴ 向量 = 581s（~10min，一次性落盘复用）。

## 3. 元数据仿真（graph::Store，SQLite 口径）

| 指标 | 实测（10⁵ entries） | 对照 |
|---|---|---|
| `add_file_batch` 吞吐 | ≈ 6,644 行/s（debug 构建） | hub fjall release 109k/s（M3-WP01 10⁷）——**口径不同**：graph=SQLite 单机图谱（M2 线），hub=fjall 分片（M3 线），两者服务不同层 |
| `entry_by_path` P50 | 67.8 µs | — |
| `entry_by_path` P99 | 94.6 µs | — |

## 4. 10⁹–10¹² 外推（调研方案 §4.1 口径，线性外推 + Limits）

| 层 | 实测基点 | 10⁹ 外推 | 10¹² 外推 | 前提/Limits |
|---|---|---|---|---|
| BM25 检索延迟 | 636µs @10⁶ | ~ms 级（tantivy 分段+分片并行） | 需分片联邦（WP02 路由）| 单索引实用上限 ~10⁸；超出按 space 分片，每 hub 10⁶–10⁸ |
| 元数据写入 | 109k/s @10⁷（hub fjall） | 64× 分片并行 ≈ 满足 | 联邦聚合（非单库） | M5-WP00 §4.1 推算：10¹² = 联邦总量 |
| 向量写入 | **16ms/条（未解）** | **不可行** | — | **阻塞项**：usearch add 吞吐需 100–1000× 提升（SIMD/量化/批量）；后续卡 |
| 元数据查询 | 95µs @10⁵（SQLite） | fjall 分片口径迁移 | — | hub 层已实测（M3-WP01 LIST p99 36ms @10⁷） |

## 5. D5–D7 状态落档

| 债 | 状态 | 证据 |
|---|---|---|
| **D5** 检索吞吐基准 | **已清偿** | 本报告 §2（三表 criterion 实测 + usearch 缺陷修复 `f2530e4`） |
| D6 三推理栈真模型冒烟 | **gated: HF 外网可达** | 环境限制，不虚报 |
| D7 真实评估集 + 双人盲标 | **gated: 真实数据面接入** | M5-WP00 既定 M5+ |

## 6. 测试与回归

`m5_wp05`：1 通过（2×10³ 冒烟正确性）+ 1 ignored（10⁵ 元数据仿真，已实测）；
bench `wp05` 全量口径 WP05_FULL=1 显式跑（CI 不跑）。全仓回归 + CI 见收官提交。

## 7. 后续卡建议（不阻塞收官）

1. **usearch add 吞吐**（阻塞 10⁵+ 向量）：查 C 层编译 flags/SIMD、
   批量 add API、量化（F16/I8）；目标 ≥10k 条/s。
2. **长查询优化**：tantivy 多 term OR 上限截断（N term 限制）或 query
   预处理。
3. **D6/D7**：外网/真实数据面解锁后按 M4-report 既定路径补录。
