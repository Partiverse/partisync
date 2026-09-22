title: tantivy BM25 多字段设计
filename: tantivy_bm25_field_design.md
tags: [tantivy, bm25, search, full-text, indexing]
updated_ns: 1704412800000000000

# tantivy BM25 多字段设计

## 字段设计原则

- **高频查询字段** = `TEXT | STORED`（分词 + 可取回）
- **精确匹配字段** = `STRING | STORED`（整词匹配）
- **元数据字段** = `INDEXED | STORED`（数字/日期）

## PartiSync 索引字段

| 字段 | 类型 | 分词 | 用途 |
|---|---|---|---|
| content_id | STRING | 无 | 主键 |
| filename | TEXT | 标准 | filename 命中 |
| tags | TEXT | 标准 | 标签命中 |
| ocr_text | TEXT | stemmed | OCR 文本 |
| transcript_text | TEXT | stemmed | 转写文本 |
| updated_ns | I64 INDEXED | 无 | 增量更新 |

## query 构造

```rust
let parser = QueryParser::for_index(&index, vec![filename, tags, ocr_text, transcript_text]);
let query = parser.parse_query("夏威夷 度假")?;
```

## 高亮

snippet = 前 200 字符 + 命中关键词 `<em>` 包。

## 性能

- 写入：`IndexWriter` 缓冲 50MB flush
- 查询：TopDocs.with_limit(100).order_by_score()
- commit：`commit()` 返回 Opstamp（用于增量）

## 坑位

- `TopDocs::with_limit(n)` 不是 Collector，必须 `.order_by_score()`
- `IndexWriter` commit 需要 `&mut self`
- reserve ahead of insertions（usearch 类似）