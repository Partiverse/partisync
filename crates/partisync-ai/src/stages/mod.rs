//! stage 集注册（T02：缩略图+EXIF 实装；OCR/转写/嵌入在 T04/T05 进场前
//! 走占位降级——SPEC M4-WP01 裁定 5）。

pub mod exif;
pub mod thumbnail;

use std::sync::Arc;

use crate::pipeline::{MimeKind, SidecarStage, StageError, StageInput, StageOutput};
use crate::sidecar::stage_ids;

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

/// T02 默认 stage 集：缩略图+EXIF 实装，后三阶段占位降级。
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
        Arc::new(PlaceholderStage::new(
            stage_ids::EMBED,
            "embed 模型未进场（T04 交付）",
        )),
    ]
}
