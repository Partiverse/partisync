//! stage2 OCR：ort + PaddleOCR mobile det(DBNet) + rec(CRNN)
//! （ADR-0017，SPEC M4-WP01 裁定 3；RapidOCR ONNX 口径）。
//!
//! - 纯函数后处理（[`det_resize_dims`] / `chw_normalize` / [`find_boxes`]
//!   / [`ctc_decode`]）恒编译，CI 用合成张量钉行为契约；
//! - 真实路径 [`OcrStage`]（`ai-ocr` feature，默认关）：det 概率图 →
//!   连通域取轴对齐框（简化不做旋转框/多边形 unclip——冒烟校正项）→
//!   rec 逐框识别 → CTC 解码；词典 = rec 模型目录 `dict.txt`
//!   （运行时加载，不内嵌——词典随模型分发，避免仓库固化上游词表）；
//! - CI 路径 [`FakeOcrStage`]（恒编译）：同一 ModelManager 降级语义 +
//!   确定性伪识别文本。
//! - **推理口径未冒烟校正**：预处理张量布局/后处理阈值按 PaddleOCR
//!   公开口径实现，真模型实测（HF 恢复可达后）复核——登记 T05 报告。

use crate::models::{ModelManager, OCR_MODEL_DET, OCR_MODEL_REC};
use crate::pipeline::{MimeKind, SidecarStage, StageError, StageInput, StageOutput};
use crate::sidecar::stage_ids;

/// det 输入最大边（PP-OCR mobile 口径），对齐 32 倍数。
pub const DET_MAX_SIDE: u32 = 960;
const DET_MULTIPLE: u32 = 32;
/// DB 概率图二值化阈值。
pub const DET_THRESH: f32 = 0.3;
/// 连通域最小边（像素，滤噪点）。
pub const DET_MIN_BOX: u32 = 3;
/// rec 输入高（PP-OCR CRNN 口径）。
pub const REC_TARGET_H: u32 = 48;
/// rec 输入最大宽。
pub const REC_MAX_W: u32 = 320;
/// CTC blank 索引（PP-OCR 约定：0=blank，1..=dict 词典，dict+1=空格）。
const CTC_BLANK: usize = 0;

/// 轴对齐文本框（像素，含于图像内，x1/y1 为排他上界）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextBox {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl TextBox {
    #[must_use]
    pub fn width(&self) -> u32 {
        self.x1 - self.x0
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.y1 - self.y0
    }
}

/// det 输入尺寸：最长边 ≤ [`DET_MAX_SIDE`] 且 H/W 均 32 对齐。
#[must_use]
pub fn det_resize_dims(w: u32, h: u32) -> (u32, u32) {
    let scale = if w.max(h) > DET_MAX_SIDE {
        f64::from(DET_MAX_SIDE) / f64::from(w.max(h))
    } else {
        1.0
    };
    let align = |v: f64| {
        let m = (v / f64::from(DET_MULTIPLE)).round().max(1.0) as u32;
        m * DET_MULTIPLE
    };
    (align(f64::from(w) * scale), align(f64::from(h) * scale))
}

/// RGB8 → CHW f32，ImageNet mean/std 归一（PaddleOCR 口径）。
#[must_use]
pub fn chw_normalize(rgb: &[u8], w: u32, h: u32) -> Vec<f32> {
    const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
    const STD: [f32; 3] = [0.229, 0.224, 0.225];
    let n = (w * h) as usize;
    let mut out = vec![0f32; n * 3];
    for c in 0..3 {
        for i in 0..n {
            out[c * n + i] = (f32::from(rgb[i * 3 + c]) / 255.0 - MEAN[c]) / STD[c];
        }
    }
    out
}

