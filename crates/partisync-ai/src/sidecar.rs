//! SidecarStore：per-content per-stage 状态行（schema v12，SPEC M4-WP01
//! 裁定 1/2）+ 产物落盘（BlobSink）。
//!
//! 状态机：pending → running → done | skipped | failed。stage 幂等——
//! claim 是原子的（`status=0` 条件更新），重跑/并发重复入队均 no-op；
//! 崩溃残留的 running 行由 [`SidecarStore::reset_running`] 复位后重入队。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use partisync_core::error::{PartisyError, Severity};
use partisync_graph::store::Store;
use sqlx::Row;

/// stage 常量与固定次序（SPEC 裁定 3）。
pub mod stage_ids {
    pub const THUMBNAIL: &str = "thumbnail";
    pub const EXIF: &str = "exif";
    pub const OCR: &str = "ocr";
    pub const TRANSCRIBE: &str = "transcribe";
    pub const EMBED: &str = "embed";
}

/// 固定管线次序（stage1 缩略图+EXIF → stage2 OCR → stage3 转写 → stage4 嵌入）。
pub const STAGE_ORDER: [&str; 5] = [
    stage_ids::THUMBNAIL,
    stage_ids::EXIF,
    stage_ids::OCR,
    stage_ids::TRANSCRIBE,
    stage_ids::EMBED,
];

/// sidecar_items.status。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemStatus {
    Pending = 0,
    Running = 1,
    Done = 2,
    Skipped = 3,
    Failed = 4,
}

impl ItemStatus {
    pub fn name(self) -> &'static str {
        match self {
            ItemStatus::Pending => "pending",
            ItemStatus::Running => "running",
            ItemStatus::Done => "done",
            ItemStatus::Skipped => "skipped",
            ItemStatus::Failed => "failed",
        }
    }
}

/// sidecar_items 行视图。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SidecarItemRow {
    pub content_id: String,
    pub stage: String,
    pub status: i64,
    pub detail: Option<String>,
    pub artifact: Option<String>,
    pub updated_ns: i64,
}

impl SidecarItemRow {
    pub fn status_name(&self) -> &'static str {
        match self.status {
            0 => "pending",
            1 => "running",
            2 => "done",
            3 => "skipped",
            _ => "failed",
        }
    }
}

/// 状态计数（stats 查询结果）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SidecarStats {
    pub pending: u64,
    pub running: u64,
    pub done: u64,
    pub skipped: u64,
    pub failed: u64,
}

fn err(what: &str, e: sqlx::Error) -> PartisyError {
    PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("{what}: {e}").into()),
    }
}

fn now_ns() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as i64)
}

/// Sidecar 状态机门面（graph 库 `sidecar_items` 表，供 WP02/MCP 消费）。
pub struct SidecarStore<'a> {
    store: &'a Store,
}

impl<'a> SidecarStore<'a> {
    #[must_use]
    pub fn new(store: &'a Store) -> Self {
        SidecarStore { store }
    }

