//! M4-WP01-T05 行为契约测试（SPEC 验收「fake 模型，CI 离线」）：
//! OCR/转写 stage 的缺失降级、在场 done、mime 适用性，以及纯函数
//! 后处理钉子（det 尺寸/连通域框/CTC 解码/重采样）。

use std::sync::Arc;

use partisync_ai::pipeline::{MimeKind, Pipeline, SidecarStage, StageError, StageOutput};
use partisync_ai::sidecar::{stage_ids, BlobSink, InMemoryBlobSink, SidecarStore};
use partisync_ai::{
    FakeOcrStage, FakeTranscribeStage, ModelManager, OCR_MODEL_DET, OCR_MODEL_REC, TRANSCRIBE_MODEL,
};
use partisync_graph::store::Store;

async fn mem_store() -> Store {
    Store::open_in_memory().await.expect("store open")
}

async fn seed_content(store: &Store, id: &str, mime: &str) {
    sqlx::query("INSERT OR IGNORE INTO content (id, size, mime, kind) VALUES (?, 1, ?, 'file')")
        .bind(id)
        .bind(mime)
        .execute(store.pool_ref())
        .await
        .expect("seed content");
}

fn temp_models() -> (tempfile::TempDir, ModelManager) {
    let dir = tempfile::tempdir().unwrap();
    let mgr = ModelManager::new(dir.path());
    (dir, mgr)
}

fn png_bytes(w: u32, h: u32) -> Vec<u8> {
    let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(w, h, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 99])
    }));
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    buf.into_inner()
}

/// OCR 缺失降级：det 缺 → skipped（model-missing），rec 缺 → 同。
#[tokio::test]
async fn ocr_model_missing_skips_with_reason() {
    let store = mem_store().await;
    seed_content(&store, "c1", "image/png").await;
    let (_d, mgr) = temp_models();
    let loader = partisync_ai::InMemoryContentLoader::new([("c1".into(), png_bytes(64, 32))]);
    let pipeline = Pipeline::new(
        Arc::new(InMemoryBlobSink::new()),
        vec![Arc::new(FakeOcrStage::new(mgr))],
    );
    let s = pipeline.run_content(&store, &loader, "c1").await.unwrap();
    assert_eq!((s.skipped, s.failed), (1, 0));
    let rows = SidecarStore::new(&store).items("c1").await.unwrap();
    let ocr = rows.iter().find(|r| r.stage == stage_ids::OCR).unwrap();
    assert_eq!(ocr.status_name(), "skipped");
    assert!(ocr
        .detail
        .as_deref()
        .unwrap_or_default()
        .starts_with("model-missing"));
}

/// OCR 半缺：det 在场 rec 缺 → 仍降级（两模型均需在场）。
#[tokio::test]
async fn ocr_partial_models_still_skip() {
    let store = mem_store().await;
    seed_content(&store, "c1", "image/png").await;
    let (_d, mgr) = temp_models();
    mgr.mark_present(OCR_MODEL_DET).unwrap();
    let loader = partisync_ai::InMemoryContentLoader::new([("c1".into(), png_bytes(64, 32))]);
    let pipeline = Pipeline::new(
        Arc::new(InMemoryBlobSink::new()),
        vec![Arc::new(FakeOcrStage::new(mgr))],
    );
    let s = pipeline.run_content(&store, &loader, "c1").await.unwrap();
    assert_eq!(s.skipped, 1);
}

