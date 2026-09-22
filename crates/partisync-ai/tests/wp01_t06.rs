//! M4-WP01-T06 调度接线测试：scan → 自动入队 → 批量驱动端到端
//! （fake 模型，CI 离线）+ 崩溃恢复压力（随机点 kill × N 轮 → resume
//! 终态与全量直算等价，SPEC 验收「崩溃恢复」压力口径）。

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use partisync_ai::pipeline::{
    MimeKind, Pipeline, SidecarStage, StageError, StageInput, StageOutput,
};
use partisync_ai::sidecar::{stage_ids, InMemoryBlobSink, ItemStatus, SidecarStore};
use partisync_ai::{
    close_stale_sidecar_jobs, run_pending_sidecars, sidecar_auto_enqueue, ExifStage,
    FakeEmbedStage, ModelManager, ThumbnailStage,
};
use partisync_cas::ChunkStore;
use partisync_graph::indexer::index_path_job;
use partisync_graph::jobs as graph_jobs;
use partisync_graph::store::Store;

/// xorshift64（测试内确定性随机，不引 rand）。
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// 参照 store 的 pending 计数（未注册 stage 口径对齐）。
async fn want_pending(store: &Store) -> u64 {
    SidecarStore::new(store).stats().await.unwrap().pending
}

fn png_bytes(w: u32, h: u32, seed: u8) -> Vec<u8> {
    let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(w, h, |x, y| {
        image::Rgb([x as u8 ^ seed, y as u8 ^ seed, seed])
    }));
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    buf.into_inner()
}

/// 临时索引根：2 份不同 PNG + 1 份与前两者内容不同的 PNG（共 3 entry
/// 2 content，钉 content 去重在调度层的呈现）。
fn seed_root(tag: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.png"), png_bytes(64, 48, 1)).unwrap();
    std::fs::write(dir.path().join("b.png"), png_bytes(80, 60, 2)).unwrap();
    std::fs::write(dir.path().join("dup.png"), png_bytes(64, 48, 1)).unwrap();
    let _ = tag;
    dir
}

async fn index_root(store: &Store, root: &Path) {
    let cas = ChunkStore::open_in_memory(&tempfile::tempdir().unwrap().keep())
        .await
        .unwrap();
    let report = index_path_job(store, Some(&cas), root, None)
        .await
        .expect("index_path_job");
    assert_eq!(report.files, 3, "3 entry 入库");
}

/// 测试 stage 集：缩略图+EXIF 实装 + fake 嵌入（临时模型缓存，两模型
/// 均标记在场）——全链 done 口径。
fn full_stages(mgr: ModelManager) -> Vec<Arc<dyn SidecarStage>> {
    mgr.mark_present(partisync_ai::EMBED_MODEL_TEXT).unwrap();
    mgr.mark_present(partisync_ai::EMBED_MODEL_IMAGE).unwrap();
    vec![
        Arc::new(ThumbnailStage::new()),
        Arc::new(ExifStage::new()),
        Arc::new(FakeEmbedStage::new(mgr)),
    ]
}

/// 验收「调度接线」端到端：scan（indexer）→ auto_enqueue →
/// run_pending_sidecars → 作业/状态行全链可见；content 去重 = 2 content
/// 恰 2 作业；重跑全 no-op。
#[tokio::test]
async fn e2e_scan_enqueue_pipeline_cli_state() {
    let root = seed_root("e2e");
    let store = Store::open_in_memory().await.unwrap();
    index_root(&store, root.path()).await;

    // 自动入队：3 entry → 2 content（dup 内容同 hash）
    let enqueued = sidecar_auto_enqueue(&store).await.unwrap();
    assert_eq!(enqueued, 2, "content 去重后 2 个入队");
    assert_eq!(sidecar_auto_enqueue(&store).await.unwrap(), 0, "幂等");

    let dir_guard = tempfile::tempdir().unwrap();
    let mgr = ModelManager::new(dir_guard.path());
    let loader = partisync_ai::LocalContentLoader::scan(root.path(), &store)
        .await
        .unwrap();
    let pipeline = Pipeline::new(Arc::new(InMemoryBlobSink::new()), full_stages(mgr));
    let rep = run_pending_sidecars(&store, &pipeline, &loader, None)
        .await
        .unwrap();
    assert_eq!(rep.jobs, 2, "2 content 恰 2 作业");

    // 全链状态可查（CLI sidecar-status 口径）：mime 由 loader 按扩展名
    // 兜底（indexer 不写 mime 列）→ PNG 内容 缩略图+嵌入 done、EXIF 降级；
    // OCR/转写未注册保持 pending（T05 进场补算）；c2pa 本测试管线未注册
    // 同保持 pending（M4-WP05 起 3 stage/content 未注册 → 2×3=6）
    let st = SidecarStore::new(&store).stats().await.unwrap();
    assert_eq!(st.pending, 6, "未注册 stage 待补算");
    assert_eq!(st.running, 0);
    assert_eq!(st.done, 2 * 2, "缩略图+嵌入全 done");
    assert_eq!((st.skipped, st.failed), (2, 0), "EXIF no-exif 降级");
    let jobs = graph_jobs::list(&store).await.unwrap();
    assert_eq!(jobs.iter().filter(|j| j.kind == "sidecar").count(), 2);

    // 重跑全 no-op
    let rep2 = run_pending_sidecars(&store, &pipeline, &loader, None)
        .await
        .unwrap();
    assert_eq!((rep2.jobs, rep2.done, rep2.failed), (0, 0, 0));
}