/// DB 概率图二值化 → 连通域（4 邻接 BFS）→ 轴对齐框（滤 < [`DET_MIN_BOX`]）。
#[must_use]
pub fn find_boxes(prob: &[f32], w: u32, h: u32) -> Vec<TextBox> {
    let mask: Vec<bool> = prob.iter().map(|&p| p > DET_THRESH).collect();
    let mut seen = vec![false; mask.len()];
    let mut boxes = Vec::new();
    let wi = w as usize;
    let hi = h as usize;
    for y in 0..hi {
        for x in 0..wi {
            let start = y * wi + x;
            if !mask[start] || seen[start] {
                continue;
            }
            let (mut min_x, mut max_x) = (x, x);
            let (mut min_y, mut max_y) = (y, y);
            let mut stack = vec![start];
            seen[start] = true;
            while let Some(idx) = stack.pop() {
                let cx = idx % wi;
                let cy = idx / wi;
                (min_x, max_x) = (min_x.min(cx), max_x.max(cx));
                (min_y, max_y) = (min_y.min(cy), max_y.max(cy));
                for (nx, ny) in [
                    (cx.wrapping_sub(1), cy),
                    (cx + 1, cy),
                    (cx, cy.wrapping_sub(1)),
                    (cx, cy + 1),
                ] {
                    if nx < wi && ny < hi {
                        let n = ny * wi + nx;
                        if mask[n] && !seen[n] {
                            seen[n] = true;
                            stack.push(n);
                        }
                    }
                }
            }
            let (bw, bh) = ((max_x - min_x + 1) as u32, (max_y - min_y + 1) as u32);
            if bw >= DET_MIN_BOX && bh >= DET_MIN_BOX {
                boxes.push(TextBox {
                    x0: min_x as u32,
                    y0: min_y as u32,
                    x1: max_x as u32 + 1,
                    y1: max_y as u32 + 1,
                });
            }
        }
    }
    boxes.sort_by_key(|b| (b.y0, b.x0));
    boxes
}

/// CRNN CTC 解码：逐帧 argmax → 去重连续同帧 → 跳 blank → 词典映射
/// （idx ∈ 1..=dict → dict[idx-1]；idx == dict+1 → 空格）。
#[must_use]
pub fn ctc_decode(logits: &[f32], frames: usize, classes: usize, dict: &[char]) -> String {
    let mut out = String::new();
    let mut prev = CTC_BLANK;
    for t in 0..frames {
        let row = &logits[t * classes..(t + 1) * classes];
        let (best, _p) = row
            .iter()
            .enumerate()
            .fold((CTC_BLANK, f32::MIN), |(bi, bp), (i, &p)| {
                if p > bp {
                    (i, p)
                } else {
                    (bi, bp)
                }
            });
        if best != prev && best != CTC_BLANK {
            if best == dict.len() + 1 {
                out.push(' ');
            } else if let Some(&ch) = dict.get(best - 1) {
                out.push(ch);
            }
        }
        prev = best;
    }
    out
}

/// rec 输入宽：等比缩放到高 [`REC_TARGET_H`]，宽 clamp ≤ [`REC_MAX_W`]。
#[must_use]
pub fn rec_target_w(box_w: u32, box_h: u32) -> u32 {
    let ratio_w = (f64::from(box_w) * f64::from(REC_TARGET_H) / f64::from(box_h.max(1))).ceil();
    (ratio_w as u32).clamp(1, REC_MAX_W)
}

/// CI/离线 OCR stage：ModelManager 判定可用性（det+rec 均需在场），
/// 缺失 → skipped；确定性伪识别文本。
#[derive(Debug, Clone)]
pub struct FakeOcrStage {
    manager: ModelManager,
}

impl FakeOcrStage {
    #[must_use]
    pub fn new(manager: ModelManager) -> Self {
        Self { manager }
    }
}

impl SidecarStage for FakeOcrStage {
    fn stage(&self) -> &'static str {
        stage_ids::OCR
    }

    fn applicable(&self, mime: &MimeKind) -> bool {
        *mime == MimeKind::Image
    }

    fn run(&self, input: &StageInput) -> Result<StageOutput, StageError> {
        self.manager.verify(OCR_MODEL_DET).map_err(map_verify_err)?;
        self.manager.verify(OCR_MODEL_REC).map_err(map_verify_err)?;
        let text = fake_text(&input.content_id);
        Ok(StageOutput {
            blob: Some(text.clone().into_bytes()),
            detail: Some(format!(
                "boxes=1 chars={} model=pp-ocr (fake)",
                text.chars().count()
            )),
            ..StageOutput::default()
        })
    }
}

fn fake_text(content_id: &str) -> String {
    let seed = blake3::hash(content_id.as_bytes());
    format!("ocr-{}", &seed.to_hex().to_string()[..8])
}

