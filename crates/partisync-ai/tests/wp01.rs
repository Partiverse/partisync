//! M4-WP01-T02 行为契约测试（SPEC M4-WP01 验收标准）：
//! content 去重、stage 幂等、崩溃恢复复位、模型缺失降级、失败不中断、
//! 缩略图/EXIF 实装契约、作业终态。CI 离线（无模型、无网络）。

use std::io::Cursor;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use partisync_ai::pipeline::{
    ContentLoader, InMemoryContentLoader, MimeKind, Pipeline, SidecarStage, StageError, StageInput,
    StageOutput,
};
use partisync_ai::sidecar::{
    stage_ids, BlobSink, InMemoryBlobSink, ItemStatus, SidecarStore, STAGE_ORDER,
};
use partisync_ai::{default_stages, run_sidecar_job};
use partisync_graph::store::Store;

async fn mem_store() -> Store {
    Store::open_in_memory().await.expect("store open")
}

/// 建一个 content 行（最小列；FK 由 sidecar_items 消费）。
async fn seed_content(store: &Store, id: &str, mime: &str) {
    sqlx::query("INSERT OR IGNORE INTO content (id, size, mime, kind) VALUES (?, 1, ?, 'file')")
        .bind(id)
        .bind(mime)
        .execute(store.pool_ref())
        .await
        .expect("seed content");
}

/// 100x60 红蓝渐变 PNG（确定字节）。
fn png_bytes(w: u32, h: u32) -> Vec<u8> {
    let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(w, h, |x, y| {
        image::Rgb([(x * 255 / w.max(1)) as u8, (y * 255 / h.max(1)) as u8, 128])
    }));
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("png encode");
    buf.into_inner()
}

/// 计数 fake stage（钉「每阶段恰执行一次」）。
struct CountingStage {
    id: &'static str,
    mime: MimeKind,
    behavior: Behavior,
    calls: AtomicUsize,
}

enum Behavior {
    Ok,
    Skipped,
    Failed,
}

