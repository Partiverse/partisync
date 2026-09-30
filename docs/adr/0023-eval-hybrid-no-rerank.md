# ADR-0023: EvalRunner 接入 hybrid_no_rerank——BM25 + 向量 RRF 融合

版本: 0.2 · 状态: **接受**（2026-10-01 状态头补登记 [M8-D1-T01]：决策自
M6-D67-T03 EvalRunner::run_hybrid_no_rerank 实装即生效，LCSTS 三档
对比数字入仓；本修订仅回填台账，不改决策内容）· 关联: M6-D67 §D7
（三档对比， bm25_only 已实装见 T02）、M4-WP02（混合检索框架已实装）、
ADR-0018（fastembed 批次）、M6-D67-T02（bm25_only 真档 Recall=0.95）
负责人: @lead · 起草日期: 2026-09-27

## 背景

M6-D67 SPEC §D7 要求三档对比：

| 模式 | T02 状态 | 本 ADR 范围 |
|------|----------|-------------|
| bm25_only | ✅ 落档 Recall=0.9500 / MRR=0.9375 / nDCG=0.9408 | (前序) |
| **hybrid_no_rerank** | ⏳ **本 ADR 落地** | 本 ADR |
| hybrid_with_rerank | ⏳ 待 T04 + jina-rerank 接入 | T04 |

hybrid_no_rerank = tantivy BM25 + usearch HNSW 向量检索 + RRF 融合
（k=60， SPEC M4-WP02 §4）， **不接** reranker（避免 fastembed `Rerank`
模型额外 ~700MB 权重下载， 与本任务「端到端真档冒烟」目的不匹配）。

前置资产（均已实装）：

- `crates/partisync-index/src/search/hybrid.rs::HybridSearch`：
  RRF 融合 + `Bm25Source` / `VectorSource` trait（line 82-117）
- `crates/partisync-index/src/search/engine.rs::IndexEngine`：
  `Bm25Index` + `VectorStore` + `HybridSearch` 门面（line 55-194）
- `crates/partisync-index/src/search/vector.rs::VectorStore`：
  usearch HNSW 持久化（`upsert_batch` + `search`）
- `crates/partisync-index/src/eval/runner.rs::EvalRunner`：
  当前仅暴露 `run_bm25_only`
- `crates/partisync-ai/src/stages/embed.rs`：
  fastembed TextEmbedding 调用样板（feature `ai-embed`）

依赖 (`Cargo.toml` workspace pins)： `fastembed = "=7.0.1"`（ADR-0018 已钉），
`usearch = "=2.26.2"`（workspace）， `tantivy = "0.26"`。 **零新增顶层依赖**。

## 决策

新增 `EvalRunner::run_hybrid_no_rerank`， 三阶段：

```
load_corpus       ──►  200 篇 markdown ──►  IndexedDoc
                              │
              ┌───────────────┴────────────────┐
              ▼                                ▼
       Bm25Index::upsert_batch          fastembed::TextEmbedding
       (text_dense ocr_field)           ::embed(200 texts, batch=64)
              │                                │
              ▼                                ▼
       Bm25 索引就绪                  Vec<512d f32> per doc
                                            │
                                            ▼
                                    VectorStore::upsert_batch
                                    (kind=TextDense)
                                            │
                                            ▼
                          for each query: embed + HybridSearch
                                            │
                                            ▼
                                    PerQueryMetrics 累计
```

关键约束：

- **embedding 模型**：`EmbeddingModel::BGESmallZHV15`（512d 维度， 与
  `VectorKind::TextDense` 默认维度匹配）， 模型权重 ~95MB， hf-mirror.com
  → cas-bridge.xethub.hf.co redirect 链可达（与 LCSTS 同源， 实测
  `307 → download` 路径已验证）。
- **batch 大小**： fastembed `embed(&[...], None)` 内部已 batch 调度；
  EvalRunner 端显式按 `chunk=64` 切 corpus 控制峰值内存（200 doc 实测
  < 50MB 临时缓冲）。
- **HybridSearch 用 `search_with_vector` 接口**： 避开
  `vector_search(text)` 的「fallback empty」语义（hybrid.rs line 215-222
  engine.rs 已明文）。
- **不走 reranker**： feature `index-embed` 默认开 + `index-rerank` 默认关；
  EvalRunner 不接受 `--rerank` 开关（避免与 M6-D67-T04 重叠）。

## 备选方案

| 备选 | 评估 | 否决理由 |
|------|------|----------|
| **A. 接 hybrid_no_rerank（采纳）** | 已有 HybridSearch + VectorStore + fastembed 全栈； SPEC 三档对比硬性需求 | — |
| B. 跳过 hybrid，直接走 T04 hybrid_with_rerank | 一步到位 | reranker 权重 ~700MB + 推理 100ms+/query； 与「端到端真档冒烟」目的不匹配； 且 T03 hybrid 是 T04 的前置（无 hybrid 跑通前接 reranker 是抽象错配） |
| C. 引入 text-embedding-bge-m3（多语种，1024d） | 语义更通用 | SPEC §D7 已锁 BGESmallZHv15； M3 多语种场景非 M6 优先级； 改 vec 维度会牵连现有 VectorKind 矩阵 |
| D. 复用 `partisync-ai` `EmbedStage` | 一处管理 | `EmbedStage` 走 `StageInput` 抽象（带 mime/data 二元结构）， EvalRunner 直接喂 markdown 文本反而绕远； 重复抽象不值 |

## 后果

正面：

- LCSTS 真档数字三档对齐 SPEC §D7 验收对账的「三档对比」承诺
- `EvalRunner::run_hybrid_no_rerank` 复用现有 `IndexEngine` 路径， 零新增
  架构层组件
- 端到端冒烟覆盖 `fastembed::TextEmbedding::try_new` + 批量 embed +
  HNSW 写 + RRF 读全链路

负面 / 放弃：

- 200 doc × 512d float32 = ~400KB HNSW 写， 但 `bm25_only` 跑过 EvalRunner
  无此开销， 评估速度变慢（embedding 200 条 ≈ 5s @ CPU BGE-small-zh）
- feature `index-embed` 默认开 → CI 编译时间小幅上升（fastembed 全栈链接）
- EvalRunner 现在接受 `--mode` 多分支， 错误信息要分别提示「bm25_only
  / hybrid_no_rerank / hybrid_with_rerank 三档」， 加 `match` 分支维护面
- `vector_search(text)` fallback 路径保留但 EvalRunner 不用； 后续 T04 接
  reranker 时若想用 text→vector 自动 embed， 需在该路径上串 fastembed

无 deny/advisory 影响（fastembed 已在 ADR-0018 通过审查， 无新 advisory）。

## 重新评估条件

以下任一信号出现必须复议本 ADR 并可能再起扩展：

- SPEC §D7 三档需求变更（如新增 hybrid_with_colbert 等）
- fastembed BGE 模型权重更新（新版签名校验失败）
- `VectorKind::TextDense` 维度调整 → 牵连 RRF 候选维度一致性
- LCSTS 实测显示 hybrid 比 bm25_only **没有** 显著提升（Recall 差异 < 0.05）：
  说明检索相关性已饱和， 三档对比意义减弱， 可降低 T04 优先级