fn map_verify_err(e: crate::models::ModelError) -> StageError {
    match e {
        crate::models::ModelError::Unknown(_) => StageError::Failed(e.to_string()),
        _ => StageError::Skipped(e.to_string()),
    }
}

/// 真实 OCR stage（ort；`ai-ocr` feature 编译）。
#[cfg(feature = "ai-ocr")]
pub struct OcrStage {
    manager: ModelManager,
    det: std::sync::Mutex<Option<ort::session::Session>>,
    rec: std::sync::Mutex<Option<ort::session::Session>>,
}

#[cfg(feature = "ai-ocr")]
impl OcrStage {
    #[must_use]
    pub fn new(manager: ModelManager) -> Self {
        Self {
            manager,
            det: std::sync::Mutex::new(None),
            rec: std::sync::Mutex::new(None),
        }
    }

    fn load_session(&self, dir: &str) -> Result<ort::session::Session, StageError> {
        let path = self.manager.model_dir(dir).join("model.bin");
        let builder = ort::session::Session::builder()
            .map_err(|e| StageError::Skipped(format!("{dir} ort 环境失败: {e}")))?;
        let builder = builder
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| StageError::Skipped(format!("{dir} 优化级别失败: {e}")))?;
        let mut builder = builder;
        builder
            .commit_from_file(&path)
            .map_err(|e| StageError::Skipped(format!("{dir} 加载失败: {e}")))
    }

    fn ensure_sessions(
        &self,
    ) -> Result<
        (
            std::sync::MutexGuard<'_, Option<ort::session::Session>>,
            std::sync::MutexGuard<'_, Option<ort::session::Session>>,
        ),
        StageError,
    > {
        let mut det = self
            .det
            .lock()
            .map_err(|_| StageError::Failed("ocr det 模型锁中毒".into()))?;
        let mut rec = self
            .rec
            .lock()
            .map_err(|_| StageError::Failed("ocr rec 模型锁中毒".into()))?;
        if det.is_none() {
            *det = Some(self.load_session(OCR_MODEL_DET)?);
        }
        if rec.is_none() {
            *rec = Some(self.load_session(OCR_MODEL_REC)?);
        }
        Ok((det, rec))
    }

    fn dict(&self) -> Result<Vec<char>, StageError> {
        let path = self.manager.model_dir(OCR_MODEL_REC).join("dict.txt");
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| StageError::Failed(format!("词典缺失 {}: {e}", path.display())))?;
        Ok(raw
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|l| l.chars().next())
            .collect())
    }
}

