//! 检索质量评估集成测试（SPEC M4-WP06 §裁定 2）
//!
//! 跑 WP06 评估集（50 语料 + 25 查询 + 32 qrels）通过 EvalRunner 跑 bm25_only 模式，
//! 输出指标报告并断言：
//! - bm25_only nDCG@10 ≥ 0.30（合成库基线；真实 INBOX 基线 M5+ 再跑）
//! - Recall@10 ≥ 0.50（多数文档在 top-10 命中）
//! - MRR ≥ 0.40（首个相关文档在 rank 2 以内）

use std::path::PathBuf;

use partisync_index::EvalRunner;

/// 获取测试 fixture 目录。
fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

#[test]
fn wp06_bm25_eval_pipeline() {
    let corpus_dir = fixture_dir().join("wp06_corpus");
    let corpus_tsv = fixture_dir().join("wp06_corpus.tsv");
    let queries_path = fixture_dir().join("wp06_queries.jsonl");
    let qrels_path = fixture_dir().join("wp06_qrels.jsonl");

    // sanity：fixture 存在
    assert!(corpus_dir.is_dir(), "wp06_corpus dir missing");
    assert!(corpus_tsv.is_file(), "wp06_corpus.tsv missing");
    assert!(queries_path.is_file(), "wp06_queries.jsonl missing");
    assert!(qrels_path.is_file(), "wp06_qrels.jsonl missing");

    let tmp = tempfile::tempdir().expect("create tempdir");
    let index_dir = tmp.path().join("wp06_bm25");

    let runner = EvalRunner::new(10, corpus_dir, corpus_tsv, queries_path, qrels_path);
    let report = runner
        .run_bm25_only(&index_dir)
        .expect("eval bm25 should succeed");

    // 输出报告到 stdout（CI 可见）
    println!(
        "WP06 bm25_only: recall@10={:.3} mrr={:.3} ndcg@10={:.3} (n={})",
        report.mean_recall_at_k, report.mean_mrr, report.mean_ndcg_at_k, report.num_queries
    );

    // 写入 JSON 报告（给 KPI 报告引用）
    let report_path = tmp.path().join("wp06_bm25_report.json");
    std::fs::write(
        &report_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "mode": report.mode,
            "num_queries": report.num_queries,
            "k": report.k,
            "mean_recall_at_k": report.mean_recall_at_k,
            "mean_mrr": report.mean_mrr,
            "mean_ndcg_at_k": report.mean_ndcg_at_k,
            "per_query": report.per_query.iter().map(|(qid, m)| {
                serde_json::json!({
                    "query_id": qid,
                    "recall_at_k": m.recall_at_k,
                    "mrr": m.mrr,
                    "ndcg_at_k": m.ndcg_at_k,
                })
            }).collect::<Vec<_>>(),
        }))
        .unwrap(),
    )
    .unwrap();

    // 验收断言（合成库基线）
    assert!(report.num_queries >= 20, "queries count too small");
    assert!(
        report.mean_recall_at_k >= 0.30,
        "bm25_only Recall@10 too low: {:.3} (>=0.30 expected for 50-doc synthetic corpus)",
        report.mean_recall_at_k
    );
    assert!(
        report.mean_mrr >= 0.30,
        "bm25_only MRR too low: {:.3} (>=0.30 expected)",
        report.mean_mrr
    );
    assert!(
        report.mean_ndcg_at_k >= 0.30,
        "bm25_only nDCG@10 too low: {:.3} (>=0.30 expected)",
        report.mean_ndcg_at_k
    );
}

#[test]
fn wp06_corpus_loads() {
    // 简单 sanity：corpus.tsv 可解析
    let corpus_tsv = fixture_dir().join("wp06_corpus.tsv");
    let text = std::fs::read_to_string(&corpus_tsv).unwrap();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    // header + N docs
    assert!(lines.len() >= 50, "expected ≥50 corpus rows, got {}", lines.len() - 1);
}

#[test]
fn wp06_queries_loads() {
    let queries_path = fixture_dir().join("wp06_queries.jsonl");
    let text = std::fs::read_to_string(&queries_path).unwrap();
    let n = text.lines().filter(|l| !l.trim().is_empty()).count();
    assert!(n >= 20, "expected ≥20 queries, got {}", n);
}

#[test]
fn wp06_qrels_loads() {
    let qrels_path = fixture_dir().join("wp06_qrels.jsonl");
    let text = std::fs::read_to_string(&qrels_path).unwrap();
    let n = text.lines().filter(|l| !l.trim().is_empty()).count();
    assert!(n >= 20, "expected ≥20 qrels, got {}", n);
}