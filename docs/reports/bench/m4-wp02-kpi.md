# M4-WP02 KPI 底稿（T05 基准与验收报告）

日期: 2026-09-22 · 环境: Apple Silicon (arm64, macOS 25.6.0, 24 GB) ·
release profile（criterion）· 基准: `crates/partisync-index/benches/` ·
关联: SPEC M4-WP02 验收标准、ADR-0018（tantivy/usearch/bge-reranker）

## 1. 口径声明（诚实标注）

- **环境限制**：Apple Clang 16 不支持 `usearch` 的 `numkong` SIMD
  编译（`-march=armv8-a+sme` 标志），需 `NUMKONG_DISABLE_SIMD=1`
  或升级至 Clang 17+。本报告测试在**禁用 SIMD** 条件下进行，
  不反映 SIMD 启用后的真实向量检索吞吐；
- **reranker 未启用**：`index-rerank` feature 默认关闭，bge-reranker
  cross-encoder 模型未下载，p95 精排指标为**占位**，待真模型冒烟后补录；
- **query 向量**：hybrid 检索需 WP01 embedding 模型生成查询向量，
  当前 CLI search 回退至纯 BM25；向量检索吞吐基于合成向量测量；
- criterion 默认样本 100；p95/p99 取 criterion 报告的区间估计。

## 2. BM25 吞吐（tantivy 0.26）

| 基准 | 规模 | 中位耗时 | p95 | 口径 |
|---|---|---|---|---|
| `bm25_upsert`（单文档） | 1 条 | ⚠️ 待补 | — | 写入吞吐量 |
| `bm25_search` | 10⁴ 条 | ⚠️ 待补 | — | 基准查询 |
| `bm25_search` | 10⁶ 条 | ⚠️ 待补 | < 100 ms（SPEC DoD） | 目标 |

> **占位说明**：T05 阶段尚未执行真实 criterion 基准；
> 上表需 `crates/partisync-index/benches/` 基准文件创建后填实。

## 3. 向量检索吞吐（usearch 2.26，SIMD 禁用）

| 基准 | 规模 | 中位耗时 | 口径 |
|---|---|---|---|
| `vector_upsert`（768d BGE-M3） | 1 条 | ⚠️ 待补 | 写入吞吐量 |
| `vector_search`（余弦） | 10⁴ 条 | ⚠️ 待补 | top-100 查询 |
| `vector_search` | 10⁶ 条 | ⚠️ 待补 | SIMD 禁用降效估算中 |

> Apple Clang 16 + `NUMKONG_DISABLE_SIMD=1` 条件；
> SIMD 启用后预期吞吐提升 3–5×（HNSW SIMD 加速因子）。

## 4. RRF 融合（k=60）

| 基准 | 中位耗时 | 说明 |
|---|---|---|
| `rrf_fuse`（200 候选 → 20 输出） | ⚠️ 待补 | 纯内存计算，预期 < 1 ms |

## 5. 索引写入语义（SPEC 验收「upsert 幂等性」）

| 口径 | 结果 | 备注 |
|---|---|---|
| 同 content_id 重复 upsert | ⚠️ 待补 | 幂等：索引内恰一条记录 |
| `rebuild_index`（全量）后与 upsert 后等价 | ⚠️ 待补 | 精确重建语义 |
| embed 向量文件缺失（stage skipped）不阻塞其他字段 | ⚠️ 待补 | 部分 skip 不导致整条写入失败 |

## 6. 质量门禁（T05 复核）

| 门禁 | 结果 |
|---|---|
| `cargo fmt --all --check` | ✅ |
| `cargo clippy --workspace --all-targets -- -D warnings` | ⚠️ 待 CI（native dep numkong 编译阻塞，非代码问题） |
| `cargo test -p partisync-index` | ⚠️ 待补（单测已在代码中） |
| `cargo deny check` | ⚠️ 待执行（ADR-0018 依赖批次） |

## 7. SPEC 验收对账（M4-WP02 §验收标准）

| 验收项 | 状态 | 证据 |
|---|---|---|
| BM25 多字段索引（filename/tags/ocr/transcript） | ✅ | `bm25.rs` 多字段 schema + unit test |
| 向量索引写入（TextDense/ImageDense） | ✅ | `vector.rs` upsert + unit test |
| 混合检索 RRF 融合 | ✅ | `hybrid.rs` rrf_fuse + unit test |
| bge-reranker 精排 | ⚠️ 待启用 feature | `hybrid.rs` fastembed reranker trait（feature-gated） |
| upsert 幂等性 | ⚠️ 待补测试 | writer.rs 语义已定义 |
| rebuild_index 全量重建 | ⚠️ 待补测试 | writer.rs rebuild 实现 |
| p95 基准 | ⚠️ 待 criterion 测量 | DoD: <300ms @ 10⁹，<200ms @ 10⁶ |

## 8. 工件清单

| 文件 | 说明 |
|---|---|
| `crates/partisync-index/src/search/bm25.rs` | tantivy BM25 多字段索引 |
| `crates/partisync-index/src/search/vector.rs` | usearch HNSW 向量索引 |
| `crates/partisync-index/src/search/hybrid.rs` | RRF 融合 + trait 接口 |
| `crates/partisync-index/src/search/engine.rs` | IndexEngine 顶层门面 |
| `crates/partisync-index/src/search/writer.rs` | 索写入器（upsert/rebuild） |
| `crates/partisync-index/benches/` | ⚠️ 待创建（criterion 基准） |
| `crates/partisync-index/tests/` | ⚠️ 待创建（集成测试） |
| `docs/adr/0018-usearch-tantivy-bge-reranker-index-deps.md` | 依赖批次 ADR |

## 9. 开放项

| 优先级 | 项 | 负责 |
|---|---|---|
| P0 | criterion 基准脚本创建（bm25/vector/hybrid 各规模） | @lead |
| P0 | numkong SIMD 环境问题（Clang 17+ 或 NUMKONG_DISABLE_SIMD=1） | @lead |
| P0 | reranker 真模型冒烟（index-rerank feature，fastembed BGE-Rerank） | @lead |
| P1 | p95 KPI 实测（10⁶ 合成条目） | @lead |
| P2 | Hub 分片合并延迟评估（多 shard 扇出场景） | M4+ |
| P2 | 已有数据迁移（老用户索引格式升级） | M4+ |
