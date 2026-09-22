//! 混合检索融合层（SPEC M4-WP02 §4）
//!
//! 融合流程：
//! ```text
//! 查询
//!   ├─ BM25 Top-100 → candidate pool
//!   ├─ usearch Top-100 → candidate pool
//!   └─ [可选] 稀疏向量 Top-100 → candidate pool
//! RRF 融合（k=60）→ Top-20
//! → bge-reranker 精排 → Top-K
//! ```
//!
//! bge-reranker 通过 fastembed 的 `Rerank` 模型实现（`index-rerank` feature）。
//! reranker 模型文件走本地推理（不调用外部 API），feature 默认关闭（ADR-0018）。

use std::sync::Arc;

use partisync_core::error::{PartisyError, Severity};

use super::bm25::{self, Bm25Hit, Bm25Query, Bm25Result};
use super::vector::{self, VectorHit, VectorKind};

/// RRF 融合器（Reciprocal Rank Fusion）。
/// k 值参考：k=60 在大多数信息检索基准上表现稳健（SPEC §4）。
const RRF_K: f32 = 60.0;

/// 混合检索最终结果。
#[derive(Debug, Clone)]
pub struct HybridHit {
    pub content_id: String,
    /// RRF 融合分（归一化前）。
    pub rrf_score: f32,
    /// reranker 精排分（仅在启用 reranker 时有效）。
    pub rerank_score: Option<f32>,
    pub highlight: Option<String>,
}

/// 混合检索请求。
#[derive(Debug, Clone)]
pub struct HybridQuery {
    /// 自然语言查询。
    pub query: String,
    /// content_id/tag/mime_kind/date_range 过滤（待后续扩展，先留空）。
    pub filters: SearchFilters,
    /// 最大返回条数（默认 20，max 100）。
    pub limit: usize,
    /// 查询向量种类（TextDense | ImageDense | Auto）。
    pub vector_kind: HybridVectorKind,
    /// 是否包含转写文本（关闭可提升速度）。
    pub include_transcript: bool,
    /// 是否启用 bge-reranker 精排。
    pub use_reranker: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub content_ids: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub mime_kinds: Option<Vec<String>>,
}

/// 向量查询种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HybridVectorKind {
    TextDense,
    ImageDense,
    /// 自动选择（仅当 query 向量生成可用时）。
    Auto,
}

/// 混合检索响应。
#[derive(Debug, Clone)]
pub struct HybridResult {
    pub hits: Vec<HybridHit>,
    /// 粗排候选池大小（RRF 融合输入总数）。
    pub total: usize,
    pub timing_ms: u32,
    /// 下次可缓存时间提示（ms）。
    pub cache_ttl_ms: Option<u32>,
}

/// RRF 融合（SPEC §4）。
///
/// 将多个有序结果列表融合为一个排序列表。
fn rrf_fuse(
    bm25_hits: &[Bm25Hit],
    vector_hits: &[VectorHit],
) -> Vec<(String, f32, Option<String>)> {
    let mut scores: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
    let mut highlights: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for (rank, hit) in bm25_hits.iter().enumerate() {
        let rrf = 1.0 / (RRF_K + rank as f32);
        *scores.entry(hit.content_id.clone()).or_insert(0.0) += rrf;
        if let Some(ref hl) = hit.highlight {
            highlights
                .entry(hit.content_id.clone())
                .or_insert_with(|| hl.clone());
        }
    }

    for (rank, hit) in vector_hits.iter().enumerate() {
        let rrf = 1.0 / (RRF_K + rank as f32);
        *scores.entry(hit.content_id.clone()).or_insert(0.0) += rrf;
    }

    let mut sorted: Vec<_> = scores.into_iter().collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    sorted
        .into_iter()
        .map(|(cid, score)| (cid, score, highlights.remove(&cid)))
        .collect()
}

/// 混合检索引擎门面。
///
/// 持有 BM25 + 向量索引引用，负责 RRF 融合和可选精排。
pub struct HybridSearch {
    bm25: Arc<dyn Bm25Source>,
    vector: Arc<dyn VectorSource>,
    #[cfg(feature = "index-rerank")]
    reranker: Option<Arc<dyn Reranker>>,
}

#[cfg(feature = "index-rerank")]
impl HybridSearch {
    /// 新建（启用 reranker）。
    pub fn new(
        bm25: Arc<dyn Bm25Source>,
        vector: Arc<dyn VectorSource>,
        reranker: Arc<dyn Reranker>,
    ) -> Self {
        Self {
            bm25,
            vector,
            reranker: Some(reranker),
        }
    }
}

impl HybridSearch {
    /// 新建（不带 reranker）。
    pub fn new_without_reranker(bm25: Arc<dyn Bm25Source>, vector: Arc<dyn VectorSource>) -> Self {
        Self {
            bm25,
            vector,
            #[cfg(feature = "index-rerank")]
            reranker: None,
        }
    }

