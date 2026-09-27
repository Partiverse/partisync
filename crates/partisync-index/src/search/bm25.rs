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
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument};

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

fn err(what: &str, e: impl std::fmt::Display) -> PartisyError {
    PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("{what}: {e}").into()),
    }
}

/// 清洗查询字符串：剥离 tantivy QueryParser 的保留算子字符与其 CJK 全角
/// 对应字， 替换为空格； 字母/数字/中文/下划线/空格保留。 后续 BM25 词项
/// 切分走 tantivy 默认 tokenzier， 因此仅在标点层做规整， 不改变语义。
///
/// 触发动机： LCSTS query 频含 `：` `？` `！` `，` 等全角标点， 直送
/// tantivy QueryParser 会报 `Syntax Error`（见 SPEC M6-D67 §D7）。
///
/// 行为契约见本文件 `tests::sanitize_query_strips_reserved` 单测（私有函数，
/// doctest 在 `pub` 范围外不可见）。
fn sanitize_query(q: &str) -> String {
    // tantivy 保留字符：+ - ^ " * ? : ~ ( ) [ ] { } \ / ! 及其 CJK 全角对应字
    const RESERVED: &[char] = &[
        '+', '-', '^', '"', '\'', '*', '?', ':', '~', '(', ')', '[', ']', '{', '}', '\\', '/', '!',
        '　', '：', '？', '！', '（', '）', '【', '】', '「', '」', '『', '』', '《', '》', '，',
        '。', '、', '；', '—', '…', '～', '＂', '＇', '＋', '＝', '＜', '＞',
    ];
    let mut out = String::with_capacity(q.len());
    let mut prev_space = false;
    for c in q.chars() {
        if RESERVED.contains(&c) {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

/// 是否 CJK 统一表意（基本汉字 + 扩展 A–F + 兼容 + 部首 + 笔画）。
/// 与 jieba 等中文分词器对「中文」的覆盖一致。
fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs (基本汉字)
        | '\u{3400}'..='\u{4DBF}' // CJK Extension A
        | '\u{20000}'..='\u{2A6DF}' // Extension B
        | '\u{2A700}'..='\u{2B73F}' // Extension C
        | '\u{2B740}'..='\u{2B81F}' // Extension D
        | '\u{2B820}'..='\u{2CEAF}' // Extension E
        | '\u{F900}'..='\u{FAFF}'   // CJK Compatibility Ideographs
        | '\u{2F800}'..='\u{2FA1F}' // Compatibility Supplement
    )
}

/// CJK 字符 n-gram 切分： 含 CJK 的 run 切成 unigram + bigram， 空格分隔。
///
/// 触发动机： tantivy `SimpleTokenizer` 把无空格中文 run 当作 ONE token
/// （UAX#29 字母连贯）， BM25 文档/查询粒度错位 → Recall=0。 本函数
/// 在写入文档 / 解析查询前把 CJK 字符 fan-out 为空格分隔的 unigram 与
/// bigram； tantivy 默认分词器随后按空格切分。 复杂度 O(n)， 零额外依赖。
///
/// 行为契约见本文件 `tests::cjk_fan_out_basic` 单测（私有函数， doctest
/// 在 `pub` 范围外不可见）。
fn cjk_fan_out(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    let mut prev: Option<char> = None;
    let mut prev_was_cjk = false;
    for c in s.chars() {
        if is_cjk(c) {
            // 单字 unigram
            out.push(c);
            out.push(' ');
            // 与前一个 CJK 拼 bigram
            if prev_was_cjk {
                if let Some(p) = prev {
                    out.push(p);
                    out.push(c);
                    out.push(' ');
                }
            }
            prev = Some(c);
            prev_was_cjk = true;
        } else {
            // 非 CJK 字符原样写入（保留分词边界）
            out.push(c);
            prev_was_cjk = false;
            prev = None;
        }
    }
    out
}

/// BM25 全文索引（tantivy 0.26）。
///
/// 实例持有 `Index`（不可变）和 `IndexWriter`（可变，共享写锁）。
/// 查询走独立 `IndexReader`。
pub struct Bm25Index {
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

        let index = if path.join("meta.json").exists() {
            // tantivy 索引目录的判据是 meta.json（目录可能已被外部预先创建）
            Index::open_in_dir(path).map_err(|e| err("open tantivy index", e))?
        } else {
            std::fs::create_dir_all(path)
                .map_err(|e| err("create tantivy index dir", std::io::Error::other(e)))?;
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

        let (_id_field, fn_field, tags_field, ocr_field, tx_field, _upd_field) = field_ids(&schema);
        let parser =
            QueryParser::for_index(&index, vec![fn_field, tags_field, ocr_field, tx_field]);

        Ok(Self {
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
        let writer = self
            .writer
            .write()
            .map_err(|_| err("writer lock poison", std::io::Error::other("RwLock poison")))?;
        let (id_field, fn_field, tags_field, ocr_field, tx_field, upd_field) =
            field_ids(&self.schema);

        // 先删旧文档（同一 content_id 的旧版本）
        let term = tantivy::Term::from_field_text(id_field, &doc.content_id);
        writer.delete_term(term);

        // 再添加新文档
        let mut d = TantivyDocument::default();
        d.add_text(id_field, &doc.content_id);
        d.add_text(fn_field, &doc.filename);
        d.add_text(tags_field, doc.tags.join(" "));
        if let Some(ref ocr) = doc.ocr_text {
            d.add_text(ocr_field, cjk_fan_out(ocr));
        }
        if let Some(ref tx) = doc.transcript_text {
            d.add_text(tx_field, cjk_fan_out(tx));
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
        let writer = self
            .writer
            .write()
            .map_err(|_| err("writer lock poison", std::io::Error::other("RwLock poison")))?;
        let (id_field, fn_field, tags_field, ocr_field, tx_field, upd_field) =
            field_ids(&self.schema);

        for doc in docs {
            let term = tantivy::Term::from_field_text(id_field, &doc.content_id);
            writer.delete_term(term);

            let mut d = TantivyDocument::default();
            d.add_text(id_field, &doc.content_id);
            d.add_text(fn_field, &doc.filename);
            d.add_text(tags_field, doc.tags.join(" "));
            if let Some(ref ocr) = doc.ocr_text {
                d.add_text(ocr_field, cjk_fan_out(ocr));
            }
            if let Some(ref tx) = doc.transcript_text {
                d.add_text(tx_field, cjk_fan_out(tx));
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
        let mut writer = self
            .writer
            .write()
            .map_err(|_| err("writer lock poison", std::io::Error::other("RwLock poison")))?;
        // tantivy 0.26 commit 返回 Opstamp（u64），丢弃
        writer
            .commit()
            .map(|_| ())
            .map_err(|e| err("tantivy commit", e))
    }

    /// 显式重载 reader（用于 OnCommitWithDelay 策略下确保下一次 search 看到最新写入）。
    ///
    /// # Errors
    /// 重载错误 → Fatal。
    pub fn reload(&self) -> Result<(), PartisyError> {
        self.reader.reload().map_err(|e| err("tantivy reload", e))
    }

    /// 执行 BM25 查询。
    ///
    /// 在丢给 tantivy `QueryParser` 前做两层预处理：
    /// 1. `sanitize_query` 移除 tantivy 保留算子字符与全角 CJK 标点， 避免
    ///    `Syntax Error`。
    /// 2. `cjk_fan_out` 把 CJK 字符 run 切到 unigram + bigram 空格分隔，
    ///    与文档写入时的预处理对齐（tantivy 默认分词器 UAX#29 把无空格
    ///    中文视作 ONE token， 导致 BM25 文档/查询粒度错位 → Recall=0）。
    ///
    /// # Errors
    /// 查询解析 / 搜索错误 → Fatal。
    pub fn search(&self, q: Bm25Query) -> Result<Bm25Result, PartisyError> {
        let start = std::time::Instant::now();
        let searcher = self.reader.searcher();

        let pre_query = cjk_fan_out(&sanitize_query(&q.query));
        let parsed = self
            .parser
            .parse_query(&pre_query)
            .map_err(|e| err("bm25 parse query", e))?;

        let (id_field, _fn_field, _tags_field, ocr_field, tx_field, _) = field_ids(&self.schema);

        let top_docs = searcher
            .search(&parsed, &TopDocs::with_limit(q.limit).order_by_score())
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
        let mut writer = self
            .writer
            .write()
            .map_err(|_| err("writer lock poison", std::io::Error::other("RwLock poison")))?;
        writer
            .delete_all_documents()
            .map_err(|e| err("tantivy delete_all", e))?;
        writer
            .commit()
            .map(|_| ())
            .map_err(|e| err("tantivy commit", e))
    }

    /// 强制同步刷新 reader（tantivy `OnCommitWithDelay` 是异步后台刷新，
    /// commit 后立刻检索可能读到旧快照；测试/紧一致场景显式调用）。
    ///
    /// # Errors
    /// reload 错误 → Fatal。
    pub fn force_reload(&self) -> Result<(), PartisyError> {
        self.reader
            .reload()
            .map_err(|e| err("tantivy reader reload", e))
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
        idx.force_reload().unwrap();

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

    #[test]
    fn sanitize_query_strips_reserved() {
        // CJK 全角标点 → 空格（去尾随空白）
        assert_eq!(sanitize_query("银行：房贷政策没变？"), "银行 房贷政策没变");
        // ASCII 双引号 / 加号替换；首尾 trim， 中间多空格保留（tantivy tokenize 时折叠）
        assert_eq!(sanitize_query("\"hello\" +world"), "hello   world");
        // 混合全角标点 + 引号；连续保留字符折叠为单空格
        assert_eq!(
            sanitize_query("（新华视点）：反腐\"灰色文化\"？"),
            "新华视点 反腐 灰色文化"
        );
        // ASCII 半角保留字符亦清洗
        assert_eq!(sanitize_query("a+b*c"), "a b c");
        // 数字 / 字母 / 下划线保留
        assert_eq!(sanitize_query("Q3_review 2024"), "Q3_review 2024");
        // 全部保留字 → 空串
        assert_eq!(sanitize_query(":::"), "");
        // 多次连续保留字符 → 单空格（trim 后为空）
        assert_eq!(sanitize_query("a?!?!b"), "a b");
    }

    #[test]
    fn search_handles_cjk_punctuation() {
        // 回归：含 CJK 全角标点的 query 不再触发 tantivy Syntax Error。
        let dir = tempfile::tempdir().unwrap();
        let idx = Bm25Index::open_or_create(dir.path()).unwrap();

        idx.upsert(IndexedDoc {
            content_id: "c1".to_string(),
            filename: "doc1.md".to_string(),
            tags: vec!["lcsts".to_string()],
            ocr_text: Some("银行：房贷政策没变？央行表态".to_string()),
            transcript_text: None,
            updated_ns: 1_700_000_000_000_000_000_i64,
        })
        .unwrap();
        idx.commit().unwrap();
        idx.force_reload().unwrap();

        let result = idx
            .search(Bm25Query {
                query: "银行：房贷政策没变？".to_string(),
                limit: 10,
                include_transcript: true,
            })
            .unwrap();
        assert!(
            result.total >= 1,
            "CJK query should match doc with shared tokens"
        );
        assert_eq!(result.hits[0].content_id, "c1");
    }

    #[test]
    fn cjk_fan_out_basic() {
        // 2 个 CJK → 2 unigram + 1 bigram + 尾部空格
        assert_eq!(cjk_fan_out("银行"), "银 行 银行 ");
        // 5 个 CJK → 5 unigram + 4 bigram
        assert_eq!(
            cjk_fan_out("新华社受权"),
            "新 华 新华 社 华社 受 社受 权 受权 "
        );
        // 非 CJK 原样保留
        assert_eq!(cjk_fan_out("hello"), "hello");
        assert_eq!(cjk_fan_out("Q3 review"), "Q3 review");
        // 混合 CJK + 标点 + 拉丁（CJK unigram + 标点/拉丁原样保留）
        assert_eq!(cjk_fan_out("中-A"), "中 -A");
        // 空串
        assert_eq!(cjk_fan_out(""), "");
    }

    #[test]
    fn cjk_fan_out_match_lcsts() {
        // 回归： 真实 LCSTS query / 文档走 cjk_fan_out 后能命中。
        let dir = tempfile::tempdir().unwrap();
        let idx = Bm25Index::open_or_create(dir.path()).unwrap();

        idx.upsert(IndexedDoc {
            content_id: "c1".to_string(),
            filename: "lcsts_doc.md".to_string(),
            tags: vec!["lcsts".to_string()],
            ocr_text: Some("新华社受权于18日全文播发修改后的立法法全文".to_string()),
            transcript_text: None,
            updated_ns: 1_700_000_000_000_000_000_i64,
        })
        .unwrap();
        idx.commit().unwrap();
        idx.force_reload().unwrap();

        // 查询 1: 完整 bigram 匹配 → 必中
        let r = idx
            .search(Bm25Query {
                query: "新华社受权".to_string(),
                limit: 10,
                include_transcript: true,
            })
            .unwrap();
        assert!(r.total >= 1, "完整 query 应命中");
        assert_eq!(r.hits[0].content_id, "c1");

        // 查询 2: 部分 unigram 匹配 → 仍命中
        let r = idx
            .search(Bm25Query {
                query: "立法".to_string(),
                limit: 10,
                include_transcript: true,
            })
            .unwrap();
        assert!(r.total >= 1, "部分 unigram 应命中");
    }
}
