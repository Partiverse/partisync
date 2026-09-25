//! M5-WP07：usearch 写吞吐根因探针（SPEC docs/specs/M5-WP07.md 裁定 1）。
//! 纯 usearch API 隔离——区分「库慢」vs「wrapper 慢」。

use std::time::Instant;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

#[test]
fn t02_raw_usearch_add_rate() {
    let opts = IndexOptions {
        dimensions: 768,
        metric: MetricKind::Cos,
        quantization: ScalarKind::F32,
        ..Default::default()
    };
    let index = Index::new(&opts).unwrap();
    index.reserve(5_000).unwrap();

    let mut total = Duration::from_secs(0);
    let mut slowest = Duration::from_secs(0);
    for i in 0..5_000u64 {
        let v: Vec<f32> = (0..768)
            .map(|k| ((i + k as u64) % 1000) as f32 / 1000.0 - 0.5)
            .collect();
        let t0 = Instant::now();
        index.add(i, &v).unwrap();
        let el = t0.elapsed();
        total += el;
        if el > slowest {
            slowest = el;
        }
        if i == 100 || i == 1_000 || i == 4_999 {
            eprintln!("add {i}: {el:?} (cum avg {:?})", total / (i as u32 + 1));
        }
    }
    eprintln!(
        "5k adds total={total:?} avg={:?} slowest={slowest:?}",
        total / 5_000
    );
}

use std::time::Duration;

use partisync_index::search::engine::{IndexEngine, IndexEngineConfig};
use partisync_index::search::vector::VectorKind;

/// 10⁵ 向量补测（SPEC 裁定 4）：add_new 吞吐 + top-10 P99。
/// 运行环境：NK_TARGET 仅关 SME/F8（NEON 保留）——全 0 = 标量回退，
/// 数字作废（T02 根因）。
#[test]
fn t03_vector_100k_build_and_search() {
    let root = std::env::temp_dir().join("wp07-100k");
    let _ = std::fs::remove_dir_all(&root);
    let engine = IndexEngine::open_or_create(IndexEngineConfig {
        index_root: root,
        enable_reranker: false,
        reranker_model_dir: None,
    })
    .unwrap();

    let n = 100_000u64;
    engine
        .vector_store()
        .reserve(VectorKind::TextDense, n)
        .unwrap();
    let t0 = std::time::Instant::now();
    for i in 0..n {
        let v: Vec<f32> = (0..768)
            .map(|k| ((i + k as u64) % 1000) as f32 / 1000.0 - 0.5)
            .collect();
        engine
            .vector_store()
            .add_new(&format!("c{i:07}"), VectorKind::TextDense, &v)
            .unwrap();
    }
    let build = t0.elapsed();
    println!(
        "10⁵ add_new: {build:?}  ≈ {:.0} 条/s",
        n as f64 / build.as_secs_f64()
    );

    // top-10 查询 P50/P99（n=200）
    let mut lat = Vec::new();
    for i in 0..200u64 {
        let q: Vec<f32> = (0..768)
            .map(|k| ((i * 31 + k as u64) % 1000) as f32 / 1000.0 - 0.5)
            .collect();
        let t1 = std::time::Instant::now();
        let hits = engine.vector_only(&q, VectorKind::TextDense, 10).unwrap();
        assert_eq!(hits.len(), 10);
        lat.push(t1.elapsed());
    }
    lat.sort();
    println!("vector top-10 @10⁵: P50={:?} P99={:?}", lat[100], lat[198]);
}
