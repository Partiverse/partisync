//! M11-WP06-T05 记忆域向量通道批量回填器（dev-target 工具，examples 不入
//! 产品依赖图；沿 import_lcsts.rs 判例——读库、直插、打印预算数字）。
//!
//! `graph-embed` feature 门控（SPEC M11-WP06 §2.1/§2.6；模型下载/推理
//! 不入 CI，评估件 §7）：`cargo run --release -p partisync-graph \
//! --features graph-embed --example backfill_memory_vectors -- <db路径> \
//! [--artifact <memory_vec.usearch>] [--model-cache <dir>] [--limit N]
//! [--no-reconcile]`
//!
//! ADR-0035 决策 3 落锤语义：**回填器按伴表嵌入状态全量扫描补齐**——
//! ①补嵌 pending/running/failed(4)/从未登记 的活行（status=2 done 跳过、
//! status=3 skipped 不重试），upsert 幂等可反复重跑；②**对账重嵌**
//! （M11-WP06-T05 审查处置，P25-d 零静默闭环）：伴表 done 行逐 id
//! `index.contains` 对账（实测 10⁵ 条目 ~0.2µs/次，临时基准 2026-10-11；
//! index crate vector.rs:170-173 判例的 30ms 口径为 remove 非 contains），
//! 缺失即重嵌——覆盖「daemon 伴生产物未落盘 + 进程崩溃」的恢复窗口
//! （daemon 优雅关停已 save，见 partisd clean-exit 路径；本对账兜底崩溃
//! 与 save 失败窗口；该丢失在 hybrid 查询面零表现，唯一对账面在此）。
//! `--no-reconcile` 显式跳过（登记用）。崩溃丢失无一致性债（向量 =
//! 本地派生缓存，重跑即补齐）。**不经 Store 通道**：自持
//! [`MemoryVectorIndex`] + [`FastembedMemoryEmbedder`] 直写 artifact
//! ——daemon 侧挂载的通道为内存索引，重启（restore）后可见回填结果
//! （SPEC §2.0 派生数据语义）。
//!
//! 预算数字（SPEC §2.6 实测项口径）：落盘 artifact 体积 vs 库主文件
//! （预算 ≤1.5×）、全量嵌入耗时 vs ≤30min、done/failed/对账计数。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use partisync_graph::memory_vector::embed::FastembedMemoryEmbedder;
use partisync_graph::{MemoryEmbedder, Store};

/// 每批进度打印行数。
const PROGRESS_EVERY: usize = 1_000;

fn print_usage() {
    eprintln!(
        "用法: backfill_memory_vectors <db路径> [--artifact <memory_vec.usearch>] \
         [--model-cache <dir>] [--limit N] [--no-reconcile]\n\
         缺省: artifact = db 同目录 memory_vec.usearch；model-cache = \
         FASTEMBED_CACHE_DIR 或 .fastembed_cache；limit = 全量；\
         对账重嵌开启（done 行 contains 对账，缺失即重嵌）"
    );
}

/// 待补嵌活行选取（ADR-0035 决策 3 口径）：deleted=0 且伴表状态非 done/
/// skipped（含从未登记）。memory_id 升序（确定性输入）。
fn pending_sql(limit: Option<i64>) -> String {
    let base = "SELECT m.memory_id FROM memory m \
         WHERE m.deleted = 0 AND ( \
           NOT EXISTS (SELECT 1 FROM memory_embedding e WHERE e.memory_id = m.memory_id) \
           OR EXISTS (SELECT 1 FROM memory_embedding e \
                      WHERE e.memory_id = m.memory_id AND e.status IN (0, 1, 4))) \
         ORDER BY m.memory_id"
        .to_string();
    match limit {
        Some(n) => format!("{base} LIMIT {n}"),
        None => base,
    }
}

/// 对账候选（M11-WP06-T05 审查处置）：伴表 done 行 JOIN 活行，升序。
fn done_sql() -> &'static str {
    "SELECT m.memory_id FROM memory_embedding e \
     JOIN memory m ON m.memory_id = e.memory_id \
     WHERE e.status = 2 AND m.deleted = 0 \
     ORDER BY m.memory_id"
}

