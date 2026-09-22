//! Sidecar 作业编排（JobSystem v2 门面，SPEC M4-WP01 裁定 1/7）：
//! 复用 graph `jobs` 表（kind = `sidecar`，root = content_id），
//! 作业入口统一做崩溃恢复（running 复位）后再驱动管线。

use partisync_core::error::PartisyError;
use partisync_graph::jobs as graph_jobs;
use partisync_graph::store::Store;

use crate::pipeline::{ContentLoader, Pipeline, PipelineSummary};

/// 一次作业执行的产物。
#[derive(Debug, Clone)]
pub struct JobOutcome {
    pub job_id: String,
    pub content_id: String,
    pub summary: PipelineSummary,
}

/// 跑单 content 的 sidecar 作业（新建 job 行 → 恢复复位 → 驱动管线 →
/// 落作业终态）。
///
/// 终态语义（SPEC 契约 §1）：无 failed → completed（done 计入 processed）；
/// 有 failed → failed + error 注明 partial（单 stage 可 retry）。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn run_sidecar_job(
    store: &Store,
    pipeline: &Pipeline,
    loader: &dyn ContentLoader,
    content_id: &str,
) -> Result<JobOutcome, PartisyError> {
    let job_id = graph_jobs::create(store, "sidecar", content_id).await?;
    graph_jobs::start(store, &job_id).await?;
    let summary = drive(store, pipeline, loader, content_id).await?;
    finalize(store, &job_id, content_id, summary).await?;
    Ok(JobOutcome {
        job_id,
        content_id: content_id.to_owned(),
        summary,
    })
}

/// 恢复最近一个可恢复的 sidecar 作业（interrupted / 陈旧 running）。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn resume_latest_sidecar_job(
    store: &Store,
    pipeline: &Pipeline,
    loader: &dyn ContentLoader,
) -> Result<Option<JobOutcome>, PartisyError> {
    let Some(row) = graph_jobs::latest_resumable_by_kind(store, "sidecar").await? else {
        return Ok(None);
    };
    let content_id = row.root.clone();
    graph_jobs::start(store, &row.id).await?;
    let summary = drive(store, pipeline, loader, &content_id).await?;
    finalize(store, &row.id, &content_id, summary).await?;
    Ok(Some(JobOutcome {
        job_id: row.id,
        content_id,
        summary,
    }))
}

async fn drive(
    store: &Store,
    pipeline: &Pipeline,
    loader: &dyn ContentLoader,
    content_id: &str,
) -> Result<PipelineSummary, PartisyError> {
    let sidecars = crate::sidecar::SidecarStore::new(store);
    sidecars.reset_running(Some(content_id)).await?;
    pipeline.run_content(store, loader, content_id).await
}

async fn finalize(
    store: &Store,
    job_id: &str,
    content_id: &str,
    summary: PipelineSummary,
) -> Result<(), PartisyError> {
    if summary.failed > 0 {
        graph_jobs::fail(
            store,
            job_id,
            &format!(
                "partial: content={content_id} done={} skipped={} failed={}",
                summary.done, summary.skipped, summary.failed
            ),
        )
        .await
    } else {
        graph_jobs::complete(store, job_id, summary.processed()).await
    }
}

/// 查询作业（透传 graph::jobs，供 CLI/测试）。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn get_job(store: &Store, job_id: &str) -> Result<graph_jobs::JobRow, PartisyError> {
    graph_jobs::get(store, job_id).await
}
