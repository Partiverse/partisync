//! Sidecar 作业编排（JobSystem v2 门面，SPEC M4-WP01 裁定 1/7）：
//! 复用 graph `jobs` 表（kind = `sidecar`，root = content_id），
//! 作业入口统一做崩溃恢复（running 复位）后再驱动管线。
//!
//! T06 调度接线：scan 投影（content 表）→ [`sidecar_auto_enqueue`]
//! 批量入队 → [`run_pending_sidecars`] 批量驱动；[`LocalContentLoader`]
//! 按 entry vpath 从本地盘读内容字节（单设备 CLI 口径；CAS 直读归 M5）。

use std::path::{Path, PathBuf};

use partisync_core::error::{PartisyError, Severity};
use partisync_graph::jobs as graph_jobs;
use partisync_graph::store::Store;

use crate::pipeline::{ContentLoader, Pipeline, PipelineSummary};
use crate::sidecar::SidecarStore;

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

/// scan 投影接线（SPEC 验收「scan_journal 事件 → 自动入队」）：为尚无
/// sidecar 行的 content 批量入队（content 表是 scan_journal 的应用投影，
/// 本函数幂等——重复调用 no-op）。返回新入队的 content 数。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn sidecar_auto_enqueue(store: &Store) -> Result<u64, PartisyError> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT c.id FROM content c \
         LEFT JOIN sidecar_items si ON si.content_id = c.id \
         WHERE si.content_id IS NULL",
    )
    .fetch_all(store.pool_ref())
    .await
    .map_err(|e| fatal("查未入队 content", e.to_string()))?;
    let sidecars = SidecarStore::new(store);
    for id in &ids {
        sidecars.ensure_enqueued(id).await?;
    }
    Ok(ids.len() as u64)
}

/// 收口陈旧作业：崩溃残留的 running sidecar 作业 → interrupted
/// （内容行由下次作业入口 reset_running 恢复）。返回收口数。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn close_stale_sidecar_jobs(store: &Store) -> Result<u64, PartisyError> {
    let r = sqlx::query(
        "UPDATE jobs SET status = 2, updated_ns = ? WHERE kind = 'sidecar' AND status = 1",
    )
    .bind(now_ns())
    .execute(store.pool_ref())
    .await
    .map_err(|e| fatal("收口陈旧作业", e.to_string()))?;
    Ok(r.rows_affected())
}

/// 一轮批量驱动摘要。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SidecarRunReport {
    pub jobs: u64,
    pub done: u64,
    pub skipped: u64,
    pub failed: u64,
}

impl SidecarRunReport {
    fn merge(&mut self, s: PipelineSummary) {
        self.jobs += 1;
        self.done += s.done;
        self.skipped += s.skipped;
        self.failed += s.failed;
    }
}

/// 批量调度入口（CLI `sidecar-run` / watch 集成共用）：
/// 收口陈旧作业 → 自动入队 → 恢复最近可恢复作业 → 逐 content 新建作业
/// 驱动至终态。`limit` = 本次最多处理的 content 数（None = 全部当前
/// pending；未注册 stage 的 pending 行不循环重查，防活锁）。
///
/// # Errors
/// DB / stage 任务中止 → Fatal（批量中止单个失败冒泡，重跑续走）。
pub async fn run_pending_sidecars(
    store: &Store,
    pipeline: &Pipeline,
    loader: &dyn ContentLoader,
    limit: Option<u64>,
) -> Result<SidecarRunReport, PartisyError> {
    close_stale_sidecar_jobs(store).await?;
    sidecar_auto_enqueue(store).await?;
    let mut report = SidecarRunReport::default();
    // 崩溃恢复优先：最近一个可恢复作业（其 pending 行随查询自然覆盖）
    if let Some(out) = resume_latest_sidecar_job(store, pipeline, loader).await? {
        report.merge(out.summary);
    }
    // 只取「pending 且 stage 已注册」的 content——未注册 stage 的行
    // 保持 pending 不驱动（T02 语义），避免每轮重复开作业。
    let ids: Vec<String> = {
        let registered = pipeline.stage_ids();
        if registered.is_empty() {
            Vec::new()
        } else {
            let mut qb = sqlx::QueryBuilder::new(
                "SELECT DISTINCT si.content_id FROM sidecar_items si \
                 WHERE si.status = 0 AND si.stage IN (",
            );
            let mut sep = "";
            for s in registered {
                qb.push(sep).push_bind(s);
                sep = ",";
            }
            qb.push(") ORDER BY si.content_id");
            qb.build_query_scalar::<String>()
                .fetch_all(store.pool_ref())
                .await
                .map_err(|e| fatal("查 pending content", e.to_string()))?
        }
    };
    for id in ids {
        if limit.is_some_and(|cap| report.jobs >= cap) {
            break;
        }
        let out = run_sidecar_job(store, pipeline, loader, &id).await?;
        report.merge(out.summary);
    }
    Ok(report)
}

