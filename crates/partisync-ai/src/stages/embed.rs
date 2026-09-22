//! stage4 嵌入：fastembed BGE-M3（文本）+ CLIP ViT-B/32（图像）
//! （SPEC M4-WP01 任务卡 T04；ADR-0017）。
//!
//! - 真实路径 [`EmbedStage`]（`ai-embed` feature，默认关——CI 不背
//!   ort/onnxruntime 构建链）；模型可用性走 [`ModelManager`] 缺失降级；
//!   fastembed 初始化/推理失败同映射 Skipped（离线语义，裁定 5）。
//! - CI 路径 [`FakeEmbedStage`]（恒编译）：同一条 ModelManager 降级
//!   语义 + 确定性伪向量，离线钉住管线契约（验收「fake 模型」）。
//! - Audio/Video 不适用（转写 T05 进场后经 transcript 嵌入，届时
//!   放宽 applicable）。

#[cfg(feature = "ai-embed")]
use std::sync::Mutex;

use crate::models::{ModelManager, EMBED_MODEL_IMAGE, EMBED_MODEL_TEXT};
use crate::pipeline::{MimeKind, SidecarStage, StageError, StageInput, StageOutput};
use crate::sidecar::stage_ids;

/// fake 向量维度（CI 契约钉子；真实模型维度以推理结果为准）。
pub const FAKE_EMBED_DIM: usize = 8;

/// f32 向量 → 小端字节（BlobSink 落盘口径，WP02 检索按此读回）。
#[must_use]
pub fn vector_to_le_bytes(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

fn map_verify_err(e: crate::models::ModelError) -> StageError {
    match e {
        crate::models::ModelError::Unknown(_) => StageError::Failed(e.to_string()),
        _ => StageError::Skipped(e.to_string()),
    }
}

/// CI/离线嵌入 stage：ModelManager 判定可用性，缺失 → skipped；
/// 向量由 content_id 确定性派生（同输入字节级稳定，验收「阶段幂等」）。
#[derive(Debug, Clone)]
pub struct FakeEmbedStage {
    manager: ModelManager,
    dim: usize,
}

impl FakeEmbedStage {
    #[must_use]
    pub fn new(manager: ModelManager) -> Self {
        Self {
            manager,
            dim: FAKE_EMBED_DIM,
        }
    }

    fn model_for(mime: &MimeKind) -> &'static str {
        match mime {
            MimeKind::Image => EMBED_MODEL_IMAGE,
            _ => EMBED_MODEL_TEXT,
        }
    }
}

impl SidecarStage for FakeEmbedStage {
    fn stage(&self) -> &'static str {
        stage_ids::EMBED
    }

    fn applicable(&self, mime: &MimeKind) -> bool {
        matches!(mime, MimeKind::Image | MimeKind::Text)
    }

    fn run(&self, input: &StageInput) -> Result<StageOutput, StageError> {
        let model = Self::model_for(&input.mime);
        self.manager.verify(model).map_err(map_verify_err)?;
        // 确定性伪向量：blake3(content_id) 播种 xorshift64 填充。
        let seed = blake3::hash(input.content_id.as_bytes());
        let seed = u64::from_le_bytes(
            seed.as_bytes()[..8]
                .try_into()
                .expect("blake3 输出 ≥ 8 字节"),
        );
        let mut rng = seed | 1;
        let v: Vec<f32> = (0..self.dim)
            .map(|_| {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                (rng % 20_001) as f32 / 10_000.0 - 1.0
            })
            .collect();
        Ok(StageOutput {
            blob: Some(vector_to_le_bytes(&v)),
            detail: Some(format!("dim={} model={model} (fake)", self.dim)),
            ..StageOutput::default()
        })
    }
}

/// 真实嵌入 stage（fastembed；`ai-embed` feature 编译）。
#[cfg(feature = "ai-embed")]
pub struct EmbedStage {
    manager: ModelManager,
    text: Mutex<Option<fastembed::TextEmbedding>>,
    image: Mutex<Option<fastembed::ImageEmbedding>>,
}

#[cfg(feature = "ai-embed")]
impl EmbedStage {
    pub fn new(manager: ModelManager) -> Self {
        Self {
            manager,
            text: Mutex::new(None),
            image: Mutex::new(None),
        }
    }