    /// 按 content 入队全部五阶段（INSERT OR IGNORE——重复入队 no-op，
    /// content 粒度去重由主键保证）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn ensure_enqueued(&self, content_id: &str) -> Result<(), PartisyError> {
        let ns = now_ns();
        for stage in STAGE_ORDER {
            sqlx::query(
                "INSERT OR IGNORE INTO sidecar_items (content_id, stage, status, updated_ns) \
                 VALUES (?, ?, 0, ?)",
            )
            .bind(content_id)
            .bind(stage)
            .bind(ns)
            .execute(self.store.pool_ref())
            .await
            .map_err(|e| err("入队 sidecar", e))?;
        }
        Ok(())
    }

    /// 原子认领一个 pending 行（`status=0` 条件更新，受影响行数 = 认领结果）。
    /// 返回 None = 已被处理/认领（幂等重跑的 no-op 路径）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn claim(&self, content_id: &str, stage: &str) -> Result<bool, PartisyError> {
        let r = sqlx::query(
            "UPDATE sidecar_items SET status = 1, updated_ns = ? \
             WHERE content_id = ? AND stage = ? AND status = 0",
        )
        .bind(now_ns())
        .bind(content_id)
        .bind(stage)
        .execute(self.store.pool_ref())
        .await
        .map_err(|e| err("认领 sidecar", e))?;
        Ok(r.rows_affected() == 1)
    }

    async fn set_status(
        &self,
        content_id: &str,
        stage: &str,
        status: ItemStatus,
        detail: Option<&str>,
        artifact: Option<&str>,
    ) -> Result<(), PartisyError> {
        sqlx::query(
            "UPDATE sidecar_items SET status = ?, detail = ?, artifact = ?, updated_ns = ? \
             WHERE content_id = ? AND stage = ?",
        )
        .bind(status as i64)
        .bind(detail)
        .bind(artifact)
        .bind(now_ns())
        .bind(content_id)
        .bind(stage)
        .execute(self.store.pool_ref())
        .await
        .map_err(|e| err("更新 sidecar 状态", e))?;
        Ok(())
    }

    /// 标记完成（artifact = 产物引用，detail = 产物摘要）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn mark_done(
        &self,
        content_id: &str,
        stage: &str,
        artifact: Option<&str>,
        detail: Option<&str>,
    ) -> Result<(), PartisyError> {
        self.set_status(content_id, stage, ItemStatus::Done, detail, artifact)
            .await
    }

    /// 标记降级跳过（模型缺失/不适用等，SPEC 裁定 5）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn mark_skipped(
        &self,
        content_id: &str,
        stage: &str,
        reason: &str,
    ) -> Result<(), PartisyError> {
        self.set_status(content_id, stage, ItemStatus::Skipped, Some(reason), None)
            .await
    }

    /// 标记失败（意外错误，可 retry 单 stage）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn mark_failed(
        &self,
        content_id: &str,
        stage: &str,
        reason: &str,
    ) -> Result<(), PartisyError> {
        self.set_status(content_id, stage, ItemStatus::Failed, Some(reason), None)
            .await
    }

    /// 崩溃恢复：running 复位为 pending（`content_id=None` 全表）。
    /// 返回复位行数。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn reset_running(&self, content_id: Option<&str>) -> Result<u64, PartisyError> {
        let r = match content_id {
            Some(id) => sqlx::query(
                "UPDATE sidecar_items SET status = 0, updated_ns = ? \
                     WHERE status = 1 AND content_id = ?",
            )
            .bind(now_ns())
            .bind(id)
            .execute(self.store.pool_ref())
            .await
            .map_err(|e| err("复位 running", e))?,
            None => {
                sqlx::query("UPDATE sidecar_items SET status = 0, updated_ns = ? WHERE status = 1")
                    .bind(now_ns())
                    .execute(self.store.pool_ref())
                    .await
                    .map_err(|e| err("复位 running", e))?
            }
        };
        Ok(r.rows_affected())
    }

    /// 某 content 的全部 stage 行（按固定次序排列）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn items(&self, content_id: &str) -> Result<Vec<SidecarItemRow>, PartisyError> {
        let rows = sqlx::query(
            "SELECT content_id, stage, status, detail, artifact, updated_ns \
             FROM sidecar_items WHERE content_id = ?",
        )
        .bind(content_id)
        .fetch_all(self.store.pool_ref())
        .await
        .map_err(|e| err("查 sidecar 行", e))?;
        let mut items: Vec<SidecarItemRow> = rows
            .iter()
            .map(|r| SidecarItemRow {
                content_id: r.get("content_id"),
                stage: r.get("stage"),
                status: r.get("status"),
                detail: r.get("detail"),
                artifact: r.get("artifact"),
                updated_ns: r.get("updated_ns"),
            })
            .collect();
        items.sort_by_key(|it| {
            STAGE_ORDER
                .iter()
                .position(|s| *s == it.stage)
                .unwrap_or(usize::MAX)
        });
        Ok(items)
    }

    /// 全库降级/失败明细（skipped/failed，新→旧，cap 供 CLI 展示）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn problems(&self, limit: u32) -> Result<Vec<SidecarItemRow>, PartisyError> {
        let rows = sqlx::query(
            "SELECT content_id, stage, status, detail, artifact, updated_ns \
             FROM sidecar_items WHERE status IN (3, 4) ORDER BY updated_ns DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(self.store.pool_ref())
        .await
        .map_err(|e| err("查 sidecar 明细", e))?;
        Ok(rows
            .iter()
            .map(|r| SidecarItemRow {
                content_id: r.get("content_id"),
                stage: r.get("stage"),
                status: r.get("status"),
                detail: r.get("detail"),
                artifact: r.get("artifact"),
                updated_ns: r.get("updated_ns"),
            })
            .collect())
    }

    /// 全表状态计数。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn stats(&self) -> Result<SidecarStats, PartisyError> {
        let rows = sqlx::query("SELECT status, COUNT(*) AS n FROM sidecar_items GROUP BY status")
            .fetch_all(self.store.pool_ref())
            .await
            .map_err(|e| err("统计 sidecar", e))?;
        let mut st = SidecarStats::default();
        for r in rows {
            let n: i64 = r.get("n");
            match r.get::<i64, _>("status") {
                0 => st.pending = n as u64,
                1 => st.running = n as u64,
                2 => st.done = n as u64,
                3 => st.skipped = n as u64,
                _ => st.failed = n as u64,
            }
        }
        Ok(st)
    }
}

