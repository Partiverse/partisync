//! M6-WP01 demo 单查询驱动器（SPEC M6-WP01 v0.1）
//!
//! CLI 上的 `partisync search` 走 IndexEngine 的 tantivy 路径（需要 sidecar
//! pipeline 预先灌入）。 demo 场景里我们直接用 Bm25Index 走纯 BM25 路径，
//! 1 个 fixture corpus + 1 条 query 即时出命中结果。
//!
//! 输入（位置参数）:
#![allow(clippy::doc_markdown)]
//!   $1 = corpus_dir  markdown 语料目录
//!   $2 = query       单个查询字符串（demo 演示用）
//!   $3 = index_dir   BM25 索引目录（首次写入后保留）
//!   $4 = limit       top-K（默认 5）
//!
//! 输出：人友好格式，可被 demo.sh 收集进 transcript。
//!
//! 实现：复用 EvalRunner 的 corpus loader（driven by corpus.tsv）而非穷举
//! ls .md —— 这样 tags / updated_ns 等元数据也能被 IndexedDoc 吸收， 与
//! EvalRunner 行为一致。

use std::path::PathBuf;

use partisync_index::search::bm25::{Bm25Index, Bm25Query, IndexedDoc};

fn load_corpus(
    corpus_dir: &std::path::Path,
    corpus_tsv: Option<&std::path::Path>,
) -> Vec<IndexedDoc> {
    // 简化：直接扫 .md， 不强求 TSV。 Markdown body → ocr_text 模拟。
    let mut docs = Vec::new();
    let entries = std::fs::read_dir(corpus_dir).expect("read corpus dir");
    for ent in entries.flatten() {
        let path = ent.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let body = std::fs::read_to_string(&path).unwrap_or_default();
        let content_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown.md")
            .to_string();
        docs.push(IndexedDoc {
            content_id,
            filename,
            tags: vec!["demo".into()],
            ocr_text: Some(body),
            transcript_text: None,
            updated_ns: 0,
        });
    }
    let _ = corpus_tsv;
    docs
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!(
            "usage: demo_query <corpus_dir> <query> <index_dir> [limit]\n\
             corpus_dir:  markdown directory (e.g. wp06_corpus)\n\
             query:       single query string\n\
             index_dir:   bm25 index dir (will be created/reused)\n\
             limit:       top-K (default 5)"
        );
        std::process::exit(2);
    }
    let corpus_dir = PathBuf::from(&args[0]);
    let query = args[1].clone();
    let index_dir = PathBuf::from(&args[2]);
    let limit: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(5);

    std::fs::create_dir_all(&index_dir).expect("create index dir");
    let bm25 = Bm25Index::open_or_create(&index_dir).expect("open bm25");
    let docs = load_corpus(&corpus_dir, None);
    bm25.upsert_batch(docs).expect("upsert");
    bm25.commit().expect("commit");
    bm25.reload().expect("reload");

    let result = bm25
        .search(Bm25Query {
            query: query.clone(),
            limit,
            include_transcript: true,
        })
        .expect("search");

    println!("  mode: BM25 (top-{limit}, took {}ms)", result.timing_ms);
    if result.hits.is_empty() {
        println!("  (no hits)");
        return;
    }
    for hit in &result.hits {
        let hl = hit.highlight.as_deref().unwrap_or("-");
        println!(
            "  [{score:.4}] {content_id}  {hl}",
            score = hit.score,
            content_id = hit.content_id,
            hl = hl
        );
    }
    println!("  ({} total hits)", result.total);
}