    /// 执行混合检索。
    ///
    /// # Errors
    /// BM25 / 向量查询错误 → Fatal。
    pub fn search(&self, req: HybridQuery) -> Result<HybridResult, PartisyError> {
        let start = std::time::Instant::now();

        let limit = req.limit.min(100).max(1);
        let bm25_limit = 100.min(limit * 5); // BM25 top-100 足够 RRF 候选

        // BM25 查询
        let bm25_result = self
            .bm25
            .bm25_search(bm25::Bm25Query {
                query: req.query.clone(),
                limit: bm25_limit,
                include_transcript: req.include_transcript,
            })
            .map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("bm25 search: {e}").into()),
            })?;

        // 向量查询
        let vector_kind = match req.vector_kind {
            HybridVectorKind::TextDense => VectorKind::TextDense,
            HybridVectorKind::ImageDense => VectorKind::ImageDense,
            HybridVectorKind::Auto => VectorKind::TextDense, // TODO: query embedding 自动选择
        };

        // 生成查询向量（暂不支持文字→向量，需要先跑 embedding，这里做占位）
        // 实际场景：由调用方通过 `search_with_query_vector` 提供 query 向量
        let vector_result = self
            .vector
            .vector_search(&req.query, vector_kind, bm25_limit)
            .map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("vector search: {e}").into()),
            })?;

        // RRF 融合
        let fused = rrf_fuse(&bm25_result.hits, &vector_result.hits);
        let total = fused.len();

        // 取 top-N 做精排
        let rerank_candidates = fused.into_iter().take(20).collect::<Vec<_>>();

        let hits = if req.use_reranker {
            #[cfg(feature = "index-rerank")]
            {
                if let Some(ref reranker) = self.reranker {
                    let reranked = reranker
                        .rerank(&req.query, &rerank_candidates, limit)
                        .map_err(|e| PartisyError {
                            severity: Severity::Fatal,
                            source: Some(format!("rerank: {e}").into()),
                        })?;
                    reranked
                } else {
                    rerank_candidates
                        .into_iter()
                        .map(|(cid, score, hl)| HybridHit {
                            content_id: cid,
                            rrf_score: score,
                            rerank_score: None,
                            highlight: hl,
                        })
                        .collect()
                }
            }
            #[cfg(not(feature = "index-rerank"))]
            {
                rerank_candidates
                    .into_iter()
                    .map(|(cid, score, hl)| HybridHit {
                        content_id: cid,
                        rrf_score: score,
                        rerank_score: None,
                        highlight: hl,
                    })
                    .collect()
            }
        } else {
            rerank_candidates
                .into_iter()
                .map(|(cid, score, hl)| HybridHit {
                    content_id: cid,
                    rrf_score: score,
                    rerank_score: None,
                    highlight: hl,
                })
                .collect()
        };

        let elapsed = start.elapsed().as_millis() as u32;
        Ok(HybridResult {
            hits,
            total,
            timing_ms: elapsed,
            cache_ttl_ms: Some(5000),
        })
    }

    /// 使用外部提供的查询向量执行混合检索（用于 embedding 模型已就绪的场景）。
    ///
    /// # Errors
    /// BM25 / 向量查询错误 → Fatal。
    pub fn search_with_vector(
        &self,
        query_text: &str,
        query_vector: &[f32],
        vector_kind: VectorKind,
        req: HybridQuery,
    ) -> Result<HybridResult, PartisyError> {
        let start = std::time::Instant::now();

        let limit = req.limit.min(100).max(1);
        let bm25_limit = 100.min(limit * 5);

        let bm25_result = self
            .bm25
            .bm25_search(bm25::Bm25Query {
                query: query_text.to_string(),
                limit: bm25_limit,
                include_transcript: req.include_transcript,
            })
            .map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("bm25 search: {e}").into()),
            })?;

        let vector_result = self
            .vector
            .vector_search_precomputed(query_vector, vector_kind, bm25_limit)
            .map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("vector search: {e}").into()),
            })?;

        let fused = rrf_fuse(&bm25_result.hits, &vector_result.hits);
        let total = fused.len();

        let rerank_candidates = fused.into_iter().take(20).collect::<Vec<_>>();
        let hits = rerank_candidates
            .into_iter()
            .map(|(cid, score, hl)| HybridHit {
                content_id: cid,
                rrf_score: score,
                rerank_score: None,
                highlight: hl,
            })
            .collect();

        let elapsed = start.elapsed().as_millis() as u32;
        Ok(HybridResult {
            hits,
            total,
            timing_ms: elapsed,
            cache_ttl_ms: Some(5000),
        })
    }
}

// ─── Trait 接口（允许注入 mock / 不同实现）────────────────────────────────

/// BM25 数据源 trait（允许注入 fake 实现供测试）。
pub trait Bm25Source: Send + Sync {
    fn bm25_search(
        &self,
        q: Bm25Query,
    ) -> Box<dyn std::future::Future<Output = Result<Bm25Result, PartisyError>> + Send + '_>;
}

