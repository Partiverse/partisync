//! AI 层：Sidecar 管线（M4-WP01）、fastembed/whisper 推理、MCP
//! server(rmcp)、数据集出口、C2PA（后四项按里程碑推进）。
//!
//! 当前内容（M4-WP01-T02）：JobSystem v2 编排（[`jobs`]）、Stage trait
//! 与管线执行器（[`pipeline`]）、per-content per-stage 状态机与产物
//! 落盘（[`sidecar`]）、缩略图/EXIF 实装 + 占位降级（[`stages`]）。
//! 依赖隔离：ort/fastembed/whisper 类型不穿透 trait 边界（ADR-0015
//! 裁定 1 先例，ADR-0017 feature 门控）。

pub mod jobs;
pub mod pipeline;
pub mod sidecar;
pub mod stages;

pub use jobs::{get_job, resume_latest_sidecar_job, run_sidecar_job, JobOutcome};
pub use pipeline::{
    ContentLoader, InMemoryContentLoader, MimeKind, Pipeline, PipelineSummary, SidecarStage,
    StageError, StageInput, StageOutput,
};
pub use sidecar::{
    stage_ids, BlobSink, FsBlobSink, InMemoryBlobSink, ItemStatus, SidecarItemRow, SidecarStats,
    SidecarStore, STAGE_ORDER,
};
pub use stages::{default_stages, ExifStage, PlaceholderStage, ThumbnailStage};
