//! tantivy BM25 全文索引（SPEC M4-WP02 §2）
//!
//! 索引字段：filename / tags / ocr_text / transcript_text。
//! 分词：filename = text 标准分词；其余 = text_stemmed（降噪 OCR/转写）。
//!
//! 索引 schema：`content_id`（主键） + 四字段 + `updated_ns`（增量更新时间戳）。

use std::path::Path;
use std::sync::RwLock;

use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument};

use partisync_core::error::{PartisyError, Severity};

/// BM25 索引字段名（与 SPEC §2 一致）。
const FIELD_CONTENT_ID: &str = "content_id";
const FIELD_FILENAME: &str = "filename";
const FIELD_TAGS: &str = "tags";
const FIELD_OCR_TEXT: &str = "ocr_text";
const FIELD_TRANSCRIPT_TEXT: &str = "transcript_text";
const FIELD_UPDATED_NS: &str = "updated_ns";

/// 单条可检索文档（写入 BM25 索引前从 sidecar_items + entries 聚合）。
#[derive(Debug, Clone)]
pub struct IndexedDoc {
    pub content_id: String,
    pub filename: String,
    pub tags: Vec<String>,
    pub ocr_text: Option<String>,
    pub transcript_text: Option<String>,
    pub updated_ns: i64,
}

/// BM25 全文检索结果。
#[derive(Debug, Clone)]
pub struct Bm25Hit {
    pub content_id: String,
    pub score: f32,
    /// BM25 snippet（取 `ocr_text` 或 `transcript_text` 匹配段）。
    pub highlight: Option<String>,
}

/// BM25 查询结果。
#[derive(Debug, Clone)]
pub struct Bm25Result {
    pub hits: Vec<Bm25Hit>,
    pub total: usize,
    pub timing_ms: u32,
}

/// 查询参数。
#[derive(Debug, Clone)]
pub struct Bm25Query {
    /// 原始查询字符串。
    pub query: String,
    /// 最大返回条数。
    pub limit: usize,
    /// 是否在结果中包含转写文本（关闭可提升速度）。
    pub include_transcript: bool,
}

fn schema() -> Schema {
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field(FIELD_CONTENT_ID, STRING | STORED);
    schema_builder.add_text_field(FIELD_FILENAME, TEXT | STORED);
    schema_builder.add_text_field(FIELD_TAGS, TEXT | STORED); // keyword_array 展开为 TEXT
    schema_builder.add_text_field(FIELD_OCR_TEXT, TEXT);
    schema_builder.add_text_field(FIELD_TRANSCRIPT_TEXT, TEXT);
    schema_builder.add_i64_field(FIELD_UPDATED_NS, INDEXED | STORED);
    schema_builder.build()
}

fn field_ids(schema: &Schema) -> (Field, Field, Field, Field, Field, Field) {
    (
        schema.get_field(FIELD_CONTENT_ID).unwrap(),
        schema.get_field(FIELD_FILENAME).unwrap(),
        schema.get_field(FIELD_TAGS).unwrap(),
        schema.get_field(FIELD_OCR_TEXT).unwrap(),
        schema.get_field(FIELD_TRANSCRIPT_TEXT).unwrap(),
        schema.get_field(FIELD_UPDATED_NS).unwrap(),
    )
}

