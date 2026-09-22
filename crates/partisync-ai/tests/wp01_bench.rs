//! M4-WP01-T07 去重收益测量（SPEC KPI「去重收益 unique 计数」）：
//! 合成根目录（重复组 + 独立文件）走真 indexer → sidecar 自动入队，
//! 输出 entry 数 vs 唯一 content 数与 sidecar 计算节省比例。
//! 断言从宽（防 CI 抖动）；数字供 m4-wp01-kpi.md §5 登记。

use std::path::Path;

use partisync_ai::sidecar::SidecarStore;
use partisync_cas::ChunkStore;
use partisync_graph::indexer::index_path_job;
use partisync_graph::store::Store;

fn png_of(w: u32, h: u32, seed: u8) -> Vec<u8> {
    let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(w, h, |x, y| {
        image::Rgb([x as u8 ^ seed, y as u8 ^ seed, seed])
    }));
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    buf.into_inner()
}

fn seed_root(root: &Path) {
    // 10 组重复 × 5 份（每组同内容）+ 70 份独立 = 120 entry / 80 content
    std::fs::create_dir_all(root).unwrap();
    let mut n = 0;
    for group in 0..10u8 {
        let bytes = png_of(96, 72, group + 1);
        for _ in 0..5 {
            std::fs::write(root.join(format!("dup{group:02}_{n:03}.png")), &bytes).unwrap();
            n += 1;
        }
    }
    for i in 0..70u8 {
        std::fs::write(root.join(format!("uniq{i:03}.png")), png_of(96, 72, i + 11)).unwrap();
        n += 1;
    }
    assert_eq!(n, 120);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dedup_savings_unique_counts() {
    let root = tempfile::tempdir().unwrap();
    seed_root(root.path());
    let store = Store::open_in_memory().await.unwrap();
    let cas = ChunkStore::open_in_memory(&tempfile::tempdir().unwrap().keep())
        .await
        .unwrap();
    let report = index_path_job(&store, Some(&cas), root.path(), None)
        .await
        .expect("index");
    assert_eq!(report.files, 120, "entry 数");

    let enqueued = partisync_ai::sidecar_auto_enqueue(&store).await.unwrap();
    let stats = SidecarStore::new(&store).stats().await.unwrap();
    let contents = stats.pending / partisync_ai::sidecar::STAGE_ORDER.len() as u64; // stage 行/content
    let logical = 120f64;
    let unique = contents as f64;
    let saved = (1.0 - unique / logical) * 100.0;

    println!(
        "DEDUP: entries={} unique_contents={} enqueued={enqueued}",
        report.files, contents
    );
    println!("DEDUP: sidecar_compute_saving={saved:.1}%");

    assert_eq!(contents, 80, "10 组 ×5 重复合并 → 80 唯一 content");
    assert_eq!(enqueued, 80);
    assert!(saved > 30.0 && saved < 40.0, "节省 ≈33%，实测 {saved:.1}%");
}
