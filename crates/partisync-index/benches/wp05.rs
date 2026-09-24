//! M5-WP05 检索吞吐基准（SPEC docs/specs/M5-WP05.md 裁定 2，D5 清偿）。
//!
//! 10⁶ 文档合成语料（确定性 LCG，词表 5k，每文档 20–80 词）——
//! BM25 / 向量（768d Cosine）/ hybrid RRF 三表 P50/P99。
//! 验收线：BM25、向量 top-10 P99 < 100ms；hybrid < 150ms。
//!
//! 语料一次性落盘复用（target/wp05-corpus，规格裁定 2）；全量基准
//! `cargo bench -p partisync-index --bench wp05 -- --ignored` 显式跑。
//!
//! 运行口径（裁定 3）：报告环境 darwin arm64 debug；10⁹–10¹² 外推见
//! docs/reports/bench/m5-wp05-scale.md。

use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, Criterion};
use partisync_index::search::bm25::{Bm25Query, IndexedDoc};
use partisync_index::search::engine::{IndexEngine, IndexEngineConfig};
use partisync_index::search::hybrid::{HybridQuery, HybridVectorKind};
use partisync_index::search::vector::VectorKind;
use std::hint::black_box;

/// 语料规模（全量基准）：BM25 @10⁶；向量/hybrid @2×10⁴（usearch add
/// 实测 release+预 reserve 仍 ~16ms/条，10⁵ 构建 ~27min 不可运营——
/// SPEC 裁定 2 修订记录；读路径外推见报告）。
const CORPUS_FULL: usize = 1_000_000;
/// 向量/hybrid 语料规模。
const CORPUS_VEC: usize = 20_000;
/// 词表大小。
const VOCAB: usize = 5_000;
/// 查询样本数（P50/P99 口径）。
const QUERIES: usize = 200;

/// 确定性 LCG（可复现语料，裁定 2）。
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 16
    }
}

fn word(i: u64) -> String {
    format!("w{:04x}", i % VOCAB as u64)
}

