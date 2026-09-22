//! 检索引擎子模块（SPEC M4-WP02）
//!
//! 模块划分：
//! - [`bm25`]：tantivy BM25 全文索引（多字段：filename/tags/ocr_text/transcript_text）
//! - [`vector`]：usearch HNSW 向量索引（稠密向量：文本 BGE-M3 / 图像 CLIP）
//! - [`hybrid`]：混合检索融合层（BM25 + 向量 RRF 融合 + bge-reranker 精排）
//! - [`writer`]：索引写入器（管线完成后异步 upsert + rebuild）
//! - [`engine`]：顶层检索引擎（IndexEngine + IndexEngineConfig）

pub mod bm25;
pub mod engine;
pub mod hybrid;
pub mod vector;
pub mod writer;
