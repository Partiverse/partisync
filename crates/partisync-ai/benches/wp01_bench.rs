//! M4-WP01 基准（SPEC T07：per-stage 吞吐 / 编排开销）。
//!
//! 口径：真实代码路径（缩略图/EXIF stage、OCR-CTC 与 DB 连通域后处理、
//! 重采样、fake 嵌入/编排）。真实模型推理吞吐待冒烟（HF 不可达移交，
//! 见 m4-wp01-kpi.md §7）。
//!
//! 用法：`cargo bench -p partisync-ai`。

use std::io::Cursor;
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use partisync_ai::pipeline::{MimeKind, SidecarStage, StageError, StageInput, StageOutput};
use partisync_ai::stages::ocr::{ctc_decode, find_boxes};
use partisync_ai::stages::transcribe::resample_to_16k;
use partisync_ai::{ExifStage, FakeEmbedStage, ModelManager, ThumbnailStage};

/// 1080p 红蓝渐变 PNG（一次编码，反复解码测 stage 全链）。
fn png_1080p() -> Vec<u8> {
    png_of(1920, 1080)
}

fn png_of(w: u32, h: u32) -> Vec<u8> {
    let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(w, h, |x, y| {
        image::Rgb([(x * 255 / w.max(1)) as u8, (y * 255 / h.max(1)) as u8, 128])
    }));
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    buf.into_inner()
}

fn bench_thumbnail(c: &mut Criterion) {
    let png = png_1080p();
    let stage = ThumbnailStage::new();
    let input = StageInput {
        content_id: "bench-thumb".into(),
        mime: MimeKind::Image,
        data: png.clone(),
    };
    let mut g = c.benchmark_group("thumbnail");
    g.throughput(Throughput::Bytes(png.len() as u64));
    g.sample_size(30);
    g.bench_function("png_1080p_decode_resize_encode", |b| {
        b.iter(|| {
            let out: StageOutput = stage.run(&input).expect("thumbnail ok");
            std::hint::black_box(out.blob);
        })
    });
    g.finish();
}

fn bench_exif(c: &mut Criterion) {
    let png = png_of(640, 480);
    let stage = ExifStage::new();
    let input = StageInput {
        content_id: "bench-exif".into(),
        mime: MimeKind::Image,
        data: png,
    };
    c.bench_function("exif/png_640p_no_exif_early_exit", |b| {
        b.iter(|| {
            let r: Result<StageOutput, StageError> = stage.run(&input);
            std::hint::black_box(r.is_err())
        })
    });
}

fn bench_ocr_postprocess(c: &mut Criterion) {
    // DB 连通域：960x540 概率图、200 处文本区块
    let (w, h) = (960u32, 540u32);
    let mut prob = vec![0f32; (w * h) as usize];
    let mut seed = 0x1234_5678u64;
    let mut rng = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..200 {
        let bx = (rng() % u64::from(w - 60)) as usize;
        let by = (rng() % u64::from(h - 20)) as usize;
        for y in by..by + 16 {
            for x in bx..bx + 48 {
                prob[y * w as usize + x] = 0.9;
            }
        }
    }
    let mut g = c.benchmark_group("ocr");
    g.throughput(Throughput::Elements((w * h) as u64));
    g.bench_function("db_find_boxes_960x540_200blocks", |b| {
        b.iter(|| std::hint::black_box(find_boxes(&prob, w, h)))
    });

    // CTC：50 帧 × 6625 类（PP-OCR rec 口径），6623 字词典
    let (frames, classes) = (50usize, 6625usize);
    let logits: Vec<f32> = (0..frames * classes)
        .map(|i| ((i * 2654435761) % 1000) as f32 / 1000.0)
        .collect();
    let dict: Vec<char> = (0..6623)
        .map(|i| char::from_u32(0x4E00 + i as u32).unwrap_or('字'))
        .collect();
    g.throughput(Throughput::Elements((frames * classes) as u64));
    g.bench_function("ctc_decode_50frames_6625classes", |b| {
        b.iter(|| std::hint::black_box(ctc_decode(&logits, frames, classes, &dict)))
    });
    g.finish();
}

