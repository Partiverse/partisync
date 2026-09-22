//! stage 集注册（T05 后五阶段全部实装：feature 开=真实推理栈，
//! 关=CI fake，同一条 ModelManager 模型缺失降级语义——裁定 5）。

pub mod c2pa;
pub mod embed;
pub mod exif;
pub mod ocr;
pub mod thumbnail;
pub mod transcribe;

use std::sync::Arc;

use crate::pipeline::{MimeKind, SidecarStage, StageError, StageInput, StageOutput};

pub use c2pa::C2paStage;
#[cfg(feature = "ai-embed")]
pub use embed::EmbedStage;
pub use embed::{default_embed_stage, FakeEmbedStage, FAKE_EMBED_DIM};
pub use exif::ExifStage;
#[cfg(feature = "ai-ocr")]
pub use ocr::OcrStage;
pub use ocr::{default_ocr_stage, FakeOcrStage};
pub use thumbnail::{ThumbnailStage, THUMB_JPEG_QUALITY, THUMB_MAX_DIM};
#[cfg(feature = "ai-transcribe")]
pub use transcribe::TranscribeStage;
pub use transcribe::{default_transcribe_stage, FakeTranscribeStage};

/// 占位 stage：模型/feature 未进场时注册，一律 Skipped（原因可查）。
#[derive(Debug, Clone)]
pub struct PlaceholderStage {
    id: &'static str,
    reason: &'static str,
}

impl PlaceholderStage {
    #[must_use]
    pub fn new(id: &'static str, reason: &'static str) -> Self {
        Self { id, reason }
    }
}

impl SidecarStage for PlaceholderStage {
    fn stage(&self) -> &'static str {
        self.id
    }

    fn applicable(&self, _mime: &MimeKind) -> bool {
        true
    }

    fn run(&self, _input: &StageInput) -> Result<StageOutput, StageError> {
        Err(StageError::Skipped(self.reason.to_owned()))
    }
}

/// 默认 stage 集（五阶段 + C2PA 校验，M4-WP05）：推理栈 feature 开=真实，
/// 关=CI fake（同一条模型缺失降级语义）；c2pa 纯 Rust 轻栈不门控（ADR-0020）。
#[must_use]
pub fn default_stages() -> Vec<Arc<dyn SidecarStage>> {
    vec![
        Arc::new(ThumbnailStage::new()),
        Arc::new(ExifStage::new()),
        default_ocr_stage(),
        default_transcribe_stage(),
        default_embed_stage(),
        Arc::new(C2paStage::new()),
    ]
}
