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
use tantivy::snippet::{collapse_overlapped_ranges, SnippetGenerator};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, Searcher, TantivyDocument};

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
                                                              // M10-WP01-T02：ocr/transcript 补 STORED——tantivy 0.26 的 `TEXT` 常量
                                                              // stored: false（schema/text_options.rs:276），此前 get_first 恒取不到
                                                              // 值、highlight 恒 None，「头部 200 字截断」实际从未产出过内容。
                                                              // snippet（SnippetGenerator.snippet_from_doc）同样依赖 stored 值。
                                                              // 存量索引按目录内嵌旧 schema 打开：打开/检索不受影响，highlight 走
                                                              // 回落 None，重建（partisync reindex）后摘要生效。
    schema_builder.add_text_field(FIELD_OCR_TEXT, TEXT | STORED); // snippet 依赖 stored 值
    schema_builder.add_text_field(FIELD_TRANSCRIPT_TEXT, TEXT | STORED);
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

/// snippet 窗口字符上限（M10-WP01 §2.2「≤ ~200 字符量级」，与旧头部截断
/// 同量级；`SnippetGenerator::set_max_num_chars` 按字符计）。
const SNIPPET_MAX_CHARS: usize = 200;
/// 命中词 sentinel：highlight 载荷为「纯文本 + `[[`/`]]` 包裹命中词」，
/// 前端 esc 全串后替换为 `<mark>`/`</mark>`（SPEC §2.2）。R2：与正文撞串
/// 仅视觉误差（esc 后无其他 HTML 引入面），接受。
const SNIPPET_OPEN: &str = "[[";
const SNIPPET_CLOSE: &str = "]]";

/// 建 per-field `SnippetGenerator`（窗口 [`SNIPPET_MAX_CHARS`] 字符）。
/// 创建失败（如查询词在该字段无词面 → terms_text 为空不算失败）→ None，
/// 该字段回落旧头部截断——不 panic、不新增错误面（SPEC §2.2）。
fn snippet_generator(
    searcher: &Searcher,
    query: &dyn tantivy::query::Query,
    field: Field,
) -> Option<SnippetGenerator> {
    let mut gen = SnippetGenerator::create(searcher, query, field).ok()?;
    gen.set_max_num_chars(SNIPPET_MAX_CHARS);
    Some(gen)
}

/// 命中词定位摘要：text 含任一查询词面 → snippet fragment 以 sentinel
/// `[[`/`]]` 包裹命中 token 后返回；空文本 / 无生成器 / 无词面（如纯向量
/// 命中、查询词仅命中 filename/tags 字段）→ None（调用方回落旧头部截断）。
///
/// 自拼 sentinel 而不用 `Snippet::to_html()`：后者会 `encode_minimal` 转
/// 义 fragment（`&`→`&amp;` 等），前端按契约还要整体 esc 一次 → 双重转义
/// 显示错字。载荷保持纯文本，转义职责完全归前端。
fn field_snippet(gen: Option<&SnippetGenerator>, text: Option<&str>) -> Option<String> {
    let text = text.filter(|s| !s.is_empty())?;
    let snippet = gen?.snippet(text);
    if snippet.is_empty() {
        return None;
    }
    let fragment = snippet.fragment();
    let mut out = String::with_capacity(
        fragment.len() + snippet.highlighted().len() * (SNIPPET_OPEN.len() + SNIPPET_CLOSE.len()),
    );
    let mut pos = 0usize;
    for range in collapse_overlapped_ranges(snippet.highlighted()) {
        out.push_str(fragment.get(pos..range.start)?);
        out.push_str(SNIPPET_OPEN);
        out.push_str(fragment.get(range.start..range.end)?);
        out.push_str(SNIPPET_CLOSE);
        pos = range.end;
    }
    out.push_str(fragment.get(pos..)?);
    Some(out)
}