/// OCR 在场 → done：伪识别文本落盘 + detail 摘要。
#[tokio::test]
async fn ocr_models_present_done() {
    let store = mem_store().await;
    seed_content(&store, "c1", "image/png").await;
    let (_d, mgr) = temp_models();
    mgr.mark_present(OCR_MODEL_DET).unwrap();
    mgr.mark_present(OCR_MODEL_REC).unwrap();
    let loader = partisync_ai::InMemoryContentLoader::new([("c1".into(), png_bytes(64, 32))]);
    let sink = Arc::new(InMemoryBlobSink::new());
    let pipeline = Pipeline::new(sink.clone(), vec![Arc::new(FakeOcrStage::new(mgr))]);
    let s = pipeline.run_content(&store, &loader, "c1").await.unwrap();
    assert_eq!((s.done, s.failed), (1, 0));
    let blob = sink.get("c1", stage_ids::OCR).unwrap().unwrap();
    assert!(!blob.is_empty());
    let rows = SidecarStore::new(&store).items("c1").await.unwrap();
    let ocr = rows.iter().find(|r| r.stage == stage_ids::OCR).unwrap();
    assert!(ocr
        .detail
        .as_deref()
        .unwrap_or_default()
        .contains("model=pp-ocr (fake)"));
}

/// 转写缺失降级 + 非 Audio 不适用。
#[tokio::test]
async fn transcribe_missing_and_applicability() {
    let store = mem_store().await;
    seed_content(&store, "a1", "audio/mpeg").await;
    seed_content(&store, "v1", "video/mp4").await;
    let (_d, mgr) = temp_models();
    let fake = Arc::new(FakeTranscribeStage::new(mgr));
    assert!(fake.applicable(&MimeKind::Audio));
    assert!(
        !fake.applicable(&MimeKind::Video),
        "视频解封装不支持（披露）"
    );
    let loader = partisync_ai::InMemoryContentLoader::new([
        ("a1".into(), b"mp3-bytes".to_vec()),
        ("v1".into(), b"mp4-bytes".to_vec()),
    ]);
    let pipeline = Pipeline::new(Arc::new(InMemoryBlobSink::new()), vec![fake]);
    let s = pipeline.run_content(&store, &loader, "a1").await.unwrap();
    assert_eq!((s.skipped, s.failed), (1, 0), "模型缺失降级");
    let s2 = pipeline.run_content(&store, &loader, "v1").await.unwrap();
    assert_eq!(s2.skipped, 1, "video → not-applicable");
    let rows = SidecarStore::new(&store).items("a1").await.unwrap();
    let tr = rows
        .iter()
        .find(|r| r.stage == stage_ids::TRANSCRIBE)
        .unwrap();
    assert!(tr
        .detail
        .as_deref()
        .unwrap_or_default()
        .starts_with("model-missing"));
}

/// 转写在场 → done：伪转写字节级稳定（幂等）。
#[tokio::test]
async fn transcribe_present_done_deterministic() {
    let store = mem_store().await;
    seed_content(&store, "a1", "audio/mpeg").await;
    let (_d, mgr) = temp_models();
    mgr.mark_present(TRANSCRIBE_MODEL).unwrap();
    let loader = partisync_ai::InMemoryContentLoader::new([("a1".into(), b"mp3-bytes".to_vec())]);
    let sink = Arc::new(InMemoryBlobSink::new());
    let pipeline = Pipeline::new(sink.clone(), vec![Arc::new(FakeTranscribeStage::new(mgr))]);
    let s = pipeline.run_content(&store, &loader, "a1").await.unwrap();
    assert_eq!((s.done, s.failed), (1, 0));
    let b1 = sink.get("a1", stage_ids::TRANSCRIBE).unwrap().unwrap();
    // 强制重跑
    sqlx::query("UPDATE sidecar_items SET status = 0 WHERE content_id = 'a1'")
        .execute(store.pool_ref())
        .await
        .unwrap();
    pipeline.run_content(&store, &loader, "a1").await.unwrap();
    let b2 = sink.get("a1", stage_ids::TRANSCRIBE).unwrap().unwrap();
    assert_eq!(b1, b2, "同输入伪转写字节级稳定");
}

// ─── 纯函数后处理钉子 ───────────────────────────────────────────