/// 本地内容加载器：content_id → entry.path（vpath，索引器产出的根相对
/// 路径）→ `<root>/<vpath>` 读盘。单设备 CLI 口径（CAS 直读归 M5）。
/// 路径映射在构造时一次性预扫（sync [`ContentLoader`] 的桥接代价，
/// CLI 本地规模可接受），load 为纯查表 + 读文件。
#[derive(Clone)]
pub struct LocalContentLoader {
    root: PathBuf,
    entries: std::collections::HashMap<String, (String, Option<String>)>,
}

impl LocalContentLoader {
    /// 预扫 entry 表建 content_id → (vpath, mime 猜测) 映射
    /// （同 content 取最小 vpath；mime 按扩展名猜测——indexer 现状
    /// 不写 mime 列，T06 实测确认）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn scan(root: impl Into<PathBuf>, store: &Store) -> Result<Self, PartisyError> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT content_id, MIN(path) FROM entry \
             WHERE content_id IS NOT NULL GROUP BY content_id",
        )
        .fetch_all(store.pool_ref())
        .await
        .map_err(|e| fatal("预扫 entry 路径", e.to_string()))?;
        Ok(LocalContentLoader {
            root: root.into(),
            entries: rows
                .into_iter()
                .map(|(id, path)| {
                    let mime = guess_mime(&path).map(str::to_owned);
                    (id, (path, mime))
                })
                .collect(),
        })
    }
}

/// 扩展名 → mime 大类（sidecar applicable 判据口径；未知 → None）。
fn guess_mime(vpath: &str) -> Option<&'static str> {
    let ext = Path::new(vpath).extension()?.to_str()?.to_ascii_lowercase();
    let kind = match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "heic" => "image",
        "mp3" | "flac" | "wav" | "m4a" | "aac" | "ogg" => "audio",
        "mp4" | "mov" | "mkv" | "avi" | "webm" => "video",
        "txt" | "md" | "rst" | "csv" | "json" => "text",
        _ => return None,
    };
    Some(kind)
}

impl ContentLoader for LocalContentLoader {
    fn load(&self, content_id: &str) -> Result<Vec<u8>, PartisyError> {
        let Some((vpath, _)) = self.entries.get(content_id) else {
            return Err(fatal(
                "加载内容",
                format!("content 无 entry 落点: {content_id}"),
            ));
        };
        // vpath 以 "/" 锚定（索引器根条目口径），join 前剥掉防整体替换
        let rel = vpath.strip_prefix('/').unwrap_or(vpath);
        std::fs::read(self.root.join(rel)).map_err(|e| {
            fatal(
                "加载内容",
                format!("{}: {e}", self.root.join(rel).display()),
            )
        })
    }

    fn mime(&self, content_id: &str) -> Option<String> {
        self.entries.get(content_id).and_then(|(_, m)| m.clone())
    }
}

fn now_ns() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as i64)
}

fn fatal(what: &str, msg: String) -> PartisyError {
    PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("{what}: {msg}").into()),
    }
}
