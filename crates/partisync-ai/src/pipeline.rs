//! Sidecar 管线：Stage trait + 去重/幂等/降级驱动的执行器
//! （SPEC M4-WP01 契约 §1/§2）。
//!
//! 依赖隔离：ort/fastembed/whisper 类型只在 `stages` 内部可见，不穿透
//! [`SidecarStage`] trait 边界（学 ADR-0015 裁定 1）。管线执行语义：
//!
//! - stage 次序固定（[`sidecar::STAGE_ORDER`]），逐 stage 认领（原子）；
//! - 不适用 / 未注册 / 模型缺失 → skipped + 原因可查，后续 stage 照常
//!   （裁定 5 降级语义）；
//! - 意外失败 → failed，后续 stage 照常，作业终态 = 部分完成；
//! - 崩溃残留 running 行由 job 层 [`jobs::run_sidecar_job`] 入口统一复位。

use std::sync::Arc;

use partisync_core::error::{PartisyError, Severity};
use partisync_graph::store::Store;

use crate::sidecar::{self, stage_ids, ItemStatus, SidecarStats, SidecarStore};

/// 内容粗分类（mime 前缀归并；stage 的 applicable 判据）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MimeKind {
    Image,
    Audio,
    Video,
    Text,
    Other,
}

impl MimeKind {
    #[must_use]
    pub fn parse(mime: Option<&str>) -> Self {
        match mime
            .unwrap_or("")
            .split('/')
            .collect::<Vec<_>>()
            .first()
            .copied()
        {
            Some("image") => MimeKind::Image,
            Some("audio") => MimeKind::Audio,
            Some("video") => MimeKind::Video,
            Some("text") => MimeKind::Text,
            _ => MimeKind::Other,
        }
    }
}

/// 内容字节加载器（真实实现 = CAS 读，T06 接线；测试用内存桩）。
pub trait ContentLoader: Send + Sync {
    ///
    /// # Errors
    /// 内容缺失/读失败 → Fatal。
    fn load(&self, content_id: &str) -> Result<Vec<u8>, PartisyError>;

    /// mime 提示（content.mime 列为 NULL 时的兜底，如按扩展名猜测；
    /// 真实 scan 现状 indexer 不写 mime 列——T06 实测确认）。默认 None。
    fn mime(&self, _content_id: &str) -> Option<String> {
        None
    }
}

/// 内存 ContentLoader（测试/T06 前的桩）。
#[derive(Default, Clone)]
pub struct InMemoryContentLoader {
    map: Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>>,
}

impl InMemoryContentLoader {
    #[must_use]
    pub fn new(entries: impl IntoIterator<Item = (String, Vec<u8>)>) -> Self {
        let loader = Self::default();
        {
            let mut m = loader.map.lock().expect("loader 锁");
            for (k, v) in entries {
                m.insert(k, v);
            }
        }
        loader
    }
}

impl ContentLoader for InMemoryContentLoader {
    fn load(&self, content_id: &str) -> Result<Vec<u8>, PartisyError> {
        self.map
            .lock()
            .map(|m| {
                m.get(content_id).cloned().ok_or_else(|| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("内容缺失: {content_id}").into()),
                })
            })
            .map_err(|_| PartisyError {
                severity: Severity::Fatal,
                source: Some("content loader 锁中毒".into()),
            })?
    }
}

/// stage 输入。
pub struct StageInput {
    pub content_id: String,
    pub mime: MimeKind,
    pub data: Vec<u8>,
}

/// stage 输出：blob（二进制产物，经 BlobSink 落盘）/ artifact（引用）/
/// detail（摘要文本，如尺寸/字段数/向量维度）。
#[derive(Debug, Default)]
pub struct StageOutput {
    pub blob: Option<Vec<u8>>,
    pub artifact: Option<String>,
    pub detail: Option<String>,
}

/// stage 执行错误：Skipped = 降级（模型缺失等，语义内）；Failed = 意外失败。
#[derive(Debug)]
pub enum StageError {
    Skipped(String),
    Failed(String),
}

/// Sidecar 阶段契约。实现须幂等：同输入产物确定（验收「阶段幂等」）。
pub trait SidecarStage: Send + Sync {
    /// stage 标识（[`sidecar::stage_ids`] 常量）。
    fn stage(&self) -> &'static str;

    /// 该 mime 是否适用（不适用的行由管线标 skipped("not-applicable")）。
    fn applicable(&self, mime: &MimeKind) -> bool;

    /// 执行（CPU/推理密集，管线在 blocking 线程池驱动）。
    ///
    /// # Errors
    /// [`StageError::Skipped`] 降级 / [`StageError::Failed`] 意外失败。
    fn run(&self, input: &StageInput) -> Result<StageOutput, StageError>;
}

/// 单 content 一轮执行的摘要。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PipelineSummary {
    pub done: u64,
    pub skipped: u64,
    pub failed: u64,
}

impl PipelineSummary {
    /// 本轮实际驱动到终态的行数。
    #[must_use]
    pub fn processed(&self) -> u64 {
        self.done + self.skipped + self.failed
    }
}

fn err(what: &str, msg: String) -> PartisyError {
    PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("{what}: {msg}").into()),
    }
}

/// 管线执行器：注册 stages + 产物落盘通道。
pub struct Pipeline {
    stages: Vec<Arc<dyn SidecarStage>>,
    sink: Arc<dyn sidecar::BlobSink>,
}

impl Pipeline {
    #[must_use]
    pub fn new(sink: Arc<dyn sidecar::BlobSink>, stages: Vec<Arc<dyn SidecarStage>>) -> Self {
        Pipeline { stages, sink }
    }

