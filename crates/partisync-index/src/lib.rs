//! 检索引擎：tantivy 全文 + usearch 向量 + sidecar 管线调度（SPEC M4-WP02）
//!
//! 当前内容（M4-WP02）：
//! - [`search`]：全文检索（tantivy BM25）+ 向量检索（usearch HNSW）+ 混合融合层（RRF + bge-reranker）
//!
//! 架构（SPEC M4-WP02 §1）：
//! - BM25 索引：filename / tags / ocr_text / transcript_text 多字段
//! - 向量索引：文本 BGE-M3 768d + 图像 CLIP 512d，usearch HNSW
//! - 融合：BM25 + 向量 RRF 融合（k=60）+ 可选 bge-reranker 精排
//! - 索引写入：管线完成后异步 upsert，支持 rebuild（全量重读 sidecar_items）

pub mod search;

pub use search::{
    bm25::{Bm25Hit, Bm25Query, Bm25Result, IndexedDoc},
    hybrid::{HybridHit, HybridQuery, HybridResult, HybridSearch, HybridVectorKind, SearchFilters},
    vector::{VectorHit, VectorKind, VectorStore},
    writer::{IndexError, IndexWriter, RebuildReport},
};
