//! stage6 C2PA 摄取校验：c2pa 0.90.22 只读校验 + manifest 保留（ADR-0020，
//! SPEC M4-WP05 裁定 1/2/3）。
//!
//! 语义：`verify_trust=false` 的离线确定性 Context（信任链判定属环境相关
//! 属性，非摄取校验义务，SPEC 非目标）；无 manifest = done("state=absent")
//! ——完成的校验结论，非失败非降级；容器不支持 = Skipped；其他解析错误 =
//! Failed。**无效清单同样保留**（保留义务是证据学义务）——blob 携带
//! manifest store report JSON，state 进 detail，由 pipeline 回写
//! `content.c2pa` 列（裁定 3）。
//!
//! API 口径核实出处：c2pa 0.90.22 源码 `reader.rs`（`from_context` :166 /
//! `with_stream` :221，自由函数 `from_stream` 已 deprecated）、`context.rs`
//! （`new` :319 / `with_settings` :366）、`validation_results.rs`
//! （`ValidationState` :36）、`error.rs`（`JumbfNotFound` :160 /
//! `UnsupportedType` :178）。

use std::io::Cursor;

use c2pa::{Context, Error as C2paError, Reader};

use crate::pipeline::{MimeKind, SidecarStage, StageError, StageInput, StageOutput};
use crate::sidecar::stage_ids;

/// 离线确定性校验 Context：关闭信任链核验（ADR-0020「后果」节）。
/// 每次 run 新建（Context 随 settings 内化，无跨线程共享承诺）。
fn offline_context() -> Result<Context, StageError> {
    Context::new()
        .with_settings(r#"{"verify": {"verify_trust": false}}"#)
        .map_err(|e| StageError::Failed(format!("c2pa settings: {e}")))
}

/// 粗粒度格式提示（每 kind 取代表容器 mime）。提示串仅兜底：c2pa
/// `format_from_stream`（0.90.22 `jumbf_io.rs` :227）以字节嗅探为准，
/// 提示与实际容器不一致时以检出格式优先；嗅探失败的字节本就非有效
/// 容器，最终归 UnsupportedType/absent（判定表覆盖）。
fn format_hint(mime: &MimeKind) -> &'static str {
    match mime {
        MimeKind::Image => "image/jpeg",
        MimeKind::Video => "video/mp4",
        MimeKind::Audio => "audio/wav",
        _ => "application/octet-stream",
    }
}

/// C2PA 校验 stage。
#[derive(Debug, Clone, Default)]
pub struct C2paStage;

impl C2paStage {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl SidecarStage for C2paStage {
    fn stage(&self) -> &'static str {
        stage_ids::C2PA
    }

    fn applicable(&self, mime: &MimeKind) -> bool {
        matches!(mime, MimeKind::Image | MimeKind::Video | MimeKind::Audio)
    }

    fn run(&self, input: &StageInput) -> Result<StageOutput, StageError> {
        let context = offline_context()?;
        // 无清单 = 正常结论（JumbfNotFound → done "state=absent"），故
        // 错误在此处三分而不是一律 map_err 成 Err（Err 只有降级/失败）。
        let reader = match Reader::from_context(context)
            .with_stream(format_hint(&input.mime), Cursor::new(&input.data))
        {
            Ok(reader) => reader,
            Err(C2paError::JumbfNotFound) => {
                return Ok(StageOutput {
                    detail: Some("state=absent".into()),
                    ..StageOutput::default()
                });
            }
            Err(C2paError::UnsupportedType) => {
                return Err(StageError::Skipped("format-unsupported-by-c2pa".into()));
            }
            Err(other) => return Err(StageError::Failed(format!("c2pa: {other}"))),
        };

        // 无效清单同样保留：json() 无条件返回 manifest store report，
        // 校验结论只进 detail（json_checked() 才以校验失败为 Err）。
        let json = reader.json();
        let state = reader.validation_state();
        let detail = format!(
            "state={state:?} label={} bytes={}",
            reader.active_label().unwrap_or("-"),
            json.len(),
        );
        Ok(StageOutput {
            blob: Some(json.into_bytes()),
            detail: Some(detail),
            ..StageOutput::default()
        })
    }
}