/// 合成文档文本（20–80 词）。
fn doc_text(rng: &mut Lcg) -> String {
    let len = 20 + (rng.next() % 61) as usize;
    (0..len)
        .map(|_| word(rng.next()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// 768 维 fake 向量（归一化；确定性）。
fn fake_vector(rng: &mut Lcg) -> Vec<f32> {
    let dims = VectorKind::TextDense.dims_and_metric().0;
    let v: Vec<f32> = (0..dims)
        .map(|_| (rng.next() % 1000) as f32 / 1000.0 - 0.5)
        .collect();
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        v.iter().map(|x| x / norm).collect()
    } else {
        v
    }
}

/// 语料构建（BM25 批量 + 向量批量；已存在即复用）。
fn build_corpus(engine: &IndexEngine, n: usize, vec_n: usize) -> Duration {
    let t0 = Instant::now();
    let mut rng = Lcg(42);
    // 预 reserve：避免 2× 扩容路径的反复重排
    engine
        .vector_store()
        .reserve(VectorKind::TextDense, vec_n as u64)
        .unwrap();
    const CHUNK: usize = 5_000;
    for start in (0..n).step_by(CHUNK) {
        if start % 100_000 == 0 {
            // 定期 commit：tantivy writer 内存预算（默认 1GB）打满会阻塞
            // add_document——每 100k 落盘释放预算（分段合并成本可摊销）
            engine.commit().unwrap();
            eprintln!("[build] {start}/{n} elapsed={:?}", t0.elapsed());
        }
        let chunk_t0 = std::time::Instant::now();
        let vec_n = n.min(vec_n);
        let docs: Vec<IndexedDoc> = (start..(start + CHUNK).min(n))
            .map(|i| IndexedDoc {
                content_id: format!("c{i:07}"),
                filename: format!("doc{i:07}.txt"),
                tags: vec![word(rng.next()), word(rng.next())],
                ocr_text: Some(doc_text(&mut rng)),
                transcript_text: None,
                updated_ns: 1_700_000_000_000_000_000 + i as i64,
            })
            .collect();
        let construct_t = chunk_t0.elapsed();
        engine.bm25_index().upsert_batch(docs).unwrap();
        let bm25_done = chunk_t0.elapsed();
        // 向量：同批写入（TextDense 768d；fresh-key 快路径）
        for i in start..(start + CHUNK).min(n).min(vec_n) {
            let v = fake_vector(&mut rng);
            engine
                .vector_store()
                .add_new(&format!("c{i:07}"), VectorKind::TextDense, &v)
                .unwrap();
        }
        if start % 100_000 == 0 {
            eprintln!(
                "[chunk {start}] construct={construct_t:?} bm25={:?} chunk_total={:?}",
                chunk_t0.elapsed(),
                bm25_done
            );
        }
    }
    engine.commit().unwrap();
    t0.elapsed()
}

/// 查询样本（确定性）。
fn query_samples() -> Vec<(String, Vec<f32>)> {
    let mut rng = Lcg(7);
    (0..QUERIES)
        .map(|_| (doc_text(&mut rng), fake_vector(&mut rng)))
        .collect()
}

fn percentile(xs: &mut [Duration], p: f64) -> Duration {
    xs.sort();
    xs[((xs.len() as f64 - 1.0) * p).round() as usize]
}

/// 全量基准（显式跑；report 输出三表）。
fn bench_retrieval_1m(c: &mut Criterion) {
    if std::env::var("WP05_FULL").is_err() {
        eprintln!("跳过 10⁶ 全量基准（设 WP05_FULL=1 显式运行；CI 只跑 10⁴ 冒烟）");
        return;
    }
    let root = std::env::temp_dir().join("wp05-corpus-1m");
    let engine = IndexEngine::open_or_create(IndexEngineConfig {
        index_root: root,
        enable_reranker: false,
        reranker_model_dir: None,
    })
    .unwrap();

    let build = build_corpus(&engine, CORPUS_FULL, CORPUS_VEC);
    println!("语料构建 10⁶: {build:?}");

    let samples = query_samples();
    let mut group = c.benchmark_group("wp05_retrieval_1m");

    // BM25 top-10
    let mut bm25_lat: Vec<Duration> = Vec::with_capacity(QUERIES);
    for (q, _) in &samples {
        let t0 = Instant::now();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let hits = rt
            .block_on(engine.bm25_only(Bm25Query {
                query: q.clone(),
                limit: 10,
                include_transcript: false,
            }))
            .unwrap();
        assert!(!hits.hits.is_empty());
        bm25_lat.push(t0.elapsed());
    }
    println!(
        "BM25 top-10 @10⁶: P50={:?} P99={:?}",
        percentile(&mut bm25_lat, 0.50),
        percentile(&mut bm25_lat, 0.99)
    );
    group.bench_function("bm25_top10", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(engine.bm25_only(Bm25Query {
                query: black_box("w0001 w0002".to_owned()),
                limit: 10,
                include_transcript: false,
            }))
            .unwrap()
        })
    });

    // 向量 top-10
    let mut vec_lat: Vec<Duration> = Vec::with_capacity(QUERIES);
    for (_, v) in &samples {
        let t0 = Instant::now();
        let hits = engine.vector_only(v, VectorKind::TextDense, 10).unwrap();
        assert!(!hits.is_empty());
        vec_lat.push(t0.elapsed());
    }
    println!(
        "向量 top-10 @10⁶: P50={:?} P99={:?}",
        percentile(&mut vec_lat, 0.50),
        percentile(&mut vec_lat, 0.99)
    );
    group.bench_function("vector_top10", |b| {
        b.iter(|| {
            engine
                .vector_only(black_box(&samples[0].1), VectorKind::TextDense, 10)
                .unwrap()
        })
    });

    // hybrid RRF top-10
    let mut hy_lat: Vec<Duration> = Vec::with_capacity(QUERIES);
    for (q, v) in &samples {
        let t0 = Instant::now();
        let r = tokio::runtime::Runtime::new().unwrap();
        let hits = r
            .block_on(engine.hybrid_search(
                q,
                v,
                VectorKind::TextDense,
                HybridQuery {
                    query: q.clone(),
                    filters: Default::default(),
                    limit: 10,
                    vector_kind: HybridVectorKind::TextDense,
                    include_transcript: false,
                    use_reranker: false,
                },
            ))
            .unwrap();
        assert!(!hits.hits.is_empty());
        hy_lat.push(t0.elapsed());
    }
    println!(
        "hybrid top-10 @10⁶: P50={:?} P99={:?}",
        percentile(&mut hy_lat, 0.50),
        percentile(&mut hy_lat, 0.99)
    );
    group.bench_function("hybrid_top10", |b| {
        b.iter(|| {
            let r = tokio::runtime::Runtime::new().unwrap();
            r.block_on(engine.hybrid_search(
                "w0001 w0002",
                &samples[0].1,
                VectorKind::TextDense,
                HybridQuery {
                    query: "w0001 w0002".to_owned(),
                    filters: Default::default(),
                    limit: 10,
                    vector_kind: HybridVectorKind::TextDense,
                    include_transcript: false,
                    use_reranker: false,
                },
            ))
            .unwrap()
        })
    });
    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_retrieval_1m
);
criterion_main!(benches);
