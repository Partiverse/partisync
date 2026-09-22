//! stage 集注册（T04：缩略图/EXIF/嵌入实装；OCR/转写 T05 进场前占位
//! 降级——SPEC M4-WP01 裁定 5）。

pub mod embed;
pub mod exif;
pub mod thumbnail;

use std::sync::Arc;

use crate::pipeline::{MimeKind, SidecarStage, StageError, StageInput, StageOutput};
use crate::sidecar::stage_ids;

#[cfg(feature = "ai-embed")]
pub use embed::EmbedStage;
pub use embed::{default_embed_stage, FakeEmbedStage, FAKE_EMBED_DIM};
pub use exif::ExifStage;
pub use thumbnail::{ThumbnailStage, THUMB_JPEG_QUALITY, THUMB_MAX_DIM};

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

/// 默认 stage 集：缩略图+EXIF 实装；OCR/转写占位（T05）；嵌入走
/// [`default_embed_stage`]（T04：feature 开=真实 fastembed，关=CI fake，
/// 同一条模型缺失降级语义）。
#[must_use]
pub fn default_stages() -> Vec<Arc<dyn SidecarStage>> {
    vec![
        Arc::new(ThumbnailStage::new()),
        Arc::new(ExifStage::new()),
        Arc::new(PlaceholderStage::new(
            stage_ids::OCR,
            "ocr 模型未进场（T05 交付）",
        )),
        Arc::new(PlaceholderStage::new(
            stage_ids::TRANSCRIBE,
            "transcribe 模型未进场（T05 交付）",
        )),
        default_embed_stage(),
    ]
}