fn bench_transcribe_resample(c: &mut Criterion) {
    // 30 s 单声道 @44.1 kHz → 16 kHz
    let samples: Vec<f32> = (0..30 * 44_100)
        .map(|i: u64| ((i.wrapping_mul(2_654_435_761) % 65536) as f32 / 32768.0) - 1.0)
        .collect();
    let mut g = c.benchmark_group("transcribe");
    g.throughput(Throughput::Elements(samples.len() as u64));
    g.bench_function("resample_linear_30s_44k1_to_16k", |b| {
        b.iter(|| std::hint::black_box(resample_to_16k(&samples, 44_100)))
    });
    g.finish();
}

fn bench_embed_fake(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let mgr = ModelManager::new(dir.path());
    mgr.mark_present(partisync_ai::EMBED_MODEL_TEXT).unwrap();
    let stage = FakeEmbedStage::new(mgr);
    let input = StageInput {
        content_id: "bench-embed".into(),
        mime: MimeKind::Text,
        data: b"benchmark text payload".to_vec(),
    };
    c.bench_function("embed/fake_dim8_with_model_verify", |b| {
        b.iter(|| {
            let out: StageOutput = stage.run(&input).expect("fake embed ok");
            std::hint::black_box(out.blob);
        })
    });
}

fn bench_pipeline_orchestration(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    // 预置：内存库 + 1 content + 5 行全终态 done（读路径地板）
    let store = rt
        .block_on(partisync_graph::store::Store::open_in_memory())
        .unwrap();
    let stages: Vec<Arc<dyn SidecarStage>> = vec![
        Arc::new(ThumbnailStage::new()),
        Arc::new(ExifStage::new()),
        Arc::new(partisync_ai::FakeOcrStage::new(ModelManager::new(
            tempfile::tempdir().unwrap().path(),
        ))),
        Arc::new(partisync_ai::FakeTranscribeStage::new(ModelManager::new(
            tempfile::tempdir().unwrap().path(),
        ))),
        Arc::new(FakeEmbedStage::new(ModelManager::new(
            tempfile::tempdir().unwrap().path(),
        ))),
    ];
    let loader = partisync_ai::InMemoryContentLoader::new([("c1".into(), png_of(320, 240))]);
    let pipeline =
        partisync_ai::Pipeline::new(Arc::new(partisync_ai::InMemoryBlobSink::new()), stages);

    rt.block_on(async {
        sqlx::query(
            "INSERT INTO content (id, size, mime, kind) VALUES ('c1', 1, 'image/png', 'file')",
        )
        .execute(store.pool_ref())
        .await
        .unwrap();
        let sidecars = partisync_ai::SidecarStore::new(&store);
        sidecars.ensure_enqueued("c1").await.unwrap();
        pipeline.run_content(&store, &loader, "c1").await.unwrap();
    });

    let mut g = c.benchmark_group("pipeline");
    g.sample_size(30);
    // 读路径地板：全终态 rerun（无 stage 执行，纯行扫描 + no-op）
    g.bench_function("rerun_noop_1content_5stages", |b| {
        b.iter(|| rt.block_on(pipeline.run_content(&store, &loader, "c1")))
    });
    // 全驱动：复位 pending + 五 stage 全驱动（含 DB 状态机往返）
    g.bench_function("drive_1content_5stages_with_reset", |b| {
        b.iter(|| {
            rt.block_on(async {
                sqlx::query("UPDATE sidecar_items SET status = 0 WHERE content_id = 'c1'")
                    .execute(store.pool_ref())
                    .await
                    .unwrap();
                pipeline.run_content(&store, &loader, "c1").await
            })
        })
    });
    g.finish();
}

criterion_group!(
    benches,
    bench_thumbnail,
    bench_exif,
    bench_ocr_postprocess,
    bench_transcribe_resample,
    bench_embed_fake,
    bench_pipeline_orchestration
);
criterion_main!(benches);
