//! 检索引擎顶层门面（SPEC M4-WP02 §4/§5）
//!
//! [`IndexEngine`] 是用户/CLI/MCP 最外层 API：
//! - 持有 BM25 + 向量索引 + 可选 reranker
//! - 实现 [`hybrid::Bm25Source`] + [`hybrid::VectorSource`] trait
//! - 提供统一的 `hybrid_search` / `bm25_only` / `vector_only` 方法
//!
//! ## 使用方式
//! ```ignore
//! let engine = IndexEngine::open(index_dir, Default::default())?;
//! let result = engine.hybrid_search(query, Default::default()).await?;
//! ```

use std::sync::Arc;

use partisync_core::error::{PartisyError, Severity};

use super::bm25::{self, Bm25Index, Bm25Query, Bm25Result};
use super::hybrid::{Bm25Source, HybridQuery, HybridResult, HybridSearch, VectorSource};
use super::vector::{VectorHit, VectorKind, VectorStore};

/// 检索引擎配置。
#[derive(Debug, Clone)]
pub struct IndexEngineConfig {
    /// 索引根目录。
    pub index_root: std::path::PathBuf,
    /// 是否启用 reranker（需 `index-rerank` feature）。
    pub enable_reranker: bool,
    /// reranker 模型目录（feature `index-rerank` 开启时）。
    pub reranker_model_dir: Option<std::path::PathBuf>,
}

impl Default for IndexEngineConfig {
    fn default() -> Self {
        Self {
            index_root: dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".partisync")
                .join("index"),
            enable_reranker: false,
            reranker_model_dir: None,
        }
    }
}

/// 检索引擎（顶层 API，SPEC §4/§5）。
///
/// ```text
/// IndexEngine
///   ├─ bm25:    Arc<Bm25Index>
///   ├─ vector:  Arc<VectorStore>
///   ├─ hybrid:  HybridSearch
///   └─ reranker: Option<Arc<...>>  (index-rerank feature)
/// ```
pub struct IndexEngine {
    bm25: Arc<Bm25Index>,
    vector: Arc<VectorStore>,
    hybrid: HybridSearch,
}

impl IndexEngine {
    /// 打开或创建索引引擎。
    ///
    /// # Errors
    /// 索引初始化错误 → Fatal。
    pub fn open_or_create(config: IndexEngineConfig) -> Result<Self, PartisyError> {
        std::fs::create_dir_all(&config.index_root).map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("create index root: {e}").into()),
        })?;

        let bm25_path = config.index_root.join("bm25");
        let vector_path = config.index_root.join("vector");

        let bm25 = Arc::new(
            Bm25Index::open_or_create(&bm25_path).map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("open bm25: {e}").into()),
            })?,
        );
        let vector =
            Arc::new(
                VectorStore::open_or_create(&vector_path).map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("open vector: {e}").into()),
                })?,
            );

        let hybrid = if config.enable_reranker {
            #[cfg(feature = "index-rerank")]
            {
                let reranker_model_dir = config
                    .reranker_model_dir
                    .as_deref()
                    .unwrap_or_else(|| std::path::Path::new("~/.partisync/models/rerank"));
                use super::hybrid::Reranker;
                let reranker = Arc::new(
                    super::hybrid::FastembedReranker::new(reranker_model_dir).map_err(|e| {
                        PartisyError {
                            severity: Severity::Fatal,
                            source: Some(format!("init reranker: {e}").into()),
                        }
                    })? as Arc<dyn Reranker>,
                );
                HybridSearch::new(bm25.clone(), vector.clone(), reranker)
            }
            #[cfg(not(feature = "index-rerank"))]
            {
                let _ = config.enable_reranker; // silence unused warning
                HybridSearch::new_without_reranker(
                    Arc::new(bm25.clone()) as Arc<dyn Bm25Source>,
                    Arc::new(vector.clone()) as Arc<dyn VectorSource>,
                )
            }
        } else {
            HybridSearch::new_without_reranker(
                Arc::new(bm25.clone()) as Arc<dyn Bm25Source>,
                Arc::new(vector.clone()) as Arc<dyn VectorSource>,
            )
        };

        Ok(Self {
            bm25,
            vector,
            hybrid,
        })
    }

    /// 混合检索（BM25 + 向量 RRF 融合，可选 reranker）。
    ///
    /// 需要预计算查询向量（通过 embedding 模型）。如无查询向量，
    /// 使用 [`bm25_only`](Self::bm25_only) 或 [`vector_only`](Self::vector_only)。
    ///
    /// # Errors
    /// 索引查询错误 → Fatal。
    pub async fn hybrid_search(
        &self,
        query: &str,
        query_vector: &[f32],
        vector_kind: VectorKind,
        req: HybridQuery,
    ) -> Result<HybridResult, PartisyError> {
        self.hybrid
            .search_with_vector(query, query_vector, vector_kind, req)
            .await
    }

    /// 纯 BM25 检索（轻量，无向量）。
    ///
    /// # Errors
    /// BM25 查询错误 → Fatal。
    pub async fn bm25_only(&self, q: bm25::Bm25Query) -> Result<Bm25Result, PartisyError> {
        let start = std::time::Instant::now();
        let result = self.bm25.search(q)?;
        let elapsed = start.elapsed().as_millis() as u32;
        Ok(Bm25Result {
            timing_ms: elapsed,
            ..result
        })
    }

    /// 纯向量检索（给定查询向量）。
    ///
    /// # Errors
    /// 向量查询错误 → Fatal。
    pub fn vector_only(
        &self,
        query_vector: &[f32],
        kind: VectorKind,
        limit: usize,
    ) -> Result<Vec<VectorHit>, PartisyError> {
        self.vector.search(query_vector, kind, limit)
    }

    /// 获取 BM25 索引引用（用于批量 upsert）。
    #[must_use]
    pub fn bm25_index(&self) -> Arc<Bm25Index> {
        self.bm25.clone()
    }

    /// 获取向量索引引用（用于批量 upsert）。
    #[must_use]
    pub fn vector_store(&self) -> Arc<VectorStore> {
        self.vector.clone()
    }

    /// 提交 BM25 写入。
    ///
    /// # Errors
    /// commit 错误 → Fatal。
    pub fn commit(&self) -> Result<(), PartisyError> {
        self.bm25.commit()
    }
}