/// 崩溃恢复压力：缩略图 stage 在随机执行点 panic（模拟 kill）× 15 轮，
/// 每轮恢复（close_stale + resume + 续跑）直至 Ok；终态与全量直算等价。
#[tokio::test]
async fn crash_recovery_stress_random_kill_rounds() {
    let root = seed_root("stress");
    // 参照：无故障全量直算
    let store_ref = Store::open_in_memory().await.unwrap();
    index_root(&store_ref, root.path()).await;
    let dir_guard_ref = tempfile::tempdir().unwrap();
    let mgr_ref = ModelManager::new(dir_guard_ref.path());
    let loader_ref = partisync_ai::LocalContentLoader::scan(root.path(), &store_ref)
        .await
        .unwrap();
    let want = run_pending_sidecars(
        &store_ref,
        &Pipeline::new(Arc::new(InMemoryBlobSink::new()), full_stages(mgr_ref)),
        &loader_ref,
        None,
    )
    .await
    .unwrap();

    // 故障轮：同一 store 反复「crash + resume」
    let store = Store::open_in_memory().await.unwrap();
    index_root(&store, root.path()).await;
    let dir_guard2 = tempfile::tempdir().unwrap();
    let mgr = ModelManager::new(dir_guard2.path());
    mgr.mark_present(partisync_ai::EMBED_MODEL_TEXT).unwrap();
    mgr.mark_present(partisync_ai::EMBED_MODEL_IMAGE).unwrap();
    let loader = partisync_ai::LocalContentLoader::scan(root.path(), &store)
        .await
        .unwrap();

    for seed in 1..=15u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let panic_at = 1 + (rng.next() % 3) as usize;
        let bomb = Arc::new(BombStage {
            calls: AtomicUsize::new(0),
            panic_at,
        });
        let stages: Vec<Arc<dyn SidecarStage>> = vec![
            bomb as Arc<dyn SidecarStage>,
            Arc::new(ExifStage::new()),
            Arc::new(FakeEmbedStage::new(mgr.clone())),
        ];
        let pipeline = Pipeline::new(Arc::new(InMemoryBlobSink::new()), stages);
        // 崩溃轮：Err（panic 经 spawn_blocking → JoinError → Fatal）
        let _ = run_pending_sidecars(&store, &pipeline, &loader, None).await;
        // 恢复轮：安全 stage 集续走至收敛
        let mut ok = false;
        for _ in 0..10 {
            let clean: Vec<Arc<dyn SidecarStage>> = vec![
                Arc::new(ThumbnailStage::new()),
                Arc::new(ExifStage::new()),
                Arc::new(FakeEmbedStage::new(mgr.clone())),
            ];
            let p = Pipeline::new(Arc::new(InMemoryBlobSink::new()), clean);
            match run_pending_sidecars(&store, &p, &loader, None).await {
                Ok(_) => {
                    ok = true;
                    break;
                }
                Err(_) => continue,
            }
        }
        assert!(ok, "轮 {seed}: 10 次恢复内未收敛");
    }

    // 终态等价（L5 口径）：done 计数一致、无 running、无 failed
    assert_eq!(want.done, 2 * 2, "参照全量直算（缩略图+嵌入）");
    let st = SidecarStore::new(&store).stats().await.unwrap();
    assert_eq!(
        (st.done, st.running, st.failed),
        (want.done, 0, 0),
        "15 轮崩溃后终态与全量直算等价"
    );
    assert_eq!(st.pending, want_pending(&store_ref).await, "pending 侧一致");
    // 无陈旧 sidecar 作业残留
    close_stale_sidecar_jobs(&store).await.unwrap();
    let jobs = graph_jobs::list(&store).await.unwrap();
    assert!(
        jobs.iter()
            .all(|j| j.kind != "sidecar" || j.status_name != "running"),
        "无 running 作业残留"
    );
}

/// 崩溃注入 stage：第 panic_at 次执行时 panic（之前正常产出缩略图）。
struct BombStage {
    calls: AtomicUsize,
    panic_at: usize,
}

impl SidecarStage for BombStage {
    fn stage(&self) -> &'static str {
        stage_ids::THUMBNAIL
    }

    fn applicable(&self, mime: &MimeKind) -> bool {
        *mime == MimeKind::Image
    }

    fn run(&self, input: &StageInput) -> Result<StageOutput, StageError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if n >= self.panic_at {
            panic!("模拟进程崩溃（第 {n} 次执行）");
        }
        let img = image::load_from_memory(&input.data).unwrap();
        let thumb = img.thumbnail(32, 32);
        let mut buf = std::io::Cursor::new(Vec::new());
        thumb.write_to(&mut buf, image::ImageFormat::Jpeg).unwrap();
        Ok(StageOutput {
            blob: Some(buf.into_inner()),
            detail: Some("bomb-thumb".into()),
            ..StageOutput::default()
        })
    }
}

/// 状态码与 problems() 明细口径（CLI sidecar-status 数据源）。
#[tokio::test]
async fn problems_lists_skipped_and_failed() {
    let store = Store::open_in_memory().await.unwrap();
    sqlx::query("INSERT INTO content (id, size, mime, kind) VALUES ('c1', 1, 'image/png', 'file')")
        .execute(store.pool_ref())
        .await
        .unwrap();
    let sidecars = SidecarStore::new(&store);
    sidecars.ensure_enqueued("c1").await.unwrap();
    sidecars
        .mark_skipped("c1", stage_ids::EXIF, "no-exif: t")
        .await
        .unwrap();
    sidecars
        .mark_failed("c1", stage_ids::EMBED, "boom")
        .await
        .unwrap();
    let probs = sidecars.problems(10).await.unwrap();
    assert_eq!(probs.len(), 2);
    assert!(probs.iter().any(|r| r.status_name() == "skipped"));
    assert!(probs.iter().any(|r| r.status_name() == "failed"));
    let _ = ItemStatus::Pending; // 口径引用防漂移
}
