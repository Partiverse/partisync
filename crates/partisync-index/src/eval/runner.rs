//! 评估运行器（SPEC M4-WP06 §裁定 2）
//!
//! 加载 corpus + queries + qrels → 构建 BM25 索引 → 跑 bm25_only → 计算指标 → 输出 JSON。
//!
//! ## 三档模式
//! - `bm25_only` —— 仅 BM25（已实装）
//! - `hybrid_no_rerank` —— BM25 + 向量 RRF，无 reranker（**架构基线**：因无 embedding
//!   模型可用，向量用 fake 占位；待 embedding 接入后**重新跑**为生产档）
//! - `hybrid_with_rerank` —— BM25 + 向量 RRF + bge-reranker（feature `index-rerank` 关闭时跳过）

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use partisync_core::error::{PartisyError, Severity};

use super::metrics::{MetricReport, PerQueryMetrics};
use crate::search::bm25::{Bm25Index, Bm25Query, IndexedDoc};

/// 单条查询。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Query {
    pub id: String,
    pub text: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub r#type: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_true")]
    pub include_transcript: bool,
    #[serde(default)]
    #[allow(dead_code)]
    pub vector_kind: String,
}

fn default_limit() -> usize {
    20
}
fn default_true() -> bool {
    true
}

/// 单条 ground truth（query_id, content_id, relevance）。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Qrel {
    pub query_id: String,
    pub content_id: String,
    pub relevance: i32,
}

/// 评估运行器。
pub struct EvalRunner {
    /// 评估 K（top-K）
    k: usize,
    /// 语料 markdown 文件目录
    corpus_dir: PathBuf,
    /// corpus.tsv 路径
    corpus_tsv: PathBuf,
    /// queries.jsonl 路径
    queries_path: PathBuf,
    /// qrels.jsonl 路径
    qrels_path: PathBuf,
}

