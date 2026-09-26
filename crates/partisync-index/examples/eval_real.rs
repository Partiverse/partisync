//! M6-D67 EvalRunner CLI 驱动（SPEC M6-D67 v0.1 接受 v0）
//!
//! 用法:
//!   cargo run -p partisync-index --example eval_real -- \
//!       --k 10 \
//!       --corpus-dir <dir> \
//!       --corpus-tsv <file> \
//!       --queries <file.jsonl> \
//!       --qrels <file.jsonl> \
//!       --index-dir <tmpdir> \
//!       --out <eval.json>
//!
//! 输出 JSON 到 stdout（同时 --out 落盘）， 形如：
//!   {"mode":"bm25_only","num_queries":N,"k":10,
//!    "mean_recall_at_k":...,"mean_mrr":...,"mean_ndcg_at_k":...,
//!    "per_query":[{"query_id":"...","recall_at_k":...,"mrr":...,"ndcg_at_k":...}]}

use std::path::PathBuf;
use std::process::ExitCode;

use partisync_index::EvalRunner;

struct Args {
    k: usize,
    corpus_dir: PathBuf,
    corpus_tsv: PathBuf,
    queries: PathBuf,
    qrels: PathBuf,
    index_dir: PathBuf,
    out: PathBuf,
    mode: String, // 预留: hybrid_no_rerank / hybrid_with_rerank 当前未实装
}

fn arg_value<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    let pos = args.iter().position(|a| a == key)?;
    args.get(pos + 1).map(String::as_str)
}

fn parse_args() -> Result<Args, String> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let k: usize = arg_value(&raw, "--k")
        .ok_or("缺少 --k")?
        .parse()
        .map_err(|e| format!("--k 非整数：{e}"))?;
    let get_path = |flag: &str, needed: bool| -> Result<PathBuf, String> {
        if let Some(v) = arg_value(&raw, flag) {
            Ok(PathBuf::from(v))
        } else if needed {
            Err(format!("缺少 {flag}"))
        } else {
            Ok(PathBuf::from(""))
        }
    };
    let corpus_dir = get_path("--corpus-dir", true)?;
    let corpus_tsv = get_path("--corpus-tsv", true)?;
    let queries = get_path("--queries", true)?;
    let qrels = get_path("--qrels", true)?;
    let index_dir = get_path("--index-dir", true)?;
    let out = get_path("--out", true)?;
    let mode = arg_value(&raw, "--mode").unwrap_or("bm25_only").to_string();
    Ok(Args {
        k,
        corpus_dir,
        corpus_tsv,
        queries,
        qrels,
        index_dir,
        out,
        mode,
    })
}

fn run() -> Result<(), String> {
    let a = parse_args()?;
    let runner = EvalRunner::new(a.k, &a.corpus_dir, &a.corpus_tsv, &a.queries, &a.qrels);
    let report = match a.mode.as_str() {
        "bm25_only" => runner
            .run_bm25_only(&a.index_dir)
            .map_err(|e| format!("run_bm25_only: {e:?}"))?,
        "hybrid_no_rerank" | "hybrid_with_rerank" => {
            return Err(format!(
                "模式 {} 尚未实装（EvalRunner 当前仅 bm25_only； 待 fastembed 实装后补）",
                a.mode
            ));
        }
        other => return Err(format!("未知模式：{other}")),
    };

    let json = serde_json::json!({
        "mode": report.mode,
        "k": report.k,
        "num_queries": report.num_queries,
        "mean_recall_at_k": report.mean_recall_at_k,
        "mean_mrr": report.mean_mrr,
        "mean_ndcg_at_k": report.mean_ndcg_at_k,
        "per_query": report.per_query.iter().map(|(qid, m)| serde_json::json!({
            "query_id": qid,
            "recall_at_k": m.recall_at_k,
            "mrr": m.mrr,
            "ndcg_at_k": m.ndcg_at_k,
        })).collect::<Vec<_>>(),
    });
    let s = serde_json::to_string_pretty(&json).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(&a.out, &s).map_err(|e| format!("write out: {e}"))?;
    println!("{s}");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ERROR: {e}");
            ExitCode::from(2)
        }
    }
}