    fn find_stage(&self, id: &str) -> Option<&Arc<dyn SidecarStage>> {
        self.stages.iter().find(|s| s.stage() == id)
    }

    /// 已注册 stage 标识（调度器过滤 pending 用——未注册 stage 的行
    /// 保持 pending 且不驱动，防重复开作业活锁）。
    #[must_use]
    pub fn stage_ids(&self) -> Vec<&'static str> {
        self.stages.iter().map(|s| s.stage()).collect()
    }

    /// 跑单 content 全管线。行不存在 → 先入队（幂等）；任一 stage 降级
    /// 或失败不中断后续（SPEC 契约 §1）。
    ///
    /// # Errors
    /// content 不存在 / DB 错误 → Fatal。
    pub async fn run_content(
        &self,
        store: &Store,
        loader: &dyn ContentLoader,
        content_id: &str,
    ) -> Result<PipelineSummary, PartisyError> {
        let items = SidecarStore::new(store);
        items.ensure_enqueued(content_id).await?;

        // content.mime 列可为 NULL（indexer 现状不写）——行存在性用
        // Option<Option<String>> 区分，mime 缺失时回落 loader 提示。
        let row: Option<Option<String>> =
            sqlx::query_scalar("SELECT mime FROM content WHERE id = ?")
                .bind(content_id)
                .fetch_optional(store.pool_ref())
                .await
                .map_err(|e| err("读 content", e.to_string()))?;
        let Some(mime_col) = row else {
            return Err(err("读 content", format!("不存在: {content_id}")));
        };
        let mime_kind = match mime_col.or_else(|| loader.mime(content_id)) {
            Some(ref m) => MimeKind::parse(Some(m)),
            None => MimeKind::parse(None),
        };

        // 内容字节整轮只加载一次；失败即作业级失败（CAS 缺块属系统性），
        // 不留 running 行（尚未认领，下次 run 直接待处理）。
        let data = loader.load(content_id)?;

        let mut summary = PipelineSummary::default();
        for row in items.items(content_id).await? {
            // 只驱动 pending 行：已终态（done/skipped/failed）不再重算
            // （幂等，验收「阶段幂等」）；running 残留由 job 入口
            // reset_running 复位后自然进入本轮。
            if row.status != ItemStatus::Pending as i64 {
                continue;
            }
            // 未注册 stage 保持 pending（不算终态）：T04/T05 模型进场后
            // 注册补算，不被「暂时无实现」永久阻塞（降级语义只属于
            // 「已注册但模型缺失」，见 PlaceholderStage）。
            let Some(stage) = self.find_stage(&row.stage) else {
                continue;
            };
            if !stage.applicable(&mime_kind) {
                items
                    .mark_skipped(content_id, &row.stage, "not-applicable")
                    .await?;
                summary.skipped += 1;
                continue;
            }
            // 原子认领：pending → running。认领失败 = 他方并发在跑（no-op）。
            if !items.claim(content_id, &row.stage).await? {
                continue;
            }
            let input = StageInput {
                content_id: content_id.to_owned(),
                mime: mime_kind,
                data: data.clone(),
            };
            let stage = Arc::clone(stage);
            // CPU/推理密集 → blocking 池，避免饿死 runtime。
            let outcome = tokio::task::spawn_blocking(move || stage.run(&input))
                .await
                .map_err(|e| err("stage 任务中止", e.to_string()))?;
            match outcome {
                Ok(out) => {
                    // M4-WP05 裁定 3：c2pa manifest JSON 额外落 content.c2pa
                    // 列（SQL 级可查）；列写入失败即 stage 失败，不静默丢清单。
                    let c2pa_json = (row.stage == stage_ids::C2PA)
                        .then(|| out.blob.clone())
                        .flatten();
                    let artifact = match out.blob {
                        Some(blob) => Some(self.sink.put(content_id, &row.stage, &blob)?),
                        None => out.artifact,
                    };
                    if let Some(json_bytes) = c2pa_json {
                        let json = std::str::from_utf8(&json_bytes)
                            .map_err(|e| err("c2pa 清单编码", e.to_string()))?;
                        sqlx::query("UPDATE content SET c2pa = ? WHERE id = ?")
                            .bind(json)
                            .bind(content_id)
                            .execute(store.pool_ref())
                            .await
                            .map_err(|e| err("写 content.c2pa", e.to_string()))?;
                    }
                    items
                        .mark_done(
                            content_id,
                            &row.stage,
                            artifact.as_deref(),
                            out.detail.as_deref(),
                        )
                        .await?;
                    summary.done += 1;
                }
                Err(StageError::Skipped(reason)) => {
                    items.mark_skipped(content_id, &row.stage, &reason).await?;
                    summary.skipped += 1;
                }
                Err(StageError::Failed(reason)) => {
                    items.mark_failed(content_id, &row.stage, &reason).await?;
                    summary.failed += 1;
                }
            }
        }
        Ok(summary)
    }

    /// 消费方读回产物（WP02 检索/MCP 面用）。
    ///
    /// # Errors
    /// IO 错误 → Fatal。
    pub fn read_artifact(
        &self,
        content_id: &str,
        stage: &str,
    ) -> Result<Option<Vec<u8>>, PartisyError> {
        self.sink.get(content_id, stage)
    }
}

/// T02 默认 stage 集注册序提示：缩略图+EXIF 实装，OCR/转写/嵌入在
/// feature/模型进场前走 stage-not-registered 降级（SPEC 裁定 5）。
pub const T02_IMPLEMENTED_STAGES: [&str; 2] = [stage_ids::THUMBNAIL, stage_ids::EXIF];

/// 状态计数转发（作业/CLI 层摘要用）。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn stats(store: &Store) -> Result<SidecarStats, PartisyError> {
    SidecarStore::new(store).stats().await
}
