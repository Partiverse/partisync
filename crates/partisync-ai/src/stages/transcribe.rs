//! stage3 转写：whisper-rs（whisper.cpp）+ symphonia 音频解码
//! （ADR-0017 批注，SPEC M4-WP01 裁定 3）。
//!
//! - 真实路径 [`TranscribeStage`]（`ai-transcribe` feature，默认关）：
//!   symphonia 解码 → 混音单声道 → 线性重采样 16 kHz → whisper 推理；
//!   模型 = ModelManager 缓存的 ggml（`model.bin`），缺失 → skipped。
//! - CI 路径 [`FakeTranscribeStage`]（恒编译）：同一 ModelManager 降级
//!   语义 + 确定性伪转写，离线钉行为契约（SPEC 验收「fake 模型」）。
//! - applicable 仅 Audio：视频容器解封装不支持（诚实披露；冒烟复核项）。

use std::sync::Arc;

use crate::models::{ModelManager, TRANSCRIBE_MODEL};
use crate::pipeline::{MimeKind, SidecarStage, StageError, StageInput, StageOutput};
use crate::sidecar::stage_ids;

/// whisper 输入采样率（whisper.cpp 硬约束）。
pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// 确定性伪转写（CI 契约；同输入字节级稳定）。
#[must_use]
pub fn fake_transcript(content_id: &str, n: usize) -> String {
    let seed = blake3::hash(content_id.as_bytes());
    let hex = seed.to_hex().to_string();
    const WORDS: [&str; 8] = ["转写", "样本", "离线", "契约", "测试", "音频", "文本", "桩"];
    let mut text = String::new();
    for i in 0..n {
        let b = hex.as_bytes()[i % hex.len()];
        text.push_str(WORDS[usize::from(b) % WORDS.len()]);
        if (i + 1) % 8 == 0 {
            text.push('\n');
        } else {
            text.push(' ');
        }
    }
    text
}

/// CI/离线转写 stage：ModelManager 判定可用性，缺失 → skipped。
#[derive(Debug, Clone)]
pub struct FakeTranscribeStage {
    manager: ModelManager,
    words: usize,
}

impl FakeTranscribeStage {
    #[must_use]
    pub fn new(manager: ModelManager) -> Self {
        Self { manager, words: 16 }
    }
}

impl SidecarStage for FakeTranscribeStage {
    fn stage(&self) -> &'static str {
        stage_ids::TRANSCRIBE
    }

    fn applicable(&self, mime: &MimeKind) -> bool {
        *mime == MimeKind::Audio
    }

    fn run(&self, input: &StageInput) -> Result<StageOutput, StageError> {
        self.manager
            .verify(TRANSCRIBE_MODEL)
            .map_err(map_verify_err)?;
        let text = fake_transcript(&input.content_id, self.words);
        Ok(StageOutput {
            blob: Some(text.clone().into_bytes()),
            detail: Some(format!(
                "chars={} model=transcribe-whisper (fake)",
                text.chars().count()
            )),
            ..StageOutput::default()
        })
    }
}

fn map_verify_err(e: crate::models::ModelError) -> StageError {
    match e {
        crate::models::ModelError::Unknown(_) => StageError::Failed(e.to_string()),
        _ => StageError::Skipped(e.to_string()),
    }
}

/// 线性插值重采样（纯函数，恒编译——CI 可测；whisper 需 16 kHz）。
#[must_use]
pub fn resample_to_16k(samples: &[f32], src_rate: u32) -> Vec<f32> {
    if src_rate == WHISPER_SAMPLE_RATE || samples.is_empty() || src_rate == 0 {
        return samples.to_vec();
    }
    let dst_len = (u64::from(samples.len() as u32) * u64::from(WHISPER_SAMPLE_RATE)
        / u64::from(src_rate)) as usize;
    let mut out = Vec::with_capacity(dst_len);
    let step = f64::from(src_rate) / f64::from(WHISPER_SAMPLE_RATE);
    for i in 0..dst_len {
        let pos = i as f64 * step;
        let i0 = pos.floor() as usize;
        let frac = (pos - i0 as f64) as f32;
        let s0 = samples[i0.min(samples.len() - 1)];
        let s1 = samples[(i0 + 1).min(samples.len() - 1)];
        out.push(s0 + (s1 - s0) * frac);
    }
    out
}

/// 真实转写 stage（whisper-rs；`ai-transcribe` feature 编译）。
#[cfg(feature = "ai-transcribe")]
pub struct TranscribeStage {
    manager: ModelManager,
    ctx: std::sync::Mutex<Option<whisper_rs::WhisperContext>>,
}

#[cfg(feature = "ai-transcribe")]
impl TranscribeStage {
    #[must_use]
    pub fn new(manager: ModelManager) -> Self {
        Self {
            manager,
            ctx: std::sync::Mutex::new(None),
        }
    }