impl EvalRunner {
    /// 新建评估运行器。
    #[must_use]
    pub fn new(
        k: usize,
        corpus_dir: impl Into<PathBuf>,
        corpus_tsv: impl Into<PathBuf>,
        queries_path: impl Into<PathBuf>,
        qrels_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            k,
            corpus_dir: corpus_dir.into(),
            corpus_tsv: corpus_tsv.into(),
            queries_path: queries_path.into(),
            qrels_path: qrels_path.into(),
        }
    }

    /// 加载 corpus（TSV + markdown 主体）→ IndexedDoc 列表。
    fn load_corpus(&self) -> Result<Vec<IndexedDoc>, PartisyError> {
        let tsv_text = fs::read_to_string(&self.corpus_tsv).map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("read corpus.tsv: {e}").into()),
        })?;

        let mut docs = Vec::new();
        for (line_no, line) in tsv_text.lines().enumerate() {
            if line_no == 0 || line.trim().is_empty() {
                continue; // 跳过 header
            }
            let mut fields = line.split('\t');
            let content_id = fields.next().unwrap_or("").to_string();
            let relpath = fields.next().unwrap_or("").to_string();
            let filename = fields.next().unwrap_or("").to_string();
            let title = fields.next().unwrap_or("").to_string();
            let mime = fields.next().unwrap_or("").to_string();
            let updated_ns = fields.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            let tags_str = fields.next().unwrap_or("");
            let tags: Vec<String> = tags_str
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();

            if content_id.is_empty() {
                continue;
            }

            // 读取 markdown 主体（除 frontmatter 外）
            let md_path = self.corpus_dir.join(&relpath);
            let body = fs::read_to_string(&md_path).unwrap_or_default();
            // 把 markdown 主体作为 OCR 文本（合成库无真实 OCR）
            let ocr_text = Some(body);

            docs.push(IndexedDoc {
                content_id,
                filename,
                tags,
                ocr_text,
                transcript_text: None,
                updated_ns,
            });
            let _ = (title, mime); // 静默未用
        }

        Ok(docs)
    }

    /// 加载 queries.jsonl。
    fn load_queries(&self) -> Result<Vec<Query>, PartisyError> {
        let text = fs::read_to_string(&self.queries_path).map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("read queries: {e}").into()),
        })?;
        let mut out = Vec::new();
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let q: Query = serde_json::from_str(line).map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("parse query json: {e}").into()),
            })?;
            out.push(q);
        }
        Ok(out)
    }

    /// 加载 qrels.jsonl。
    fn load_qrels(&self) -> Result<HashMap<String, HashMap<String, i32>>, PartisyError> {
        let text = fs::read_to_string(&self.qrels_path).map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("read qrels: {e}").into()),
        })?;
        let mut map: HashMap<String, HashMap<String, i32>> = HashMap::new();
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let q: Qrel = serde_json::from_str(line).map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("parse qrel json: {e}").into()),
            })?;
            map.entry(q.query_id)
                .or_default()
                .insert(q.content_id, q.relevance);
        }
        Ok(map)
    }

    /// 跑 bm25_only 评估。
    ///
    /// # Errors
    /// 索引 / IO 错误 → Fatal。
    pub fn run_bm25_only(
        &self,
        bm25_index_path: impl AsRef<Path>,
    ) -> Result<MetricReport, PartisyError> {
        let docs = self.load_corpus()?;
        let queries = self.load_queries()?;
        let qrels = self.load_qrels()?;

        // 创建临时 BM25 索引
        let bm25 =
            Bm25Index::open_or_create(bm25_index_path.as_ref()).map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("open bm25: {e}").into()),
            })?;
        bm25.upsert_batch(docs).map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("upsert: {e}").into()),
        })?;
        bm25.commit().map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("commit: {e}").into()),
        })?;
        bm25.reload().map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("reload: {e}").into()),
        })?;

        // 跑查询
        let mut per_query = Vec::new();
        for q in &queries {
            let result = bm25
                .search(Bm25Query {
                    query: q.text.clone(),
                    limit: self.k,
                    include_transcript: q.include_transcript,
                })
                .map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("bm25 search: {e}").into()),
                })?;

            let retrieved: Vec<String> = result.hits.iter().map(|h| h.content_id.clone()).collect();
            let empty_rels = HashMap::new();
            let rels = qrels.get(&q.id).unwrap_or(&empty_rels);

            let metrics = PerQueryMetrics::compute(&retrieved, rels, self.k);

            per_query.push((q.id.clone(), metrics));
        }

        Ok(MetricReport::aggregate(
            "bm25_only".to_string(),
            self.k,
            per_query,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn end_to_end_eval() {
        let tmp = TempDir::new().unwrap();
        let corpus_dir = tmp.path().join("corpus");
        fs::create_dir_all(&corpus_dir).unwrap();

        // 写两篇 markdown 文档
        fs::write(
            corpus_dir.join("d1.md"),
            "---\ntitle: 夏威夷\nfilename: hawaii.md\ntags: [travel, hawaii]\nupdated_ns: 1\n---\n\n夏威夷度假\n",
        )
        .unwrap();
        fs::write(
            corpus_dir.join("d2.md"),
            "---\ntitle: 京都\nfilename: kyoto.md\ntags: [travel, japan]\nupdated_ns: 1\n---\n\n京都樱花\n",
        )
        .unwrap();

        // 写 TSV
        let tsv = "content_id\trelpath\tfilename\ttitle\tmime\tupdated_ns\ttags\ncid1\td1.md\thawaii.md\t夏威夷\ttext/markdown\t1\ttravel,hawaii\ncid2\td2.md\tkyoto.md\t京都\ttext/markdown\t1\ttravel,japan\n";
        let tsv_path = tmp.path().join("corpus.tsv");
        fs::write(&tsv_path, tsv).unwrap();

        // 写 queries
        let q_path = tmp.path().join("queries.jsonl");
        fs::write(
            &q_path,
            r#"{"id":"Q1","text":"夏威夷","type":"short","limit":20,"include_transcript":true,"vector_kind":"auto"}
"#,
        )
        .unwrap();

        // 写 qrels
        let qr_path = tmp.path().join("qrels.jsonl");
        fs::write(
            &qr_path,
            r#"{"query_id":"Q1","content_id":"cid1","relevance":3}
"#,
        )
        .unwrap();

        let runner = EvalRunner::new(10, corpus_dir, tsv_path, q_path, qr_path);
        let index_dir = tmp.path().join("index");
        let report = runner.run_bm25_only(&index_dir).unwrap();

        assert_eq!(report.mode, "bm25_only");
        assert_eq!(report.num_queries, 1);
        assert!(report.mean_recall_at_k > 0.0); // 至少命中 cid1
        assert!(report.mean_mrr > 0.0);
        assert!(report.mean_ndcg_at_k > 0.0);
    }
}