/// 单行回填结果（口径修正：行已删/墓碑 = Skipped 非 Failed——T05 审查
/// 小疵处置；Failed = 嵌入失败已记伴表 status=4）。
enum Outcome {
    Embedded,
    Failed(String),
    Skipped,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        std::process::exit(2);
    }
    let db_path = PathBuf::from(args.remove(0));
    let mut artifact: Option<PathBuf> = None;
    let mut model_cache: Option<PathBuf> = None;
    let mut limit: Option<i64> = None;
    let mut no_reconcile = false;
    let mut rest = args.into_iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--artifact" => artifact = Some(PathBuf::from(rest.next().expect("--artifact 需要值"))),
            "--model-cache" => {
                model_cache = Some(PathBuf::from(rest.next().expect("--model-cache 需要值")))
            }
            "--limit" => limit = Some(rest.next().expect("--limit 需要值").parse()?),
            "--no-reconcile" => no_reconcile = true,
            other => {
                eprintln!("未知参数: {other}");
                print_usage();
                std::process::exit(2);
            }
        }
    }
    let artifact: PathBuf = artifact.unwrap_or_else(|| {
        let dir = db_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        dir.join("memory_vec.usearch")
    });

    let store = Store::open(&db_path).await?;
    let t_init = Instant::now();
    let cache = model_cache.unwrap_or_else(|| {
        std::env::var_os("FASTEMBED_CACHE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".fastembed_cache"))
    });
    let embedder = Arc::new(FastembedMemoryEmbedder::new(Some(&cache))?);
    println!(
        "嵌入器就绪（BGESmallZHV15，cache={}，init {}ms）",
        cache.display(),
        t_init.elapsed().as_millis()
    );
    let index = partisync_graph::MemoryVectorIndex::open_or_create(&artifact)?;
    println!(
        "artifact = {}（restore 前条目 {}）",
        artifact.display(),
        index.size()
    );

    // ① 补嵌 pending/running/failed/未登记（主口径）
    let ids: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(pending_sql(limit)))
        .fetch_all(store.pool_ref())
        .await?;
    let total = ids.len();
    println!("待补嵌活行 {total}（pending/running/failed/未登记 口径）");

    let t0 = Instant::now();
    let mut done = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    for (i, id) in ids.iter().enumerate() {
        match embed_one(&store, &index, &embedder, id).await {
            Outcome::Embedded => done += 1,
            Outcome::Failed(detail) => {
                failed += 1;
                eprintln!("回填失败 {id}: {detail}");
            }
            Outcome::Skipped => skipped += 1,
        }
        if (i + 1).is_multiple_of(PROGRESS_EVERY) {
            println!(
                "进度 {}/{total}：done {done} failed {failed} skipped {skipped}，累计 {}s",
                i + 1,
                t0.elapsed().as_secs()
            );
        }
    }
    println!(
        "补嵌完成：attempted {total}，done {done}，failed {failed}，skipped {skipped}，耗时 {}s",
        t0.elapsed().as_secs()
    );

    // ② 对账重嵌（M11-WP06-T05 审查处置，ADR-0035 决策 3 崩溃恢复闭环）：
    // done 行逐 id contains 对账，缺失即重嵌——覆盖 daemon 崩溃/save 失败
    // 窗口的向量丢失（该丢失在 hybrid 查询面零表现，唯一对账面在此）。
    let mut reconciled = 0usize;
    let mut reconcile_failed = 0usize;
    if no_reconcile {
        eprintln!("--no-reconcile：跳过 done 行对账（登记用——向量丢失窗口未闭合）");
    } else {
        let done_ids: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(done_sql().to_string()))
            .fetch_all(store.pool_ref())
            .await?;
        println!("对账 done 行 {}", done_ids.len());
        let t1 = Instant::now();
        for id in &done_ids {
            if index.contains(id)? {
                continue;
            }
            match embed_one(&store, &index, &embedder, id).await {
                Outcome::Embedded => reconciled += 1,
                Outcome::Failed(detail) => {
                    reconcile_failed += 1;
                    eprintln!("对账重嵌失败 {id}: {detail}");
                }
                Outcome::Skipped => {}
            }
        }
        println!(
            "对账完成：sweep {} 行耗时 {}ms，重嵌 {reconciled}，失败 {reconcile_failed}",
            done_ids.len(),
            t1.elapsed().as_millis()
        );
    }

    index.save(&artifact)?;
    let elapsed = t0.elapsed();
    println!(
        "回填完成：耗时 {}s（预算 ≤1800s @10⁵ 叶，SPEC §2.6）",
        elapsed.as_secs()
    );

    // 预算数字（SPEC §2.6 实测项）：artifact 体积 vs 库主文件 ≤1.5×
    let artifact_bytes = std::fs::metadata(&artifact)?.len();
    let db_bytes = std::fs::metadata(&db_path)?.len();
    let ratio = if db_bytes > 0 {
        artifact_bytes as f64 / db_bytes as f64
    } else {
        0.0
    };
    println!(
        "artifact 落盘 {} B vs 库主文件 {} B = {ratio:.3}×（预算 ≤1.5×，SPEC §2.6）",
        artifact_bytes, db_bytes
    );
    let status: (i64, i64, i64) = sqlx::query_as(
        "SELECT \
           count(*) FILTER (WHERE status = 2), \
           count(*) FILTER (WHERE status = 4), \
           count(*) FROM memory_embedding",
    )
    .fetch_one(store.pool_ref())
    .await?;
    println!(
        "伴表终态：done {} failed {} 全部 {}",
        status.0, status.1, status.2
    );
    println!("artifact 条目 {}（save 后 size）", index.size());
    if failed > 0 || reconcile_failed > 0 {
        eprintln!("存在失败行（status=4 已记账，重跑本工具即重试）");
        std::process::exit(1);
    }
    Ok(())
}