/// 产物落盘通道（二进制产物：缩略图/向量文件等）。文本产物走 detail 列。
pub trait BlobSink: Send + Sync {
    /// 落盘并返回产物引用（如 `fs:<content>/<stage>.bin`）。
    ///
    /// # Errors
    /// IO 错误 → Fatal。
    fn put(&self, content_id: &str, stage: &str, bytes: &[u8]) -> Result<String, PartisyError>;

    /// 读回产物（消费方/测试用）。
    ///
    /// # Errors
    /// IO 错误 → Fatal。
    fn get(&self, content_id: &str, stage: &str) -> Result<Option<Vec<u8>>, PartisyError>;
}

fn io_err(what: &str, e: std::io::Error) -> PartisyError {
    PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("{what}: {e}").into()),
    }
}

/// 文件区 BlobSink：`<root>/<content_id>/<stage>.bin`，tmp+rename 原子写
/// （沿 tier.rs 先例）。产物引用 = `fs:<content_id>/<stage>.bin`。
pub struct FsBlobSink {
    root: PathBuf,
}

impl FsBlobSink {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        FsBlobSink { root: root.into() }
    }

    fn path_for(&self, content_id: &str, stage: &str) -> PathBuf {
        self.root.join(content_id).join(format!("{stage}.bin"))
    }
}

impl BlobSink for FsBlobSink {
    fn put(&self, content_id: &str, stage: &str, bytes: &[u8]) -> Result<String, PartisyError> {
        let final_path = self.path_for(content_id, stage);
        let dir = final_path.parent().ok_or_else(|| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("产物路径无父目录: {}", final_path.display()).into()),
        })?;
        std::fs::create_dir_all(dir).map_err(|e| io_err("创建产物目录", e))?;
        let tmp = dir.join(format!(".{stage}.tmp-{}", partisync_core::Ulid::now()));
        std::fs::write(&tmp, bytes).map_err(|e| io_err("写产物 tmp", e))?;
        std::fs::rename(&tmp, &final_path).map_err(|e| io_err("产物原子改名", e))?;
        Ok(format!("fs:{content_id}/{stage}.bin"))
    }

    fn get(&self, content_id: &str, stage: &str) -> Result<Option<Vec<u8>>, PartisyError> {
        let p = self.path_for(content_id, stage);
        if !p.exists() {
            return Ok(None);
        }
        std::fs::read(&p).map(Some).map_err(|e| io_err("读产物", e))
    }
}

/// 内存 BlobSink（测试/桩）。
#[derive(Default)]
pub struct InMemoryBlobSink {
    blobs: Mutex<HashMap<(String, String), Vec<u8>>>,
}

impl InMemoryBlobSink {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl BlobSink for InMemoryBlobSink {
    fn put(&self, content_id: &str, stage: &str, bytes: &[u8]) -> Result<String, PartisyError> {
        self.blobs
            .lock()
            .map(|mut m| {
                m.insert((content_id.to_owned(), stage.to_owned()), bytes.to_vec());
            })
            .map_err(|_| PartisyError {
                severity: Severity::Fatal,
                source: Some("blob sink 锁中毒".into()),
            })?;
        Ok(format!("mem:{content_id}/{stage}.bin"))
    }

    fn get(&self, content_id: &str, stage: &str) -> Result<Option<Vec<u8>>, PartisyError> {
        self.blobs
            .lock()
            .map(|m| m.get(&(content_id.to_owned(), stage.to_owned())).cloned())
            .map_err(|_| PartisyError {
                severity: Severity::Fatal,
                source: Some("blob sink 锁中毒".into()),
            })
    }
}
