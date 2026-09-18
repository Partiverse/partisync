//! 持久作业系统 v1（SPEC M0-WP05）：index 作业的 checkpoint 与恢复。
//!
//! L5（恢复等价性）：「中断→resume」与「全量直index」最终 stats 完全相等。
//! 状态偏离声明：v1 状态即关系列（无需 MessagePack，SPEC §4）。

use std::time::UNIX_EPOCH;

use partisync_core::error::{PartisyError, Severity};
use serde::Serialize;

use crate::store::Store;

/// 作业状态（jobs.status）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum JobStatus {
    Queued = 0,
    Running = 1,
    Interrupted = 2,
    Completed = 3,
    Failed = 4,
}

impl JobStatus {
    fn as_str(self) -> &'static str {
        match self {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Interrupted => "interrupted",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
        }
    }
}

/// 作业行视图。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct JobRow {
    pub id: String,
    pub kind: String,
    pub status: i64,
    pub root: String,
    pub checkpoint: Option<String>,
    pub done_files: i64,
    pub error: Option<String>,
}

/// 运行期作业上下文（传给 index_path）。
#[derive(Debug, Clone)]
pub struct JobCtx {
    pub id: String,
    /// 续跑划界：vpath ≤ 此值的文件跳过（目录不跳）。
    pub skip_up_to: Option<String>,
    /// 测试/演练注入点：处理 N 个文件后返回 Interrupted。
    pub stop_after: Option<u64>,
    pub done: u64,
}

const CHECKPOINT_EVERY: u64 = 200;

fn now_ns() -> i64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as i64)
}

fn db_err(what: &str, e: sqlx::Error) -> PartisyError {
    PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("{what}: {e}").into()),
    }
}

/// 创建作业（queued）。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn create(store: &Store, kind: &str, root: &str) -> Result<String, PartisyError> {
    let id = partisync_core::Ulid::now().to_string();
    let ns = now_ns();
    sqlx::query("INSERT INTO jobs (id, kind, status, root, created_ns, updated_ns) VALUES (?, ?, 0, ?, ?, ?)")
        .bind(&id)
        .bind(kind)
        .bind(root)
        .bind(ns)
        .bind(ns)
        .execute(store.pool_ref())
        .await
        .map_err(|e| db_err("创建作业", e))?;
    Ok(id)
}

async fn set_status(
    store: &Store,
    id: &str,
    status: JobStatus,
    checkpoint: Option<&str>,
    done: i64,
    error: Option<&str>,
) -> Result<(), PartisyError> {
    sqlx::query(
        "UPDATE jobs SET status = ?, checkpoint = COALESCE(?, checkpoint),
         done_files = ?, error = ?, updated_ns = ? WHERE id = ?",
    )
    .bind(status as i64)
    .bind(checkpoint)
    .bind(done)
    .bind(error)
    .bind(now_ns())
    .bind(id)
    .execute(store.pool_ref())
    .await
    .map_err(|e| db_err("更新作业", e))?;
    Ok(())
}

/// 置 running。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn start(store: &Store, id: &str) -> Result<(), PartisyError> {
    set_status(store, id, JobStatus::Running, None, 0, None).await
}

/// 置 completed。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn complete(store: &Store, id: &str, done: u64) -> Result<(), PartisyError> {
    set_status(store, id, JobStatus::Completed, None, done as i64, None).await
}

/// 置 failed。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn fail(store: &Store, id: &str, error: &str) -> Result<(), PartisyError> {
    set_status(store, id, JobStatus::Failed, None, 0, Some(error)).await
}

/// 置 interrupted（checkpoint 保持最后一次提交值）。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn mark_interrupted(store: &Store, id: &str, done: u64) -> Result<(), PartisyError> {
    set_status(store, id, JobStatus::Interrupted, None, done as i64, None).await
}

/// 作业行 + 待续信息；陈旧 running 视同 interrupted（SPEC 风险节）。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn get(store: &Store, id: &str) -> Result<JobRow, PartisyError> {
    sqlx::query_as::<_, JobRow>(
        "SELECT id, kind, status, root, checkpoint, done_files, error FROM jobs WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(store.pool_ref())
    .await
    .map_err(|e| db_err("读作业", e))?
    .ok_or_else(|| PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("作业不存在: {id}").into()),
    })
}

/// 取最新 interrupted（或陈旧 running）作业 id。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn latest_resumable(store: &Store) -> Result<Option<JobRow>, PartisyError> {
    sqlx::query_as::<_, JobRow>(
        "SELECT id, kind, status, root, checkpoint, done_files, error FROM jobs
         WHERE status IN (2, 1) ORDER BY updated_ns DESC LIMIT 1",
    )
    .fetch_optional(store.pool_ref())
    .await
    .map_err(|e| db_err("查可恢复作业", e))
}

/// 列出全部作业（新→旧）。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn list(store: &Store) -> Result<Vec<JobRow>, PartisyError> {
    sqlx::query_as::<_, JobRow>(
        "SELECT id, kind, status, root, checkpoint, done_files, error FROM jobs
         ORDER BY created_ns DESC",
    )
    .fetch_all(store.pool_ref())
    .await
    .map_err(|e| db_err("列作业", e))
}

impl JobRow {
    #[must_use]
    pub fn status_name(&self) -> &'static str {
        match self.status {
            0 => JobStatus::Queued.as_str(),
            1 => JobStatus::Running.as_str(),
            2 => JobStatus::Interrupted.as_str(),
            3 => JobStatus::Completed.as_str(),
            _ => JobStatus::Failed.as_str(),
        }
    }
}

impl JobCtx {
    /// 从已存在作业构造续跑上下文。
    #[must_use]
    pub fn for_resume(row: &JobRow, stop_after: Option<u64>) -> Self {
        JobCtx {
            id: row.id.clone(),
            skip_up_to: row.checkpoint.clone(),
            stop_after,
            done: 0,
        }
    }

    /// 达到 checkpoint 提交节奏时落盘进度。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn maybe_flush(&mut self, store: &Store, vpath: &str) -> Result<(), PartisyError> {
        if self.done.is_multiple_of(CHECKPOINT_EVERY) {
            sqlx::query(
                "UPDATE jobs SET checkpoint = ?, done_files = ?, updated_ns = ? WHERE id = ?",
            )
            .bind(vpath)
            .bind(self.done as i64)
            .bind(now_ns())
            .bind(&self.id)
            .execute(store.pool_ref())
            .await
            .map_err(|e| db_err("提交 checkpoint", e))?;
        }
        Ok(())
    }

    /// 完成时提交最终 checkpoint。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn flush_final(&mut self, store: &Store, vpath: &str) -> Result<(), PartisyError> {
        sqlx::query("UPDATE jobs SET checkpoint = ?, done_files = ?, updated_ns = ? WHERE id = ?")
            .bind(vpath)
            .bind(self.done as i64)
            .bind(now_ns())
            .bind(&self.id)
            .execute(store.pool_ref())
            .await
            .map_err(|e| db_err("提交 checkpoint", e))?;
        Ok(())
    }
}