/// 旧「头部截断」回落（M10-WP01-T02 契约：无词面/生成器缺失时维持现状
/// 行为）。按字符边界截断——旧实现 `&s[..200]` 按字节切，中文多字节字符
/// 中界会 panic；回落路径重写时一并修正。
fn head_truncate(s: &str) -> String {
    let end = s
        .char_indices()
        .nth(SNIPPET_MAX_CHARS)
        .map_or(s.len(), |(i, _)| i);
    format!("{}…", &s[..end])
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
///
/// 只读打开（[`open_read_only`](Self::open_read_only)）时 `writer` 为
/// `None`（P24-a：不获取 `.tantivy-writer.lock`）；写方法返回结构化
/// 错误，读方法（search/approx_count/reload）全量可用。
pub struct Bm25Index {
    writer: Option<RwLock<IndexWriter>>,
    reader: IndexReader,
    parser: QueryParser,
    /// 排除转写字段的解析器（`Bm25Query::include_transcript = false` 走
    /// 此面——M9-WP03-T01 补齐：此前该参数在 bm25 路径从未被 search()
    /// 尊重，parser 恒含 tx 字段，「关闭可提升速度/排除转写」名存实亡）。
    parser_no_tx: QueryParser,
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
            .map_err(|e| {
                err(
                    "open tantivy writer (write lock busy: another writer holds the index; read-only queries can use open_read_only; stop other writers before bulk rebuild)",
                    e,
                )
            })?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| err("open tantivy reader", e))?;

        let (_id_field, fn_field, tags_field, ocr_field, tx_field, _upd_field) = field_ids(&schema);
        let parser =
            QueryParser::for_index(&index, vec![fn_field, tags_field, ocr_field, tx_field]);
        let parser_no_tx = QueryParser::for_index(&index, vec![fn_field, tags_field, ocr_field]);

        Ok(Self {
            writer: Some(RwLock::new(writer)),
            reader,
            parser,
            parser_no_tx,
            schema,
        })
    }

    /// 只读打开既存索引（P24-a：不创建/获取 `.tantivy-writer.lock`）。
    ///
    /// 与 [`open_or_create`](Self::open_or_create) 的差异：索引目录必须
    /// 已存在（以 `meta.json` 为判据），且**不建 `IndexWriter`**——写方
    /// 法返回结构化错误，search/approx_count/reload 全量可用（P24-b/c：
    /// 与活跃写者并存、多实例并发均无互斥，tantivy reader 跨进程安全）。
    ///
    /// # Errors
    /// 索引目录不存在（meta.json 缺失）或 tantivy 打开错误 → Fatal。
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, PartisyError> {
        let path = path.as_ref();
        if !path.join("meta.json").exists() {
            return Err(err(
                "open tantivy index read-only: index not found (meta.json missing)",
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("no tantivy index at {}", path.display()),
                ),
            ));
        }
        let index = Index::open_in_dir(path).map_err(|e| err("open tantivy index", e))?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| err("open tantivy reader", e))?;

        let schema = index.schema();
        let (_id_field, fn_field, tags_field, ocr_field, tx_field, _upd_field) = field_ids(&schema);
        let parser =
            QueryParser::for_index(&index, vec![fn_field, tags_field, ocr_field, tx_field]);
        let parser_no_tx = QueryParser::for_index(&index, vec![fn_field, tags_field, ocr_field]);

        Ok(Self {
            writer: None,
            reader,
            parser,
            parser_no_tx,
            schema,
        })
    }

    /// 是否只读打开（P24 探针/装配层判定用）。
    #[must_use]
    pub fn is_read_only(&self) -> bool {
        self.writer.is_none()
    }

    /// 写守卫：只读实例返回结构化错误（P24：写方法在只读实例不可用）。
    fn writer_lock(&self) -> Result<std::sync::RwLockWriteGuard<'_, IndexWriter>, PartisyError> {
        let w = self.writer.as_ref().ok_or_else(|| {
            err(
                "index opened read-only: writer unavailable",
                std::io::Error::other("read-only index (open_read_only); writes belong to the writer owner (reindex/watch)"),
            )
        })?;
        w.write()
            .map_err(|_| err("writer lock poison", std::io::Error::other("RwLock poison")))
    }

    /// 写入/更新一条文档（幂等，content_id 相同则覆盖）。
    ///
    /// # Errors
    /// 写入错误 → Fatal。
    pub fn upsert(&self, doc: IndexedDoc) -> Result<(), PartisyError> {
        let writer = self.writer_lock()?;
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
        let writer = self.writer_lock()?;
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
        let mut writer = self.writer_lock()?;
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
        let parsed = if q.include_transcript {
            self.parser.parse_query(&pre_query)
        } else {
            self.parser_no_tx.parse_query(&pre_query)
        }
        .map_err(|e| err("bm25 parse query", e))?;

        let (id_field, _fn_field, _tags_field, ocr_field, tx_field, _) = field_ids(&self.schema);

        // M10-WP01-T02：命中词定位摘要——per-field SnippetGenerator 在
        // 查询级构建一次（创建失败 → None，逐 hit 回落旧头部截断，
        // SPEC §2.2「不 panic、不新增错误面」）。
        let gen_ocr = snippet_generator(&searcher, &parsed, ocr_field);
        let gen_tx = snippet_generator(&searcher, &parsed, tx_field);

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

            // 命中词定位摘要优先（ocr → tx）；无词面/生成器缺失/字段空
            // → 回落旧头部截断（ocr → tx → None，现状行为不变）。
            let ocr_val = retrieved.get_first(ocr_field).and_then(|v| v.as_str());
            let tx_val = retrieved.get_first(tx_field).and_then(|v| v.as_str());
            let highlight = field_snippet(gen_ocr.as_ref(), ocr_val)
                .or_else(|| field_snippet(gen_tx.as_ref(), tx_val))
                .or_else(|| ocr_val.filter(|s| !s.is_empty()).map(head_truncate))
                .or_else(|| tx_val.filter(|s| !s.is_empty()).map(head_truncate));

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
        let mut writer = self.writer_lock()?;
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

    // ── M10-WP01-T02：命中词定位摘要（SPEC §2.2 / §3）──

    /// 判别性引擎例：命中词置于 >200 字符偏移处——旧「头部 200 字截断」
    /// 实现（bm25.rs:376-394，取字段前缀）摘要里绝无命中词，必败；
    /// SnippetGenerator 窗口必须定位到命中处并以 sentinel `[[`/`]]` 包裹。
    #[test]
    fn t02_snippet_window_centers_on_hit_terms_beyond_head() {
        let dir = tempfile::tempdir().unwrap();
        let idx = Bm25Index::open_or_create(dir.path()).unwrap();

        // filler >200 字符且不含查询词面
        let filler = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(8);
        assert!(filler.chars().count() > 200);
        idx.upsert(IndexedDoc {
            content_id: "s1".to_string(),
            filename: "wildlife.md".to_string(),
            tags: vec![],
            ocr_text: Some(format!("{filler}The quokka is a small marsupial.")),
            transcript_text: None,
            updated_ns: 1_700_000_000_000_000_000_i64,
        })
        .unwrap();
        idx.commit().unwrap();
        idx.force_reload().unwrap();

        let r = idx
            .search(Bm25Query {
                query: "quokka".to_string(),
                limit: 10,
                include_transcript: true,
            })
            .unwrap();
        assert_eq!(r.total, 1);
        let hl = r.hits[0].highlight.as_deref().expect("highlight Some");
        assert!(hl.contains("[[quokka]]"), "sentinel 包裹命中词：{hl}");
        // 窗口有界：去 sentinel 后 ≤ ~200 字符量级（SPEC §3「窗口有界」）
        let plain = hl.replace("[[", "").replace("]]", "");
        assert!(
            plain.chars().count() <= 200,
            "snippet 窗口有界（≤200 字符量级），得到 {} 字符：{hl}",
            plain.chars().count()
        );
    }

    /// 中文命中例（R1：CJK fan-out 词面必须可高亮）+ 无词面回落契约：
    /// 查询词仅在 filename（ocr 有文本无词面）→ 回落头部截断（无
    /// sentinel）；ocr/tx 全空 → None；全程不 panic。回落截断按字符边界
    /// （旧实现 `&s[..200]` 按字节切，中文多字节中界 panic——回落路径
    /// 重写时一并修正）。
    #[test]
    fn t02_snippet_cjk_hit_and_no_term_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let idx = Bm25Index::open_or_create(dir.path()).unwrap();

        // 中文长文：命中词「夸克」置于 >200 字符偏移，filler 不含「夸」「克」
        let zh_filler = "粒子物理标准模型描述基本粒子及其相互作用。".repeat(12);
        assert!(zh_filler.chars().count() > 200);
        idx.upsert(IndexedDoc {
            content_id: "zh1".to_string(),
            filename: "物理笔记.md".to_string(),
            tags: vec![],
            ocr_text: Some(format!("{zh_filler}实验发现了新的夸克。")),
            transcript_text: None,
            updated_ns: 1_700_000_000_000_000_000_i64,
        })
        .unwrap();
        // 无词面例：查询词仅在 filename，ocr 有文本但不含词面
        idx.upsert(IndexedDoc {
            content_id: "fb1".to_string(),
            filename: "budget-falcon.md".to_string(),
            tags: vec![],
            ocr_text: Some("这份文档正文完全不含查询词面，用于验证回落行为。".to_string()),
            transcript_text: None,
            updated_ns: 1_700_000_000_000_000_001_i64,
        })
        .unwrap();
        // 字段全空例：仅 filename 可命中
        idx.upsert(IndexedDoc {
            content_id: "mt1".to_string(),
            filename: "empty-falcon.md".to_string(),
            tags: vec![],
            ocr_text: None,
            transcript_text: None,
            updated_ns: 1_700_000_000_000_000_002_i64,
        })
        .unwrap();
        idx.commit().unwrap();
        idx.force_reload().unwrap();

        // 1) 中文命中：sentinel 包裹 fan-out 词面（bigram「夸克」单独成
        //    token，必被包裹），窗口定位在 >200 字符偏移处
        let r = idx
            .search(Bm25Query {
                query: "夸克".to_string(),
                limit: 10,
                include_transcript: true,
            })
            .unwrap();
        assert_eq!(r.total, 1);
        let hl = r.hits[0]
            .highlight
            .as_deref()
            .expect("中文命中 highlight Some");
        assert!(hl.contains("[[夸克]]"), "sentinel 包裹中文命中词：{hl}");
        let plain = hl.replace("[[", "").replace("]]", "");
        assert!(plain.chars().count() <= 200, "窗口有界：{hl}");
        assert!(hl.contains('夸'), "窗口含命中字：{hl}");

        // 2) 无词面回落例：查询词仅在 filename
        let r = idx
            .search(Bm25Query {
                query: "falcon".to_string(),
                limit: 10,
                include_transcript: true,
            })
            .unwrap();
        assert_eq!(r.total, 2);
        let by_id = |cid: &str| r.hits.iter().find(|h| h.content_id == cid).unwrap();
        // ocr 有文本无词面 → 头部截断回落（Some，无 sentinel）
        let fb = by_id("fb1").highlight.as_deref().expect("回落头截 Some");
        assert!(!fb.contains("[["), "回落不得引入 sentinel：{fb}");
        // ocr/tx 全空 → None
        assert!(by_id("mt1").highlight.is_none(), "字段全空 → None");
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