/// 向量数据源 trait。
pub trait VectorSource: Send + Sync {
    /// 用查询文本搜索（fallback，当 query 向量不可用时）。
    fn vector_search(
        &self,
        query_text: &str,
        kind: VectorKind,
        limit: usize,
    ) -> Result<Vec<VectorHit>, PartisyError>;
    /// 用预计算向量搜索。
    fn vector_search_precomputed(
        &self,
        query_vector: &[f32],
        kind: VectorKind,
        limit: usize,
    ) -> Result<Vec<VectorHit>, PartisyError>;
}

/// bge-reranker trait（index-rerank feature 下可用）。
#[cfg(feature = "index-rerank")]
pub trait Reranker: Send + Sync {
    fn rerank(
        &self,
        query: &str,
        candidates: &[(String, f32, Option<String>)],
        limit: usize,
    ) -> Result<Vec<HybridHit>, PartisyError>;
}

// ─── 实现：使用 fastembed reranker（ADR-0018）────────────────────────────

#[cfg(feature = "index-rerank")]
mod fastembed_rerank {
    use super::*;

    pub struct FastembedReranker {
        model: fastembed::Rerank,
    }

    impl FastembedReranker {
        #[allow(dead_code)]
        pub fn new(model_dir: &std::path::Path) -> Result<Self, PartisyError> {
            let model =
                fastembed::Rerank::new(fastembed::RerankModel::BgeRerankBase).map_err(|e| {
                    PartisyError {
                        severity: Severity::Fatal,
                        source: Some(format!("fastembed rerank init: {e}").into()),
                    }
                })?;
            Ok(Self { model })
        }
    }

    #[cfg(feature = "index-rerank")]
    impl Reranker for FastembedReranker {
        fn rerank(
            &self,
            query: &str,
            candidates: &[(String, f32, Option<String>)],
            limit: usize,
        ) -> Result<Vec<HybridHit>, PartisyError> {
            let docs: Vec<&str> = candidates.iter().map(|c| c.0.as_str()).collect();
            let scores = self.model.rerank(query, &docs).map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("fastembed rerank: {e}").into()),
            })?;

            let mut scored: Vec<_> = candidates
                .iter()
                .zip(scores.iter())
                .map(|(c, &s)| (c.0.clone(), c.1, Some(s), c.2.clone()))
                .collect();
            scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
            Ok(scored
                .into_iter()
                .take(limit)
                .map(|(cid, rrf, rerank, hl)| HybridHit {
                    content_id: cid,
                    rrf_score: rrf,
                    rerank_score: rerank,
                    highlight: hl,
                })
                .collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct FakeBm25 {
        hits: Vec<Bm25Hit>,
    }
    impl Bm25Source for FakeBm25 {
        fn bm25_search(
            &self,
            _: Bm25Query,
        ) -> Box<dyn std::future::Future<Output = Result<Bm25Result, PartisyError>> + Send + '_>
        {
            Box::pin(async move {
                Ok(Bm25Result {
                    hits: self.hits.clone(),
                    total: self.hits.len(),
                    timing_ms: 1,
                })
            })
        }
    }

    struct FakeVector {
        hits: Vec<VectorHit>,
    }
    impl VectorSource for FakeVector {
        fn vector_search(
            &self,
            _: &str,
            _: VectorKind,
            _: usize,
        ) -> Result<Vec<VectorHit>, PartisyError> {
            Ok(self.hits.clone())
        }
        fn vector_search_precomputed(
            &self,
            _: &[f32],
            _: VectorKind,
            _: usize,
        ) -> Result<Vec<VectorHit>, PartisyError> {
            Ok(self.hits.clone())
        }
    }

    #[tokio::test]
    async fn rrf_fusion_basic() {
        let bm25 = Arc::new(FakeBm25 {
            hits: vec![
                Bm25Hit {
                    content_id: "c1".into(),
                    score: 1.5,
                    highlight: Some("hello".into()),
                },
                Bm25Hit {
                    content_id: "c2".into(),
                    score: 1.0,
                    highlight: None,
                },
            ],
        });
        let vector = Arc::new(FakeVector {
            hits: vec![
                VectorHit {
                    content_id: "c1".into(),
                    score: 0.9,
                },
                VectorHit {
                    content_id: "c3".into(),
                    score: 0.8,
                },
            ],
        });

        let engine = HybridSearch::new_without_reranker(bm25, vector);
        let result = engine
            .search(HybridQuery {
                query: "test".to_string(),
                filters: Default::default(),
                limit: 10,
                vector_kind: HybridVectorKind::TextDense,
                include_transcript: true,
                use_reranker: false,
            })
            .unwrap();

        // c1 同时在 BM25 和 vector 中，RRF 得分最高
        assert_eq!(result.hits[0].content_id, "c1");
        // c2 仅 BM25，c3 仅 vector
        assert!(result.hits.iter().any(|h| h.content_id == "c2"));
        assert!(result.hits.iter().any(|h| h.content_id == "c3"));
    }
}
