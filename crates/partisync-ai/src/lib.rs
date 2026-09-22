//! AI 层：Sidecar 管线（M4-WP01）、fastembed/whisper 推理、MCP
//! server(rmcp)、数据集出口、C2PA（后四项按里程碑推进）。
//!
//! 当前内容（至 M4-WP01-T04）：JobSystem v2 编排（[`jobs`]）、Stage
//! trait 与管线执行器（[`pipeline`]）、per-content per-stage 状态机与
//! 产物落盘（[`sidecar`]）、模型清单钉版/缓存/缺失降级（[`models`]）、
//! 缩略图/EXIF/嵌入实装 + 占位降级（[`stages`]）。依赖隔离：
//! ort/fastembed/whisper 类型不穿透 trait 边界（ADR-0015 裁定 1 先例，
//! ADR-0017 feature 门控）。

pub mod jobs;
pub mod models;
pub mod pipeline;
pub mod sidecar;
pub mod stages;

pub use jobs::{
    close_stale_sidecar_jobs, get_job, resume_latest_sidecar_job, run_pending_sidecars,
    run_sidecar_job, sidecar_auto_enqueue, JobOutcome, LocalContentLoader, SidecarRunReport,
};
pub use models::{
    ModelError, ModelManager, ModelSpec, EMBED_MODEL_IMAGE, EMBED_MODEL_TEXT, MANIFEST,
};
pub use pipeline::{
    ContentLoader, InMemoryContentLoader, MimeKind, Pipeline, PipelineSummary, SidecarStage,
    StageError, StageInput, StageOutput,
};
pub use sidecar::{
    stage_ids, BlobSink, FsBlobSink, InMemoryBlobSink, ItemStatus, SidecarItemRow, SidecarStats,
    SidecarStore, STAGE_ORDER,
};
#[cfg(feature = "ai-embed")]
pub use stages::EmbedStage;
pub use stages::{
    default_embed_stage, default_stages, ExifStage, FakeEmbedStage, PlaceholderStage,
    ThumbnailStage,
};