/// det 尺寸：960 封顶 + 32 对齐。
#[test]
fn det_resize_dims_contract() {
    assert_eq!(
        partisync_ai::stages::ocr::det_resize_dims(4000, 3000),
        (960, 736)
    );
    assert_eq!(
        partisync_ai::stages::ocr::det_resize_dims(640, 480),
        (640, 480)
    );
    assert_eq!(partisync_ai::stages::ocr::det_resize_dims(1, 1), (32, 32));
    let (w, h) = partisync_ai::stages::ocr::det_resize_dims(1920, 1080);
    assert_eq!(w % 32, 0);
    assert_eq!(h % 32, 0);
    assert!(w.max(h) <= 960);
}

/// 连通域框：合成概率图两处高亮 → 两框、按阅读序排列。
#[test]
fn find_boxes_finds_two_regions() {
    let (w, h) = (32u32, 32u32);
    let mut prob = vec![0f32; (w * h) as usize];
    // 区块 A (2,2)-(9,9)，区块 B (20,20)-(27,27)
    for y in 2..10 {
        for x in 2..10 {
            prob[y * w as usize + x as usize] = 0.9;
        }
    }
    for y in 20..28 {
        for x in 20..28 {
            prob[y * w as usize + x as usize] = 0.9;
        }
    }
    let boxes = partisync_ai::stages::ocr::find_boxes(&prob, w, h);
    assert_eq!(boxes.len(), 2);
    assert!(boxes[0].y0 <= boxes[1].y0, "阅读序（上→下）");
    assert_eq!((boxes[0].x0, boxes[0].y0), (2, 2));
    assert_eq!((boxes[1].x1, boxes[1].y1), (28, 28));
}

/// CTC 解码：去重连续 + 跳 blank + 词典映射 + 空格类。
#[test]
fn ctc_decode_contract() {
    // 词典 ['你','好']；类数 4：0=blank,1='你',2='好',3=' '
    let dict = ['你', '好'];
    let classes = 4usize;
    let frames = 6usize;
    let mut logits = vec![0f32; frames * classes];
    let mut put = |t: usize, cls: usize, v: f32| logits[t * classes + cls] = v;
    put(0, 1, 1.0); // 你
    put(1, 1, 1.0); // 你（连续重复 → 合并）
    put(2, 0, 1.0); // blank
    put(3, 1, 1.0); // 你（blank 后重复 → 保留）
    put(4, 2, 1.0); // 好
    put(5, 3, 1.0); // 空格类
    let text = partisync_ai::stages::ocr::ctc_decode(&logits, frames, classes, &dict);
    assert_eq!(text, "你你好 ", "blank 分隔的同帧重复保留");
}

/// 重采样：16k 直通；8k → 长度翻倍。
#[test]
fn resample_contract() {
    let s16k: Vec<f32> = vec![0.1; 160];
    assert_eq!(
        partisync_ai::stages::transcribe::resample_to_16k(&s16k, 16_000).len(),
        160,
        "同率直通"
    );
    let s8k: Vec<f32> = vec![0.5; 80];
    assert_eq!(
        partisync_ai::stages::transcribe::resample_to_16k(&s8k, 8_000).len(),
        160,
        "8k → 16k 长度翻倍"
    );
}

/// rec 宽度：等比 + clamp。
#[test]
fn rec_target_w_contract() {
    use partisync_ai::stages::ocr::rec_target_w;
    assert_eq!(rec_target_w(96, 48), 96);
    assert_eq!(rec_target_w(480, 48), 320, "clamp 上限");
    assert_eq!(rec_target_w(10, 100), 5);
}

/// StageError 语义哨兵（Skipped/Failed 可区分）。
#[test]
fn stage_error_variants() {
    let s = StageError::Skipped("m".into());
    let f = StageError::Failed("b".into());
    assert_eq!(format!("{s:?}"), "Skipped(\"m\")");
    assert_eq!(format!("{f:?}"), "Failed(\"b\")");
    let _ = StageOutput::default();
}