    fn ensure_ctx(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<whisper_rs::WhisperContext>>, StageError> {
        let mut guard = self
            .ctx
            .lock()
            .map_err(|_| StageError::Failed("transcribe 模型锁中毒".into()))?;
        if guard.is_none() {
            let model = self.manager.model_dir(TRANSCRIBE_MODEL).join("model.bin");
            // Metal 实测口径：use_gpu = true（whisper.cpp 编译期 Metal
            // 后端开启时生效；不可用自动回落 CPU）。
            let cparams = whisper_rs::WhisperContextParameters {
                use_gpu: true,
                ..whisper_rs::WhisperContextParameters::default()
            };
            let ctx = whisper_rs::WhisperContext::new_with_params(&model, cparams)
                .map_err(|e| StageError::Skipped(format!("transcribe-whisper 加载失败: {e}")))?;
            *guard = Some(ctx);
        }
        Ok(guard)
    }
}

#[cfg(feature = "ai-transcribe")]
impl SidecarStage for TranscribeStage {
    fn stage(&self) -> &'static str {
        stage_ids::TRANSCRIBE
    }

    fn applicable(&self, mime: &MimeKind) -> bool {
        *mime == MimeKind::Audio
    }

    fn run(&self, input: &StageInput) -> Result<StageOutput, StageError> {
        self.manager
            .verify(TRANSCRIBE_MODEL)
            .map_err(map_verify_err)?;
        let samples = decode_audio(&input.data)?;
        let guard = self.ensure_ctx()?;
        let ctx = guard.as_ref().expect("ensure_ctx 后模型必然已初始化");
        let mut state = ctx
            .create_state()
            .map_err(|e| StageError::Failed(format!("创建转写状态失败: {e}")))?;
        let mut params =
            whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(None); // 自动检测（多语模型）
        params.set_print_progress(false);
        params.set_print_special(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        state
            .full(params, &samples)
            .map_err(|e| StageError::Failed(format!("转写推理失败: {e}")))?;
        let n = state.full_n_segments().max(0);
        let mut text = String::new();
        for i in 0..n {
            if let Some(seg) = state.get_segment(i) {
                let s = seg
                    .to_str_lossy()
                    .map_err(|e| StageError::Failed(format!("段文本读取失败: {e}")))?;
                text.push_str(&s);
            }
        }
        let secs = samples.len() as f64 / f64::from(WHISPER_SAMPLE_RATE);
        Ok(StageOutput {
            blob: Some(text.clone().into_bytes()),
            detail: Some(format!(
                "segments={n} secs={secs:.1} chars={} model=transcribe-whisper",
                text.chars().count()
            )),
            ..StageOutput::default()
        })
    }
}

/// symphonia 解码任意音频容器 → 单声道 f32 → 16 kHz（`ai-transcribe`）。
#[cfg(feature = "ai-transcribe")]
fn decode_audio(bytes: &[u8]) -> Result<Vec<f32>, StageError> {
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::errors::Error as SymError;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let mss = MediaSourceStream::new(
        Box::new(std::io::Cursor::new(bytes.to_vec())),
        symphonia::core::io::MediaSourceStreamOptions::default(),
    );
    let probed = symphonia::default::get_probe().format(
        &Hint::new(),
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    );
    let format = match probed {
        Ok(p) => p.format,
        Err(SymError::Unsupported(_)) => {
            return Err(StageError::Skipped(
                "音频容器不支持（无匹配解封装器）".into(),
            ));
        }
        Err(e) => return Err(StageError::Failed(format!("音频容器识别失败: {e}"))),
    };
    let mut format = format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or_else(|| StageError::Failed("音频轨道缺失".into()))?;
    let track_id = track.id;
    let sample_rate = track
        .codec_params
        .sample_rate
        .unwrap_or(WHISPER_SAMPLE_RATE);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| StageError::Failed(format!("音频解码器创建失败: {e}")))?;

    let mut interleaved: Vec<f32> = Vec::new();
    let mut channels: usize = 1;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(StageError::Failed(format!("音频读帧失败: {e}"))),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf) => {
                let spec = *buf.spec();
                channels = channels.max(usize::from(spec.channels.count()));
                let mut sbuf =
                    symphonia::core::audio::SampleBuffer::<f32>::new(buf.capacity() as u64, spec);
                sbuf.copy_interleaved_ref(buf);
                interleaved.extend_from_slice(sbuf.samples());
            }
            Err(SymError::DecodeError(_)) => continue, // 坏帧跳过
            Err(e) => return Err(StageError::Failed(format!("音频解码失败: {e}"))),
        }
    }
    if interleaved.is_empty() {
        return Err(StageError::Skipped("音频解码为空（无有效帧）".into()));
    }
    // 混音到单声道
    let mono: Vec<f32> = if channels > 1 {
        interleaved
            .chunks(channels)
            .map(|ch| ch.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        interleaved
    };
    Ok(resample_to_16k(&mono, sample_rate))
}

/// 默认转写 stage：feature 开 → 真实 whisper；关 → CI fake（同一降级语义）。
#[must_use]
pub fn default_transcribe_stage() -> Arc<dyn SidecarStage> {
    let manager = ModelManager::from_env();
    #[cfg(feature = "ai-transcribe")]
    {
        Arc::new(TranscribeStage::new(manager))
    }
    #[cfg(not(feature = "ai-transcribe"))]
    {
        Arc::new(FakeTranscribeStage::new(manager))
    }
}
