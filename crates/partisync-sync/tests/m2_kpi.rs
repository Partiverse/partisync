//! M2 关门 KPI 端到端基准（SPEC M2-WP00 §关门 KPI）：10⁵ 文件差异同步收敛 <5min。
//!
//! 默认 `#[ignore]`（KPI 基准不进常规 CI——搭建成本分钟级）；运行：
//! ```text
//! cargo test -p partisync-sync --test m2_kpi -- --ignored --nocapture
//! ```
//! 规模可调（环境变量）：`PARTISYNC_KPI_FILES`（默认 100_000）、
//! `PARTISYNC_KPI_DELTA`（默认 1_000，≈1% 差异）。
//!
//! 口径：初始全量同步（搭建）后，A 端改 DELTA 个文件，测 bisync 收敛耗时
//! （差异同步 = oplog 增量 push/pull；reconcile 快路径为稳态校验，不计入 KPI）。

use std::time::Instant;

use partisync_core::Ulid;
use partisync_graph::store::{EntryKind, FileInsert, Store};
use partisync_sync::{capture, reconcile, session};

fn kpi_files() -> u64 {
    std::env::var("PARTISYNC_KPI_FILES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100_000)
}

fn kpi_delta() -> u64 {
    std::env::var("PARTISYNC_KPI_DELTA")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000)
}

async fn node(tag: &str, device: &str) -> Store {
    let dir = std::env::temp_dir().join(format!("m2kpi-{tag}-{}", Ulid::now()));
    std::fs::create_dir_all(&dir).unwrap();
    let s = Store::open(&dir.join("t.db")).await.unwrap();
    s.seed_device_volume(device, device, device).await.unwrap();
    s
}

async fn make_files(s: &Store, root: &str, prefix: &str, n: u64, salt: u64) {
    for chunk in 0..n.div_ceil(5_000) {
        let mut batch = Vec::new();
        let lo = chunk * 5_000;
        let hi = lo + 5_000.min(n - lo);
        for i in lo..hi {
            let path = format!("/{prefix}{i}");
            batch.push(FileInsert {
                id: Ulid::now().to_string(),
                parent_id: Some(root.to_string()),
                name: format!("{prefix}{i}"),
                path: path.clone(),
                size: 1024 + i,
                mtime_ns: 1,
                content: Some((format!("H{salt}-{i}"), 1024 + i)),
                chunk_root: None,
            });
        }
        s.add_file_batch(&batch).await.unwrap();
    }
    // watch 路径语义：批量落库后逐路径捕获（oplog = 差异同步的增量源）
    for i in 0..n {
        capture::record_entry_upsert(s, &format!("/{prefix}{i}"))
            .await
            .unwrap();
    }
}

/// KPI：10⁵ 文件差异同步收敛 <5min（LAN）。
#[tokio::test]
#[ignore = "M2 关门 KPI 基准（分钟级搭建成本）——cargo test -- --ignored"]
async fn kpi_differential_sync_convergence() {
    let (n, delta) = (kpi_files(), kpi_delta());
    let a = node("a", "dev-a").await;
    let b = node("b", "dev-b").await;
    let root_a = a
        .add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None, None)
        .await
        .unwrap();
    let _root_b = b
        .add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None, None)
        .await
        .unwrap();

    // ---- 搭建（不计入 KPI）：N 文件 + 全量首同步 ----
    let t_setup = Instant::now();
    make_files(&a, &root_a, "f", n, 1).await;
    session::push(&a, &b).await.unwrap();
    println!("[setup] {n} files + full push: {:?}", t_setup.elapsed());

    // ---- KPI 测量：A 改 delta 个文件（删+重写 = 变更）→ bisync 差异同步收敛 ----
    let t_mutate = Instant::now();
    let mut hashes = Vec::with_capacity(delta as usize);
    for i in (0..delta).map(|i| i * (n / delta.max(1)).max(1)) {
        hashes.push(format!("H2-{i}"));
    }
    for (k, i) in (0..delta)
        .map(|i| i * (n / delta.max(1)).max(1))
        .enumerate()
    {
        let path = format!("/f{i}");
        a.remove_entry(&path).await.unwrap();
        capture::record_entry_remove(&a, &path).await.unwrap();
        a.add_entry(
            Some(&root_a),
            &path[1..],
            &path,
            EntryKind::File,
            4096 + i,
            0,
            Some((hashes[k].as_str(), 4096 + i)),
            None,
        )
        .await
        .unwrap();
        capture::record_entry_upsert(&a, &path).await.unwrap();
    }
    println!("[mutate] {delta} changes: {:?}", t_mutate.elapsed());

    let t_sync = Instant::now();
    let stats = session::bisync(&a, &b, session::BisyncOpts::default())
        .await
        .unwrap();
    let sync_elapsed = t_sync.elapsed();
    println!(
        "[KPI] differential bisync: {:?} (pushed={} pulled={})",
        sync_elapsed, stats.pushed, stats.pulled
    );

    // 收敛校验：delta 抽样 + reconcile 稳态快路径
    for i in (0..delta).map(|i| i * (n / delta.max(1)).max(1)) {
        let path = format!("/f{i}");
        let ha = a.entry_by_path(&path).await.unwrap().unwrap();
        let hb = b.entry_by_path(&path).await.unwrap().unwrap();
        assert_eq!(ha.content_id, hb.content_id, "差异同步后 {path} 等值");
    }
    let rec = reconcile::reconcile(&a, &b, reconcile::ReconcileOpts::default())
        .await
        .unwrap();
    println!(
        "[verify] reconcile: rounds={} fast_path={}",
        rec.rounds, rec.fast_path
    );

    assert!(
        sync_elapsed < std::time::Duration::from_secs(300),
        "KPI 未达标：差异同步 {sync_elapsed:?} ≥ 5min"
    );
}
