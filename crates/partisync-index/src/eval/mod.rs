//! 检索质量评估模块（SPEC M4-WP06）
//!
//! 模块划分：
//! - [`metrics`]：IR 标准指标（Recall@K / MRR / nDCG@K）—— 自写零依赖
//! - [`runner`]：评估运行器——加载 corpus + queries + qrels，跑 BM25/hybrid，输出 JSON 报告
//!
//! ## 使用
//! ```ignore
//! let report = EvalRunner::new(...)
//!     .load_corpus_from_dir("path/to/corpus")?
//!     .load_queries("path/to/queries.jsonl")?
//!     .load_qrels("path/to/qrels.jsonl")?
//!     .run_bm25_only()?
//!     .write_report("path/to/report.json")?;
//! ```

pub mod metrics;
pub mod runner;

pub use metrics::{Metric, MetricReport, PerQueryMetrics};
pub use runner::{EvalRunner, Qrel, Query};