    fn ensure_text(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<fastembed::TextEmbedding>>, StageError> {
        let mut guard = self
            .text
            .lock()
            .map_err(|_| StageError::Failed("embed text 模型锁中毒".into()))?;
        if guard.is_none() {
            let cache = self.manager.cache_dir().to_owned();
            let opts = fastembed::TextInitOptions::new(fastembed::EmbeddingModel::BGEM3)
                .with_cache_dir(cache)
                .with_show_download_progress(false);
            let model = fastembed::TextEmbedding::try_new(opts)
                .map_err(|e| StageError::Skipped(format!("embed-text 加载失败: {e}")))?;
            *guard = Some(model);
        }
        Ok(guard)
    }

    fn ensure_image(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<fastembed::ImageEmbedding>>, StageError> {
        let mut guard = self
            .image
            .lock()
            .map_err(|_| StageError::Failed("embed image 模型锁中毒".into()))?;
        if guard.is_none() {
            let cache = self.manager.cache_dir().to_owned();
            let opts = fastembed::ImageInitOptions::new(fastembed::ImageEmbeddingModel::ClipVitB32)
                .with_cache_dir(cache)
                .with_show_download_progress(false);
            let model = fastembed::ImageEmbedding::try_new(opts)
                .map_err(|e| StageError::Skipped(format!("embed-image 加载失败: {e}")))?;
            *guard = Some(model);
        }
        Ok(guard)
    }
}

#[cfg(feature = "ai-embed")]
impl SidecarStage for EmbedStage {
    fn stage(&self) -> &'static str {
        stage_ids::EMBED
    }

    fn applicable(&self, mime: &MimeKind) -> bool {
        matches!(mime, MimeKind::Image | MimeKind::Text)
    }

    fn run(&self, input: &StageInput) -> Result<StageOutput, StageError> {
        match input.mime {
            MimeKind::Image => {
                self.manager
                    .verify(EMBED_MODEL_IMAGE)
                    .map_err(map_verify_err)?;
                let mut guard = self.ensure_image()?;
                let model = guard.as_mut().expect("ensure_image 后模型必然已初始化");
                let v = model
                    .embed_bytes(&[input.data.as_slice()], None)
                    .map_err(|e| StageError::Failed(format!("图像嵌入推理失败: {e}")))?
                    .pop()
                    .ok_or_else(|| StageError::Failed("图像嵌入空输出".into()))?;
                Ok(detail_out(&v, EMBED_MODEL_IMAGE))
            }
            MimeKind::Text => {
                self.manager
                    .verify(EMBED_MODEL_TEXT)
                    .map_err(map_verify_err)?;
                let text = std::str::from_utf8(&input.data)
                    .map_err(|e| StageError::Failed(format!("文本解码失败: {e}")))?;
                let mut guard = self.ensure_text()?;
                let model = guard.as_mut().expect("ensure_text 后模型必然已初始化");
                let v = model
                    .embed(&[text], None)
                    .map_err(|e| StageError::Failed(format!("文本嵌入推理失败: {e}")))?
                    .pop()
                    .ok_or_else(|| StageError::Failed("文本嵌入空输出".into()))?;
                Ok(detail_out(&v, EMBED_MODEL_TEXT))
            }
            _ => Err(StageError::Skipped(
                "embed-requires-transcript（音视频嵌入待 T05 转写进场）".into(),
            )),
        }
    }
}

#[cfg(feature = "ai-embed")]
fn detail_out(v: &[f32], model: &str) -> StageOutput {
    StageOutput {
        blob: Some(vector_to_le_bytes(v)),
        detail: Some(format!("dim={} model={model}", v.len())),
        ..StageOutput::default()
    }
}

/// 默认嵌入 stage：feature 开 → 真实；关 → CI fake（同一降级语义）。
#[must_use]
pub fn default_embed_stage() -> std::sync::Arc<dyn SidecarStage> {
    let manager = ModelManager::from_env();
    #[cfg(feature = "ai-embed")]
    {
        std::sync::Arc::new(EmbedStage::new(manager))
    }
    #[cfg(not(feature = "ai-embed"))]
    {
        std::sync::Arc::new(FakeEmbedStage::new(manager))
    }
}
