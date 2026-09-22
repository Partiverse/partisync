//! stage1a 缩略图：image 0.25 实装（ADR-0017，SPEC M4-WP01 裁定 3）。
//! 契约：Image 适用；解码失败 = Failed；产物 JPEG ≤ max_dim 保比、
//! 同输入字节级稳定（验收「阶段幂等」）。

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;

use crate::pipeline::{MimeKind, SidecarStage, StageError, StageInput, StageOutput};
use crate::sidecar::stage_ids;

/// 缩略图最大边长（像素）。
pub const THUMB_MAX_DIM: u32 = 512;

/// JPEG 编码质量（0.25 默认 75 显式钉住，保证跨版本行为可见）。
pub const THUMB_JPEG_QUALITY: u8 = 75;

/// 缩略图 stage。
#[derive(Debug, Clone)]
pub struct ThumbnailStage {
    pub max_dim: u32,
}

impl ThumbnailStage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_dim: THUMB_MAX_DIM,
        }
    }
}

impl Default for ThumbnailStage {
    fn default() -> Self {
        Self::new()
    }
}

impl SidecarStage for ThumbnailStage {
    fn stage(&self) -> &'static str {
        stage_ids::THUMBNAIL
    }

    fn applicable(&self, mime: &MimeKind) -> bool {
        *mime == MimeKind::Image
    }

    fn run(&self, input: &StageInput) -> Result<StageOutput, StageError> {
        let img = image::load_from_memory(&input.data)
            .map_err(|e| StageError::Failed(format!("图片解码失败: {e}")))?;
        let thumb = img.thumbnail(self.max_dim, self.max_dim);
        let (w, h) = (thumb.width(), thumb.height());
        let mut buf = Cursor::new(Vec::new());
        thumb
            .write_with_encoder(JpegEncoder::new_with_quality(&mut buf, THUMB_JPEG_QUALITY))
            .map_err(|e| StageError::Failed(format!("缩略图编码失败: {e}")))?;
        Ok(StageOutput {
            blob: Some(buf.into_inner()),
            detail: Some(format!("{w}x{h} jpeg")),
            ..StageOutput::default()
        })
    }
}
