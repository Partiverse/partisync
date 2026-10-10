//! M11-WP05-T04 T1 语料积累：LCSTS 真实语料批量导入器（dev-target 工具，
//! examples 不入产品依赖图、零 Cargo.toml 变更、零产品 API 面）。
//!
//! 导入口径 = `tests/wp02_memory.rs` perf 探针判例：事务分批 + 裸 INSERT
//! memory 9 列（hlc = NULL、oplog 零行），结束后**一次**
//! [`Store::refresh_memory_root`] 全量重算。身份/列级哈希与产品写路径同源：
//! [`memory_identity`] / [`content_digest`]（与 `store.rs` `memory_write`
//! 同一函数面）；幂等 = INSERT OR IGNORE（同 content+tags+metadata 撞
//! memory_id 主键时忽略，与 `memory_write` dedup 语义一致）。
//!
//! 用法:
//!   cargo run --release -p partisync-graph --example import_lcsts -- \
//!       <lcsts.jsonl> <db路径> [最大条数，默认 120000]
//!
//! JSONL 每行一个 JSON object；content 取键 content/text/article（首个非空，
//! 沿 scripts/prepare-lcsts.py CONTENT_KEYS 键表），无 content 的行跳过。

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use partisync_graph::memory::{canonical_json, canonical_tags, content_digest, memory_identity};
use partisync_graph::Store;
use serde_json::Value;

/// 每批事务行数（wp02 perf 判例同量级分批，避免单事务过大）。
const BATCH: usize = 10_000;
/// origin_device 记名（wp02 判例 'dev-a' 同位；memory 表对 device 无 FK，
/// 直插不要求设备登记）。
const ORIGIN_DEVICE: &str = "dev-a";
/// tags = ["lcsts"]（canonical JSON array 一次算好，全量行相同）。
const TAGS_LCSTS: &str = "[\"lcsts\"]";
/// created_ns 确定性基线 + 行序号（wp02 perf 判例同口径，避免墙钟依赖）。
const CREATED_NS_BASE: i64 = 1_700_000_000_000_000_000;
/// 默认最大导入条数（T1 判定口径 ≥10⁵，留出幂等重跑余量）。
const DEFAULT_MAX_ROWS: usize = 120_000;

/// content 字段候选键（prepare-lcsts.py CONTENT_KEYS 同表）。
const CONTENT_KEYS: [&str; 3] = ["content", "text", "article"];

/// 从 JSON object 按候选键序取首个非空字符串字段。
fn pick<'a>(row: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|k| row.get(*k).and_then(Value::as_str))
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("用法: import_lcsts <lcsts.jsonl> <db路径> [最大条数，默认 120000]");
        std::process::exit(2);
    }
    let jsonl_path = Path::new(&args[0]);
    let db_path = Path::new(&args[1]);
    let max_rows: usize = match args.get(2) {
        Some(s) => s.parse()?,
        None => DEFAULT_MAX_ROWS,
    };

    let store = Store::open(db_path).await?;
    let file = std::fs::File::open(jsonl_path)?;
    let reader = BufReader::new(file);
    // metadata = {}（canonical JSON object；memory_identity 第三参要求 object 形态）
    let meta_c = canonical_json(&Value::Object(serde_json::Map::new()));
    // canonical_tags(["lcsts"]) 编译期口径对齐（release 下零成本断言可保留）
    debug_assert_eq!(canonical_tags(&["lcsts".to_string()]), TAGS_LCSTS);

    let mut attempted = 0usize; // content 非空、已尝试插入的行数
    let mut inserted = 0usize; // INSERT OR IGNORE 实际落行数
    let mut lines_seen = 0usize;
    let t0 = Instant::now();
    let mut tx = store.pool_ref().begin().await?;

    for line in reader.lines() {
        let line = line?;
        lines_seen += 1;
        let row: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue, // 坏行跳过（语料仍真实，计数留痕）
        };
        let Some(content) = pick(&row, &CONTENT_KEYS) else {
            continue;
        };
        let id = memory_identity(content, TAGS_LCSTS, &meta_c);
        let res = sqlx::query(
            "INSERT OR IGNORE INTO memory
                (memory_id, content, content_hash, tags, metadata, created_ns, origin_device, hlc, deleted)
             VALUES (?, ?, ?, ?, ?, ?, ?, NULL, 0)",
        )
        .bind(&id)
        .bind(content)
        .bind(content_digest(content))
        .bind(TAGS_LCSTS)
        .bind(&meta_c)
        .bind(CREATED_NS_BASE + attempted as i64)
        .bind(ORIGIN_DEVICE)
        .execute(&mut *tx)
        .await?;
        attempted += 1;
        inserted += res.rows_affected() as usize;
        if attempted.is_multiple_of(BATCH) {
            tx.commit().await?;
            println!(
                "进度: 尝试 {attempted} / 实插 {inserted} / 累计 {elapsed}s",
                elapsed = t0.elapsed().as_secs()
            );
            tx = store.pool_ref().begin().await?;
        }
        if attempted >= max_rows {
            break;
        }
    }
    tx.commit().await?;

    let in_table: i64 = sqlx::query_scalar("SELECT count(*) FROM memory")
        .fetch_one(store.pool_ref())
        .await?;
    println!("行扫描 {lines_seen}，尝试插入 {attempted}，INSERT OR IGNORE 实插 {inserted}");
    println!("memory 表 count(*) = {in_table}");

    let t_refresh = Instant::now();
    let snap = store.refresh_memory_root().await?;
    let refresh_ms = t_refresh.elapsed().as_millis();
    println!(
        "memory_root: count = {} root = {}（refresh 耗时 {refresh_ms}ms）",
        snap.memory_count, snap.root
    );

    let report = store.verify_memory().await?;
    println!(
        "verify_memory: ok = {} snapshot_root = {:?} recomputed_root = {} count = {} tombstones = {} content_mismatches = {}",
        report.ok,
        report.snapshot_root,
        report.recomputed_root,
        report.memory_count,
        report.tombstones,
        report.content_mismatches.len()
    );
    if !report.ok {
        eprintln!("verify_memory 未通过");
        std::process::exit(1);
    }
    Ok(())
}