fn err(what: &str, e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> PartisyError {
    PartisyError {
        severity: Severity::Fatal,
        source: Some(what.into()),
    }
}

/// BM25 全文索引（tantivy 0.26）。
///
/// 实例持有 `Index`（不可变）和 `IndexWriter`（可变，共享写锁）。
/// 查询走独立 `IndexReader`。
pub struct Bm25Index {
    index: Index,
    writer: RwLock<IndexWriter>,
    reader: IndexReader,
    parser: QueryParser,
    schema: Schema,
}

impl Bm25Index {
    /// 打开或新建索引目录。
    ///
    /// # Errors
    /// IO / tantivy 初始化错误 → Fatal。
    pub fn open_or_create(path: impl AsRef<Path>) -> Result<Self, PartisyError> {
        let path = path.as_ref();
        let schema = schema();

        let index = if path.exists() {
            Index::open_in_dir(path).map_err(|e| err("open tantivy index", e))?
        } else {
            std::fs::create_dir_all(path).map_err(|e| {
                err(
                    "create tantivy index dir",
                    std::io::Error::new(std::io::ErrorKind::Other, e),
                )
            })?;
            Index::create_in_dir(path, schema.clone())
                .map_err(|e| err("create tantivy index", e))?
        };

        let writer = index
            .writer(50_000_000) // 50 MB heap
            .map_err(|e| err("open tantivy writer", e))?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| err("open tantivy reader", e))?;

        let (id_field, fn_field, tags_field, ocr_field, tx_field, _upd_field) = field_ids(&schema);
        let parser = QueryParser::for_index(&index, [fn_field, tags_field, ocr_field, tx_field]);

        Ok(Self {
            index,
            writer: RwLock::new(writer),
            reader,
            parser,
            schema,
        })
    }

    /// 写入/更新一条文档（幂等，content_id 相同则覆盖）。
    ///
    /// # Errors
    /// 写入错误 → Fatal。
    pub fn upsert(&self, doc: IndexedDoc) -> Result<(), PartisyError> {
        let writer = self.writer.write().map_err(|_| {
            err(
                "writer lock poison",
                std::io::Error::new(std::io::ErrorKind::Other, "RwLock poison"),
            )
        })?;
        let (id_field, fn_field, tags_field, ocr_field, tx_field, upd_field) =
            field_ids(&self.schema);

        // 先删旧文档（同一 content_id 的旧版本）
        let term = tantivy::Term::from_field_text(id_field, &doc.content_id);
        writer.delete_term(term);

        // 再添加新文档
        let mut d = TantivyDocument::default();
        d.add_text(id_field, &doc.content_id);
        d.add_text(fn_field, &doc.filename);
        d.add_text(tags_field, &doc.tags.join(" "));
        if let Some(ref ocr) = doc.ocr_text {
            d.add_text(ocr_field, ocr);
        }
        if let Some(ref tx) = doc.transcript_text {
            d.add_text(tx_field, tx);
        }
        d.add_i64(upd_field, doc.updated_ns);

        writer
            .add_document(d)
            .map_err(|e| err("tantivy add_document", e))?;
        Ok(())
    }

    /// 批量 upsert（批量操作，减少 commit 次数）。
    ///
    /// # Errors
    /// 任意写入错误 → Fatal。
    pub fn upsert_batch(
        &self,
        docs: impl IntoIterator<Item = IndexedDoc>,
    ) -> Result<(), PartisyError> {
        let writer = self.writer.write().map_err(|_| {
            err(
                "writer lock poison",
                std::io::Error::new(std::io::ErrorKind::Other, "RwLock poison"),
            )
        })?;
        let (id_field, fn_field, tags_field, ocr_field, tx_field, upd_field) =
            field_ids(&self.schema);

        for doc in docs {
            let term = tantivy::Term::from_field_text(id_field, &doc.content_id);
            writer.delete_term(term);

            let mut d = TantivyDocument::default();
            d.add_text(id_field, &doc.content_id);
            d.add_text(fn_field, &doc.filename);
            d.add_text(tags_field, &doc.tags.join(" "));
            if let Some(ref ocr) = doc.ocr_text {
                d.add_text(ocr_field, ocr);
            }
            if let Some(ref tx) = doc.transcript_text {
                d.add_text(tx_field, tx);
            }
            d.add_i64(upd_field, doc.updated_ns);
            writer
                .add_document(d)
                .map_err(|e| err("tantivy add_document", e))?;
        }
        Ok(())
    }

    /// 提交所有待写入（新建索引后或 upsert_batch 后必须调用）。
    ///
    /// # Errors
    /// commit 错误 → Fatal。
    pub fn commit(&self) -> Result<(), PartisyError> {
        let writer = self.writer.write().map_err(|_| {
            err(
                "writer lock poison",
                std::io::Error::new(std::io::ErrorKind::Other, "RwLock poison"),
            )
        })?;
        writer.commit().map_err(|e| err("tantivy commit", e))
    }

    /// 执行 BM25 查询。
    ///
    /// # Errors
    /// 查询解析 / 搜索错误 → Fatal。
    pub fn search(&self, q: Bm25Query) -> Result<Bm25Result, PartisyError> {
        let start = std::time::Instant::now();
        let searcher = self.reader.searcher();

        let parsed = self
            .parser
            .parse_query(&q.query)
            .map_err(|e| err("bm25 parse query", e))?;

        let (id_field, _fn_field, _tags_field, ocr_field, tx_field, _) = field_ids(&self.schema);

        let top_docs = searcher
            .search(&parsed, &TopDocs::with_limit(q.limit))
            .map_err(|e| err("bm25 search", e))?;

        let total = top_docs.len();
        let mut hits = Vec::with_capacity(top_docs.len());

        for (score, doc_address) in top_docs {
            let retrieved: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| err("bm25 doc fetch", e))?;

            let content_id = retrieved
                .get_first(id_field)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            // 简单 highlight：取 ocr 或 transcript 字段内容片段
            let highlight = retrieved
                .get_first(ocr_field)
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| {
                    let len = s.len().min(200);
                    format!("{}…", &s[..len])
                })
                .or_else(|| {
                    retrieved
                        .get_first(tx_field)
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| {
                            let len = s.len().min(200);
                            format!("{}…", &s[..len])
                        })
                });

            hits.push(Bm25Hit {
                content_id,
                score,
                highlight,
            });
        }

        let elapsed = start.elapsed().as_millis() as u32;
        Ok(Bm25Result {
            hits,
            total,
            timing_ms: elapsed,
        })
    }

    /// 清空全部文档（用于 rebuild_index）。
    ///
    /// # Errors
    /// commit 错误 → Fatal。
    pub fn clear(&self) -> Result<(), PartisyError> {
        let writer = self.writer.write().map_err(|_| {
            err(
                "writer lock poison",
                std::io::Error::new(std::io::ErrorKind::Other, "RwLock poison"),
            )
        })?;
        writer
            .delete_all_documents()
            .map_err(|e| err("tantivy delete_all", e))?;
        writer.commit().map_err(|e| err("tantivy commit", e))
    }

    /// 返回索引当前文档数（近似）。
    #[must_use]
    pub fn approx_count(&self) -> u64 {
        self.reader.searcher().num_docs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bm25_basic() {
        let dir = tempfile::tempdir().unwrap();
        let idx = Bm25Index::open_or_create(dir.path()).unwrap();

        idx.upsert(IndexedDoc {
            content_id: "c1".to_string(),
            filename: "report.pdf".to_string(),
            tags: vec!["finance".to_string(), "2024".to_string()],
            ocr_text: Some("Q3 revenue grew 20% year over year".to_string()),
            transcript_text: None,
            updated_ns: 1_700_000_000_000_000_000_i64,
        })
        .unwrap();

        idx.upsert(IndexedDoc {
            content_id: "c2".to_string(),
            filename: "meeting.mp4".to_string(),
            tags: vec!["meeting".to_string()],
            ocr_text: Some("Slide 1: Welcome to the quarterly review".to_string()),
            transcript_text: Some("John: Let's discuss Q3 results".to_string()),
            updated_ns: 1_700_000_000_000_000_001_i64,
        })
        .unwrap();

        idx.commit().unwrap();

        // 精确查询
        let result = idx
            .search(Bm25Query {
                query: "revenue".to_string(),
                limit: 10,
                include_transcript: true,
            })
            .unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.hits[0].content_id, "c1");

        // 多字段查询（OCR + filename）
        let result2 = idx
            .search(Bm25Query {
                query: "Q3".to_string(),
                limit: 10,
                include_transcript: true,
            })
            .unwrap();
        assert_eq!(result2.total, 2); // c1 和 c2 都含 Q3

        // 精确查询无结果
        let result3 = idx
            .search(Bm25Query {
                query: "nonexistent_xyz".to_string(),
                limit: 10,
                include_transcript: true,
            })
            .unwrap();
        assert_eq!(result3.total, 0);
        assert!(result3.hits.is_empty());
    }
}
