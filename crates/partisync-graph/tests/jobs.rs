//! 作业系统测试（SPEC M0-WP05 验收；L5 恢复等价性）。

use partisync_cas::ChunkStore;
use partisync_core::error::Severity;
use partisync_core::Ulid;
use partisync_graph::indexer::index_path_job;
use partisync_graph::jobs::{self, JobCtx};
use partisync_graph::store::Store;

async fn file_store(tag: &str) -> (Store, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("jobs-{tag}-{}", Ulid::now()));
    std::fs::create_dir_all(&dir).unwrap();
    let store = Store::open(&dir.join("t.db")).await.unwrap();
    (store, dir)
}

fn make_tree(root: &std::path::Path, files: u32) {
    for i in 0..files {
        // 混合目录/点号文件命名：刻意触发 DFS 序 ≠ 字典序的 historically 危险组合
        let sub = format!("d{}", i % 3);
        let dir = root.join(&sub);
        std::fs::create_dir_all(&dir).unwrap();
        let content = format!("content-{i}-{}", "x".repeat(i as usize % 64));
        std::fs::write(dir.join(format!("f{i:03}.bin")), content).unwrap();
        if i % 7 == 0 {
            std::fs::write(root.join(format!("a{i:03}.txt")), format!("root-{i}")).unwrap();
        }
    }
}

/// L5（properties.md）：「stop_after 中断 → resume」最终 stats ≡ 「全量直index」。
#[tokio::test]
async fn l5_resume_equivalence() {
    let fs = std::env::temp_dir().join(format!("jobs-fs-{}", Ulid::now()));
    make_tree(&fs, 60);

    // 参照组：全量直 index（独立 db + cas）
    let (ref_store, ref_dir) = file_store("ref").await;
    let ref_cas_dir = ref_dir.join("cas");
    let ref_cas = ChunkStore::open(&ref_cas_dir).await.unwrap();
    index_path_job(&ref_store, Some(&ref_cas), &fs, None)
        .await
        .unwrap();
    let s_ref = ref_store.stats().await.unwrap();

    // 实验组：stop_after=15 中断 → resume
    let (store, dir) = file_store("resume").await;
    let cas_dir = dir.join("cas");
    let cas = ChunkStore::open(&cas_dir).await.unwrap();
    let job_id = jobs::create(&store, "index", fs.to_str().unwrap())
        .await
        .unwrap();
    jobs::start(&store, &job_id).await.unwrap();
    let mut ctx = JobCtx {
        id: job_id.clone(),
        skip_up_to: None,
        stop_after: Some(15),
        done: 0,
    };
    let err = index_path_job(&store, Some(&cas), &fs, Some(&mut ctx))
        .await
        .unwrap_err();
    assert_eq!(
        err.severity,
        Severity::Interrupted,
        "stop_after → Interrupted"
    );
    jobs::mark_interrupted(&store, &job_id, ctx.done)
        .await
        .unwrap();
    let row = jobs::get(&store, &job_id).await.unwrap();
    assert_eq!(row.status_name(), "interrupted");
    assert!(row.checkpoint.is_some(), "中断必须留下 checkpoint");
    let done_at_interrupt = row.done_files;

    // resume：从 checkpoint 划界续跑
    let row = jobs::get(&store, &job_id).await.unwrap();
    jobs::start(&store, &job_id).await.unwrap();
    let mut ctx = JobCtx::for_resume(&row, None);
    index_path_job(&store, Some(&cas), &fs, Some(&mut ctx))
        .await
        .unwrap();
    jobs::complete(&store, &job_id, ctx.done).await.unwrap();
    let row = jobs::get(&store, &job_id).await.unwrap();
    assert_eq!(row.status_name(), "completed");
    assert_eq!(
        row.done_files, ctx.done as i64,
        "resume 后 done 从续点累计（≥ 中断时的 15）"
    );
    assert!(row.done_files >= done_at_interrupt);

    // L5 核心：两组 stats 完全相等
    let s_resume = store.stats().await.unwrap();
    assert_eq!(s_resume, s_ref, "恢复等价性：中断续跑 ≡ 全量直index");
    assert_eq!(
        s_ref.files,
        60 + (0..60).filter(|i| i % 7 == 0).count() as i64
    );

    std::fs::remove_dir_all(&fs).ok();
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(&ref_dir).ok();
}

#[tokio::test]
async fn job_status_lifecycle_recorded() {
    let (store, dir) = file_store("lifecycle").await;
    let id = jobs::create(&store, "index", "/tmp/x").await.unwrap();
    assert_eq!(
        jobs::get(&store, &id).await.unwrap().status_name(),
        "queued"
    );
    jobs::start(&store, &id).await.unwrap();
    assert_eq!(
        jobs::get(&store, &id).await.unwrap().status_name(),
        "running"
    );
    jobs::fail(&store, &id, "disk full").await.unwrap();
    let row = jobs::get(&store, &id).await.unwrap();
    assert_eq!(row.status_name(), "failed");
    assert_eq!(row.error.as_deref(), Some("disk full"));
    assert_eq!(jobs::list(&store).await.unwrap().len(), 1);
    std::fs::remove_dir_all(&dir).ok();
}