impl CountingStage {
    fn new(id: &'static str, mime: MimeKind, behavior: Behavior) -> Arc<Self> {
        Arc::new(Self {
            id,
            mime,
            behavior,
            calls: AtomicUsize::new(0),
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl SidecarStage for CountingStage {
    fn stage(&self) -> &'static str {
        self.id
    }

    fn applicable(&self, mime: &MimeKind) -> bool {
        *mime == self.mime
    }

    fn run(&self, _input: &StageInput) -> Result<StageOutput, StageError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.behavior {
            Behavior::Ok => Ok(StageOutput {
                blob: Some(b"deterministic-blob".to_vec()),
                detail: Some("fake".into()),
                ..StageOutput::default()
            }),
            Behavior::Skipped => Err(StageError::Skipped("模型缺失（fake）".into())),
            Behavior::Failed => Err(StageError::Failed("boom（fake）".into())),
        }
    }
}

async fn run(
    store: &Store,
    stages: Vec<Arc<dyn SidecarStage>>,
    loader: &dyn ContentLoader,
    content_id: &str,
) -> partisync_ai::PipelineSummary {
    let pipeline = Pipeline::new(Arc::new(InMemoryBlobSink::new()), stages);
    pipeline
        .run_content(store, loader, content_id)
        .await
        .expect("run_content")
}

/// 验收「content 去重」：同 content 重复入队 + 管线重跑 → 每 stage 恰执行一次。
#[tokio::test]
async fn content_dedup_stage_runs_once() {
    let store = mem_store().await;
    seed_content(&store, "c1", "image/png").await;
    // 两个 entry 指向同一 content（去重锚点语义；此处入队层验证幂等）
    let sidecars = SidecarStore::new(&store);
    sidecars.ensure_enqueued("c1").await.unwrap();
    sidecars.ensure_enqueued("c1").await.unwrap();
    let stats = sidecars.stats().await.unwrap();
    assert_eq!(stats.pending, 5, "重复入队 no-op（主键保证）");

    let thumb = CountingStage::new(stage_ids::THUMBNAIL, MimeKind::Image, Behavior::Ok);
    let loader = InMemoryContentLoader::new([("c1".into(), png_bytes(64, 32))]);
    let s1 = run(&store, vec![thumb.clone()], &loader, "c1").await;
    assert_eq!((s1.done, s1.skipped), (1, 0), "未注册 stage 保持 pending");
    let s2 = run(&store, vec![thumb.clone()], &loader, "c1").await;
    assert_eq!((s2.done, s2.skipped, s2.failed), (0, 0, 0), "重跑全 no-op");
    assert_eq!(thumb.calls(), 1, "stage 恰执行一次");
    let stats = SidecarStore::new(&store).stats().await.unwrap();
    assert_eq!(stats.pending, 4, "未注册 stage 待后续模型进场补算");
}

/// 验收「阶段幂等」：强制重试（pending 复位）重执行，产物字节级稳定、不重复入队。
#[tokio::test]
async fn stage_idempotent_rerun_bytes_stable() {
    let store = mem_store().await;
    seed_content(&store, "c1", "image/png").await;
    let sink = Arc::new(InMemoryBlobSink::new());
    let thumb = CountingStage::new(stage_ids::THUMBNAIL, MimeKind::Image, Behavior::Ok);
    let loader = InMemoryContentLoader::new([("c1".into(), png_bytes(64, 32))]);
    let pipeline = Pipeline::new(sink.clone(), vec![thumb.clone()]);
    pipeline.run_content(&store, &loader, "c1").await.unwrap();
    let a1 = sink.get("c1", stage_ids::THUMBNAIL).unwrap().unwrap();

    // 强制重试：复位为 pending 再跑
    sqlx::query("UPDATE sidecar_items SET status = 0 WHERE content_id = 'c1'")
        .execute(store.pool_ref())
        .await
        .unwrap();
    let s = pipeline.run_content(&store, &loader, "c1").await.unwrap();
    assert_eq!(s.done, 1, "重试重执行");
    let a2 = sink.get("c1", stage_ids::THUMBNAIL).unwrap().unwrap();
    assert_eq!(a1, a2, "同输入产物字节级稳定");
    assert_eq!(thumb.calls(), 2);
    let sidecars = SidecarStore::new(&store);
    assert_eq!(sidecars.stats().await.unwrap().done, 1, "不重复入队");
}

/// 验收「崩溃恢复」：模拟崩溃残留 running → 复位 → 重跑完成，终态与全量直算一致。
#[tokio::test]
async fn crash_recovery_running_reset() {
    let store = mem_store().await;
    seed_content(&store, "c1", "image/png").await;
    let sidecars = SidecarStore::new(&store);
    sidecars.ensure_enqueued("c1").await.unwrap();
    // 模拟崩溃：缩略图已认领（running）即死
    assert!(sidecars.claim("c1", stage_ids::THUMBNAIL).await.unwrap());

    // 全量直算参照（无残留）
    let store_ref = mem_store().await;
    seed_content(&store_ref, "c1", "image/png").await;
    let thumb = CountingStage::new(stage_ids::THUMBNAIL, MimeKind::Image, Behavior::Ok);
    let loader = InMemoryContentLoader::new([("c1".into(), png_bytes(64, 32))]);
    let want = run(&store_ref, vec![thumb.clone()], &loader, "c1").await;

    let reset = sidecars.reset_running(Some("c1")).await.unwrap();
    assert_eq!(reset, 1, "running 复位为 pending");
    let got = run(&store, vec![thumb.clone()], &loader, "c1").await;
    assert_eq!(got, want, "恢复后终态与全量直算等价");
    assert_eq!(thumb.calls(), 2, "参照 + 恢复各一轮");
}

/// 验收「模型缺失降级」：skipped 行原因可查，后续 stage 照常完成。
#[tokio::test]
async fn skipped_degradation_records_reason_and_continues() {
    let store = mem_store().await;
    seed_content(&store, "c1", "image/png").await;
    let skipped = CountingStage::new(stage_ids::OCR, MimeKind::Image, Behavior::Skipped);
    let ok = CountingStage::new(stage_ids::EMBED, MimeKind::Image, Behavior::Ok);
    let loader = InMemoryContentLoader::new([("c1".into(), png_bytes(64, 32))]);
    let s = run(&store, vec![skipped.clone(), ok.clone()], &loader, "c1").await;
    assert_eq!((s.skipped, s.done), (1, 1), "skipped 不中断后续");

    let rows = SidecarStore::new(&store).items("c1").await.unwrap();
    let ocr = rows.iter().find(|r| r.stage == stage_ids::OCR).unwrap();
    assert_eq!(ocr.status_name(), "skipped");
    assert_eq!(ocr.detail.as_deref(), Some("模型缺失（fake）"));
    let embed = rows.iter().find(|r| r.stage == stage_ids::EMBED).unwrap();
    assert_eq!(embed.status_name(), "done");
}

/// 验收「失败不中断」：failed 记原因，后续 stage 完成，作业可 retry。
#[tokio::test]
async fn failed_records_reason_and_continues() {
    let store = mem_store().await;
    seed_content(&store, "c1", "image/png").await;
    let bad = CountingStage::new(stage_ids::OCR, MimeKind::Image, Behavior::Failed);
    let ok = CountingStage::new(stage_ids::EMBED, MimeKind::Image, Behavior::Ok);
    let loader = InMemoryContentLoader::new([("c1".into(), png_bytes(64, 32))]);
    let s = run(&store, vec![bad.clone(), ok.clone()], &loader, "c1").await;
    assert_eq!((s.failed, s.done), (1, 1));

    let rows = SidecarStore::new(&store).items("c1").await.unwrap();
    let ocr = rows.iter().find(|r| r.stage == stage_ids::OCR).unwrap();
    assert_eq!(ocr.status_name(), "failed");
    assert_eq!(ocr.detail.as_deref(), Some("boom（fake）"));
}

/// 验收「not-applicable」：OCR 对 text 内容降级 skipped。
#[tokio::test]
async fn not_applicable_marked_skipped() {
    let store = mem_store().await;
    seed_content(&store, "t1", "text/plain").await;
    let ocr = CountingStage::new(stage_ids::OCR, MimeKind::Image, Behavior::Ok);
    let loader = InMemoryContentLoader::new([("t1".into(), b"hello".to_vec())]);
    let s = run(&store, vec![ocr.clone()], &loader, "t1").await;
    assert_eq!(s.skipped, 1);
    assert_eq!(ocr.calls(), 0, "不适用不执行");
    let rows = SidecarStore::new(&store).items("t1").await.unwrap();
    let ocr_row = rows.iter().find(|r| r.stage == stage_ids::OCR).unwrap();
    assert_eq!(ocr_row.detail.as_deref(), Some("not-applicable"));
}

/// 验收「缩略图契约」：真实 PNG → JPEG ≤512 保比，跨轮字节稳定，落盘可读回。
#[tokio::test]
async fn thumbnail_stage_contract() {
    let store = mem_store().await;
    seed_content(&store, "c1", "image/png").await;
    let loader = InMemoryContentLoader::new([("c1".into(), png_bytes(1024, 600))]);
    let sink = Arc::new(InMemoryBlobSink::new());
    let pipeline = Pipeline::new(sink.clone(), default_stages());
    let s = pipeline.run_content(&store, &loader, "c1").await.unwrap();
    // 缩略图 done；EXIF 无 EXIF 降级 skipped；OCR/转写/嵌入占位 skipped
    assert_eq!((s.done, s.skipped, s.failed), (1, 4, 0));

    let thumb_bytes = sink.get("c1", stage_ids::THUMBNAIL).unwrap().unwrap();
    let decoded = image::load_from_memory(&thumb_bytes).unwrap();
    assert!(
        decoded.width() <= 512 && decoded.height() <= 512,
        "≤512 保比"
    );
    assert_eq!(
        (decoded.width(), decoded.height()),
        (512, 300),
        "1024x600 → 512x300"
    );
    let rows = SidecarStore::new(&store).items("c1").await.unwrap();
    let th = rows
        .iter()
        .find(|r| r.stage == stage_ids::THUMBNAIL)
        .unwrap();
    assert_eq!(th.detail.as_deref(), Some("512x300 jpeg"));
    assert_eq!(
        th.artifact.as_deref(),
        Some("mem:c1/thumbnail.bin"),
        "产物引用可追溯"
    );
}

/// 验收「EXIF 契约」：无 EXIF 的 PNG → skipped("no-exif: …")（降级非失败）。
#[tokio::test]
async fn exif_stage_without_exif_skips() {
    let store = mem_store().await;
    seed_content(&store, "c1", "image/png").await;
    let exif = partisync_ai::ExifStage::new();
    assert!(exif.applicable(&MimeKind::Image));
    assert!(!exif.applicable(&MimeKind::Video), "EXIF 只适用 Image");
    let loader = InMemoryContentLoader::new([("c1".into(), png_bytes(32, 32))]);
    let s = run(&store, vec![Arc::new(exif)], &loader, "c1").await;
    assert_eq!((s.skipped, s.failed), (1, 0));
    let rows = SidecarStore::new(&store).items("c1").await.unwrap();
    let ex = rows.iter().find(|r| r.stage == stage_ids::EXIF).unwrap();
    assert_eq!(ex.status_name(), "skipped");
    assert!(ex
        .detail
        .as_deref()
        .unwrap_or_default()
        .starts_with("no-exif"));
}

/// MimeKind.parse 契约（stage applicable 判据）。
#[test]
fn mime_kind_parse() {
    assert_eq!(MimeKind::parse(Some("image/png")), MimeKind::Image);
    assert_eq!(MimeKind::parse(Some("audio/flac")), MimeKind::Audio);
    assert_eq!(MimeKind::parse(Some("video/mp4")), MimeKind::Video);
    assert_eq!(MimeKind::parse(Some("text/plain")), MimeKind::Text);
    assert_eq!(MimeKind::parse(Some("application/pdf")), MimeKind::Other);
    assert_eq!(MimeKind::parse(None), MimeKind::Other);
    assert_eq!(MimeKind::parse(Some("")), MimeKind::Other);
}

/// 作业终态：无 failed → completed；done 计数 = processed。
/// STAGE_ORDER 次序契约钉住。
#[tokio::test]
async fn sidecar_job_completes_with_default_stages() {
    let store = mem_store().await;
    seed_content(&store, "c1", "image/png").await;
    let loader = InMemoryContentLoader::new([("c1".into(), png_bytes(640, 480))]);
    let sink = Arc::new(InMemoryBlobSink::new());
    let pipeline = Pipeline::new(sink, default_stages());
    let out = run_sidecar_job(&store, &pipeline, &loader, "c1")
        .await
        .unwrap();
    let job = partisync_ai::get_job(&store, &out.job_id).await.unwrap();
    assert_eq!(job.status_name, "completed");
    assert_eq!(job.done_files, 5, "done(1)+skipped(4) 计入 processed");
    assert_eq!(job.root, "c1", "sidecar 作业 root = content_id（去重锚点）");
}

/// 崩溃后 resume：latest_resumable_by_kind 找到 sidecar 作业 → 恢复完成。
#[tokio::test]
async fn resume_latest_sidecar_job_recovers() {
    let store = mem_store().await;
    seed_content(&store, "c1", "image/png").await;
    // 模拟中断：建作业置 running + 一行 running 残留
    let job_id = partisync_graph::jobs::create(&store, "sidecar", "c1")
        .await
        .unwrap();
    partisync_graph::jobs::start(&store, &job_id).await.unwrap();
    let sidecars = SidecarStore::new(&store);
    sidecars.ensure_enqueued("c1").await.unwrap();
    sidecars.claim("c1", stage_ids::THUMBNAIL).await.unwrap();

    let loader = InMemoryContentLoader::new([("c1".into(), png_bytes(200, 100))]);
    let sink = Arc::new(InMemoryBlobSink::new());
    let pipeline = Pipeline::new(sink, default_stages());
    let out = partisync_ai::resume_latest_sidecar_job(&store, &pipeline, &loader)
        .await
        .unwrap()
        .expect("找到可恢复作业");
    assert_eq!(out.job_id, job_id);
    let job = partisync_ai::get_job(&store, &out.job_id).await.unwrap();
    assert_eq!(job.status_name, "completed");
    let st = sidecars.stats().await.unwrap();
    assert_eq!(st.running, 0, "无残留 running");
    assert_eq!(st.done, 1, "缩略图补算完成，EXIF 降级 skipped");
}

/// FsBlobSink：落盘 + 读回 roundtrip + 产物引用格式。
#[test]
fn fs_blob_sink_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let sink = partisync_ai::FsBlobSink::new(dir.path());
    let r = sink.put("c1", "thumbnail", b"thumb-bytes").unwrap();
    assert_eq!(r, "fs:c1/thumbnail.bin");
    assert_eq!(
        sink.get("c1", "thumbnail").unwrap().as_deref(),
        Some(b"thumb-bytes".as_slice())
    );
    assert_eq!(sink.get("c1", "embed").unwrap(), None, "缺失返回 None");
}

/// STAGE_ORDER 固定次序契约（裁定 3）。
#[test]
fn stage_order_fixed() {
    assert_eq!(
        STAGE_ORDER,
        ["thumbnail", "exif", "ocr", "transcribe", "embed"]
    );
}

/// ItemStatus 数值与 DB 列口径一致。
#[test]
fn item_status_codes() {
    assert_eq!(ItemStatus::Pending as i64, 0);
    assert_eq!(ItemStatus::Running as i64, 1);
    assert_eq!(ItemStatus::Done as i64, 2);
    assert_eq!(ItemStatus::Skipped as i64, 3);
    assert_eq!(ItemStatus::Failed as i64, 4);
}