/// 单行回填（嵌入 → 向量落库 → 伴表落账）。行缺失/已墓碑 = Skipped
/// （非失败——T05 审查小疵处置）；嵌入/落库失败 = Failed（已记伴表
/// status=4，回填器重试）。
async fn embed_one(
    store: &Store,
    index: &partisync_graph::MemoryVectorIndex,
    embedder: &Arc<FastembedMemoryEmbedder>,
    memory_id: &str,
) -> Outcome {
    let Some((content,)) = sqlx::query_as::<_, (String,)>(
        "SELECT content FROM memory WHERE memory_id = ? AND deleted = 0",
    )
    .bind(memory_id)
    .fetch_optional(store.pool_ref())
    .await
    .expect("读 memory 行") else {
        return Outcome::Skipped;
    };
    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0);
    let record = |status: i64, detail: String| {
        let memory_id = memory_id.to_string();
        let pool = store.pool_ref().clone();
        async move {
            sqlx::query(
                "INSERT INTO memory_embedding (memory_id, status, dim, detail, updated_ns)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(memory_id) DO UPDATE SET
                    status = excluded.status, dim = excluded.dim,
                    detail = excluded.detail, updated_ns = excluded.updated_ns",
            )
            .bind(&memory_id)
            .bind(status)
            .bind(i64::try_from(partisync_graph::MEMORY_VECTOR_DIM).unwrap_or(i64::MAX))
            .bind(detail)
            .bind(now_ns)
            .execute(&pool)
            .await
        }
    };
    match embedder.embed(&content) {
        Err(e) => {
            record(4, format!("{e}")).await.expect("伴表 failed 落账");
            Outcome::Failed(format!("{e}"))
        }
        Ok(vector) => {
            if let Err(e) = index.upsert(memory_id, &vector) {
                record(4, format!("usearch upsert: {e}"))
                    .await
                    .expect("伴表 failed 落账");
                return Outcome::Failed(format!("{e}"));
            }
            record(2, "backfilled".to_string())
                .await
                .expect("伴表 done 落账");
            Outcome::Embedded
        }
    }
}