// ─── 实现 hybrid::Bm25Source（用于 HybridSearch）─────────────────────────────

impl Bm25Source for Arc<Bm25Index> {
    fn bm25_search(
        &self,
        q: Bm25Query,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Bm25Result, PartisyError>> + Send + '_>,
    > {
        // BM25 搜索是同步的，但 trait 要求返回 Future
        let bm25 = self.clone();
        Box::pin(async move { bm25.search(q) })
    }
}

// ─── 实现 hybrid::VectorSource（用于 HybridSearch）──────────────────────────

impl VectorSource for Arc<VectorStore> {
    /// 文字查询向量搜索（fallback：返回空结果，要求调用方提供 query 向量）。
    fn vector_search(
        &self,
        _query_text: &str,
        _kind: VectorKind,
        _limit: usize,
    ) -> Result<Vec<VectorHit>, PartisyError> {
        // 当无预计算向量时，向量搜索返回空——调用方应使用 `search_with_vector`
        Ok(vec![])
    }

    fn vector_search_precomputed(
        &self,
        query_vector: &[f32],
        kind: VectorKind,
        limit: usize,
    ) -> Result<Vec<VectorHit>, PartisyError> {
        self.search(query_vector, kind, limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bm25_only_search() {
        let dir = tempfile::tempdir().unwrap();
        let config = IndexEngineConfig {
            index_root: dir.path().to_path_buf(),
            ..Default::default()
        };
        let engine = IndexEngine::open_or_create(config).unwrap();

        // 无数据时返回空
        let result = engine
            .bm25_only(bm25::Bm25Query {
                query: "test".to_string(),
                limit: 10,
                include_transcript: true,
            })
            .await
            .unwrap();
        assert!(result.hits.is_empty());
    }

    #[tokio::test]
    async fn vector_only_empty() {
        let dir = tempfile::tempdir().unwrap();
        let config = IndexEngineConfig {
            index_root: dir.path().to_path_buf(),
            ..Default::default()
        };
        let engine = IndexEngine::open_or_create(config).unwrap();

        let vec = vec![0.0f32; 768];
        let result = engine.vector_only(&vec, VectorKind::TextDense, 5).unwrap();
        assert!(result.is_empty()); // 无数据
    }
}
