//! stage1b EXIF 抽取：kamadak-exif 0.6 实装（ADR-0017，SPEC 裁定 3）。
//! 契约：Image 适用；容器无 EXIF/解析失败 = Skipped("no-exif: …")（降级
//! 非失败——EXIF 缺失是常态）；字段集 BTreeMap 序列化，同输入字节级稳定。

use std::collections::BTreeMap;
use std::io::Cursor;

use exif::Reader;

use crate::pipeline::{MimeKind, SidecarStage, StageError, StageInput, StageOutput};
use crate::sidecar::stage_ids;

/// EXIF 抽取 stage。
#[derive(Debug, Clone, Default)]
pub struct ExifStage;

impl ExifStage {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl SidecarStage for ExifStage {
    fn stage(&self) -> &'static str {
        stage_ids::EXIF
    }

    fn applicable(&self, mime: &MimeKind) -> bool {
        *mime == MimeKind::Image
    }

    fn run(&self, input: &StageInput) -> Result<StageOutput, StageError> {
        let mut cursor = Cursor::new(&input.data);
        let exif = Reader::new()
            .read_from_container(&mut cursor)
            .map_err(|e| StageError::Skipped(format!("no-exif: {e}")))?;
        let mut fields = BTreeMap::new();
        for f in exif.fields() {
            fields
                .entry(f.tag.to_string())
                .or_insert_with(|| f.display_value().to_string());
        }
        if fields.is_empty() {
            return Err(StageError::Skipped("no-exif: 容器无 EXIF 字段".into()));
        }
        let json = serde_json::to_string(&fields)
            .map_err(|e| StageError::Failed(format!("EXIF 序列化失败: {e}")))?;
        Ok(StageOutput {
            detail: Some(json),
            ..StageOutput::default()
        })
    }
}