#[cfg(feature = "ai-ocr")]
impl SidecarStage for OcrStage {
    fn stage(&self) -> &'static str {
        stage_ids::OCR
    }

    fn applicable(&self, mime: &MimeKind) -> bool {
        *mime == MimeKind::Image
    }

    fn run(&self, input: &StageInput) -> Result<StageOutput, StageError> {
        use std::borrow::Cow;

        self.manager.verify(OCR_MODEL_DET).map_err(map_verify_err)?;
        self.manager.verify(OCR_MODEL_REC).map_err(map_verify_err)?;
        let img = image::load_from_memory(&input.data)
            .map_err(|e| StageError::Failed(format!("图片解码失败: {e}")))?;
        let rgb = img.to_rgb8();
        let (mut det_guard, mut rec_guard) = self.ensure_sessions()?;

        // ── det：DBNet 概率图 → 连通域框 ──
        let (dw, dh) = det_resize_dims(rgb.width(), rgb.height());
        let scaled = image::imageops::resize(&rgb, dw, dh, image::imageops::FilterType::Triangle);
        let det_in = chw_normalize(scaled.as_raw(), dw, dh);
        let det_session = det_guard.as_mut().expect("ensure_sessions 后必然已初始化");
        let det_tensor =
            ort::value::Tensor::from_array((vec![1i64, 3, dh as i64, dw as i64], det_in))
                .map_err(|e| StageError::Failed(format!("det 张量构造失败: {e}")))?;
        let det_out = det_session
            .run(vec![(Cow::from("x"), det_tensor)])
            .map_err(|e| StageError::Failed(format!("det 推理失败: {e}")))?;
        let det_name = det_out
            .keys()
            .next()
            .ok_or_else(|| StageError::Failed("det 无输出".into()))?
            .to_owned();
        let (shape, prob) = det_out[det_name.as_str()]
            .try_extract_tensor::<f32>()
            .map_err(|e| StageError::Failed(format!("det 输出提取失败: {e}")))?;
        // [1,1,H,W] → 平面（末两维尺寸；展平数据即 H*W 概率）
        let (ph, pw) = match shape.len() {
            4 => (shape[2] as u32, shape[3] as u32),
            _ => (dh, dw),
        };
        let want = (ph as usize) * (pw as usize);
        let prob: &[f32] = if prob.len() >= want {
            &prob[prob.len() - want..]
        } else {
            prob
        };
        let boxes_scaled = find_boxes(prob, pw, ph);
        // 缩放回原图坐标 + 2px 外扩
        let sx = f64::from(rgb.width()) / f64::from(pw);
        let sy = f64::from(rgb.height()) / f64::from(ph);
        let boxes: Vec<TextBox> = boxes_scaled
            .iter()
            .map(|b| TextBox {
                x0: (f64::from(b.x0) * sx).floor().max(0.0) as u32,
                y0: (f64::from(b.y0) * sy).floor().max(0.0) as u32,
                x1: ((f64::from(b.x1) * sx).ceil() as u32).min(rgb.width()),
                y1: ((f64::from(b.y1) * sy).ceil() as u32).min(rgb.height()),
            })
            .filter(|b| b.width() >= DET_MIN_BOX && b.height() >= DET_MIN_BOX)
            .collect();

        // ── rec：逐框 CRNN + CTC ──
        let dict = self.dict()?;
        let rec_session = rec_guard.as_mut().expect("ensure_sessions 后必然已初始化");
        let mut text = String::new();
        for b in &boxes {
            let tw = rec_target_w(b.width(), b.height());
            let crop = image::imageops::crop_imm(&rgb, b.x0, b.y0, b.width(), b.height());
            let scaled = image::imageops::resize(
                &crop.to_image(),
                tw,
                REC_TARGET_H,
                image::imageops::FilterType::Triangle,
            );
            let rec_in = chw_normalize(scaled.as_raw(), tw, REC_TARGET_H);
            let rec_tensor = ort::value::Tensor::from_array((
                vec![1i64, 3, i64::from(REC_TARGET_H), i64::from(tw)],
                rec_in,
            ))
            .map_err(|e| StageError::Failed(format!("rec 张量构造失败: {e}")))?;
            let rec_out = rec_session
                .run(vec![(Cow::from("x"), rec_tensor)])
                .map_err(|e| StageError::Failed(format!("rec 推理失败: {e}")))?;
            let rec_name = rec_out
                .keys()
                .next()
                .ok_or_else(|| StageError::Failed("rec 无输出".into()))?
                .to_owned();
            let (rshape, logits) = rec_out[rec_name.as_str()]
                .try_extract_tensor::<f32>()
                .map_err(|e| StageError::Failed(format!("rec 输出提取失败: {e}")))?;
            let frames = *rshape.iter().rev().nth(1).unwrap_or(&0) as usize;
            let classes = *rshape.last().unwrap_or(&0) as usize;
            if frames > 0 && classes > 0 {
                text.push_str(&ctc_decode(logits, frames, classes, &dict));
                text.push('\n');
            }
        }
        Ok(StageOutput {
            blob: Some(text.clone().into_bytes()),
            detail: Some(format!(
                "boxes={} chars={} model=pp-ocr-mobile",
                boxes.len(),
                text.chars().count()
            )),
            ..StageOutput::default()
        })
    }
}

/// 默认 OCR stage：feature 开 → 真实 ort；关 → CI fake（同一降级语义）。
#[must_use]
pub fn default_ocr_stage() -> std::sync::Arc<dyn SidecarStage> {
    let manager = ModelManager::from_env();
    #[cfg(feature = "ai-ocr")]
    {
        std::sync::Arc::new(OcrStage::new(manager))
    }
    #[cfg(not(feature = "ai-ocr"))]
    {
        std::sync::Arc::new(FakeOcrStage::new(manager))
    }
}
