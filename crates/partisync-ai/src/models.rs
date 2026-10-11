//! 模型管理（SPEC M4-WP01 裁定 5）：清单钉版（名称/版本/blake3/来源）
//! + 本地缓存目录 + 缺失降级语义。
//!
//! 缓存布局：`<cache_dir>/<name>/`，`model.ok` 为可用性标记（冒烟安装
//! 成功后落盘），`model.bin` 为权重主文件（清单钉住 blake3 时校验它）。
//! 真实模型下载/推理只走手动冒烟（T05），CI 一律离线——缺失时 stage 标
//! skipped（原因可查），管线继续。
//!
//! `MANIFEST` 中 `blake3: None` 表示待冒烟实测后填实（诚实标注）；
//! 填实后 `verify` 对 `model.bin` 做哈希校验，防缓存被静默替换。

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use partisync_core::error::{PartisyError, Severity};

/// 嵌入文本模型逻辑名（清单键 / 缓存目录名）。
pub const EMBED_MODEL_TEXT: &str = "embed-text";
/// 记忆域语义嵌入模型逻辑名（M11-WP06-T05；清单键 / 缓存目录名）。
pub const EMBED_MODEL_TEXT_ZH: &str = "embed-text-zh";
/// 嵌入图像模型逻辑名。
pub const EMBED_MODEL_IMAGE: &str = "embed-image";
/// 转写模型逻辑名（whisper ggml，冒烟定版）。
pub const TRANSCRIBE_MODEL: &str = "transcribe-whisper";
/// OCR 检测模型逻辑名（PP-OCR mobile det，RapidOCR ONNX 口径）。
pub const OCR_MODEL_DET: &str = "ocr-det";
/// OCR 识别模型逻辑名（PP-OCR mobile rec）。
pub const OCR_MODEL_REC: &str = "ocr-rec";

/// 可用性标记文件名（位于 model_dir 内）。
pub const PRESENT_MARKER: &str = "model.ok";
/// 权重主文件名（位于 model_dir 内；blake3 钉住时校验对象）。
pub const MODEL_BIN: &str = "model.bin";

/// 模型清单条目（钉版：逻辑名 × 版本 × blake3 × 来源）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSpec {
    pub name: &'static str,
    pub version: &'static str,
    pub source: &'static str,
    /// 权重 blake3 hex；None = 待冒烟实测填实（verify 跳过哈希校验）。
    pub blake3: Option<&'static str>,
}

/// 模型文件钉版条目（M11-WP06-T05：逐文件 blake3 + 字节数——评估件
/// M11-WP05-t05 §2 候填哈希表的同源 Rust 常量形态）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFileSpec {
    /// 相对模型 repo 根的文件路径（fastembed 缓存布局）。
    pub path: &'static str,
    /// blake3 hex（64 字符，评估件 §2 逐字）。
    pub blake3: &'static str,
    /// 文件字节数（评估件 §2）。
    pub bytes: u64,
}

/// BGESmallZHV15 五文件钉版（SPEC M11-WP06 §2.3；值 = 评估件
/// M11-WP05-t05-vector-reinitiation-eval.md §2 候填哈希表**逐字**填入，
/// 2026-10-11 冒烟实录：Xenova/bge-small-zh-v1.5，snapshot
/// 75c43b069aac4d136ba6bc1122f995fedcfd2781）。真模型文件哈希重验为
/// 本地验收（模型文件不进 CI，评估件 §7）——下载流程
/// `HF_ENDPOINT=https://hf-mirror.com`，校验不符即拒绝使用。
pub const BGE_SMALL_ZH_V15_FILES: [ModelFileSpec; 5] = [
    ModelFileSpec {
        path: "onnx/model.onnx",
        blake3: "f1c661f38644b20ef51d58cfb9cf92cf21ed80ed796c8994eb78cfd4f4764529",
        bytes: 94_851_877,
    },
    ModelFileSpec {
        path: "tokenizer.json",
        blake3: "2948cdb566aa39990222fc687d6b2b214f91c602627c7f664b8748114756afad",
        bytes: 439_125,
    },
    ModelFileSpec {
        path: "config.json",
        blake3: "8455defe17a2d02c705ccd02d2703a401e9a012b172041ab1dfc89ee734b081b",
        bytes: 716,
    },
    ModelFileSpec {
        path: "tokenizer_config.json",
        blake3: "4bd4620b7434b5da448bf9a6cd8859aec19f7bdefacf6f2dd6f0eac70ccfa259",
        bytes: 367,
    },
    ModelFileSpec {
        path: "special_tokens_map.json",
        blake3: "a18633786802dd5a6c0bd6b1a99ea58d6c846b73efd862830fdb86318943fdff",
        bytes: 125,
    },
];

/// 模型清单（v3，M11-WP06-T05：+记忆域语义嵌入 BGESmallZHV15——五文件
/// blake3 全套钉版，评估件 §2 候填哈希表逐字；嵌入两模型 + 转写 + OCR
/// det/rec 的 blake3 仍 None——欠账不在 M11-WP06 强制范围，SPEC §2.3）。
pub const MANIFEST: [ModelSpec; 6] = [
    ModelSpec {
        name: EMBED_MODEL_TEXT_ZH,
        version: "BGESmallZHV15 (fastembed 7.0.1 EmbeddingModel::BGESmallZHV15; \
                  Xenova/bge-small-zh-v1.5 snapshot 75c43b069aac4d136ba6bc1122f995fedcfd2781)",
        source: "Xenova/bge-small-zh-v1.5",
        // 权重主文件 = onnx/model.onnx（94,851,877 B，verify 校验对象）；
        // 五文件全套钉版见 BGE_SMALL_ZH_V15_FILES（常量对照单测锁定）。
        blake3: Some("f1c661f38644b20ef51d58cfb9cf92cf21ed80ed796c8994eb78cfd4f4764529"),
    },
    ModelSpec {
        name: EMBED_MODEL_TEXT,
        version: "bge-m3 (fastembed 7.0.1 EmbeddingModel::BGEM3)",
        source: "BAAI/bge-m3",
        blake3: None, // TODO(冒烟): 实测后填实（M11-WP06 不强制，SPEC §2.3）
    },
    ModelSpec {
        name: EMBED_MODEL_IMAGE,
        version: "clip-vit-b32 (fastembed 7.0.1 ImageEmbeddingModel::ClipVitB32)",
        source: "Qdrant/clip_ViT_B_32",
        blake3: None, // TODO(冒烟): 实测后填实
    },
    ModelSpec {
        name: TRANSCRIBE_MODEL,
        version: "whisper ggml（base 多语候选，冒烟定版）",
        source: "ggerganov/whisper.cpp（HF）",
        blake3: None, // TODO(冒烟): 实测后填实
    },
    ModelSpec {
        name: OCR_MODEL_DET,
        version: "pp-ocrv4-mobile-det ONNX（RapidOCR 口径，冒烟定版）",
        source: "SWHL/RapidOCR（HF）",
        blake3: None, // TODO(冒烟): 实测后填实
    },
    ModelSpec {
        name: OCR_MODEL_REC,
        version: "pp-ocrv4-mobile-rec ONNX + dict.txt（冒烟定版）",
        source: "SWHL/RapidOCR（HF）",
        blake3: None, // TODO(冒烟): 实测后填实
    },
];

/// 模型管理错误（stage 层映射为 Skipped/Failed）。
#[derive(Debug)]
pub enum ModelError {
    /// 清单中无此模型（配置错误）。
    Unknown(String),
    /// 模型未安装/未冒烟（缺失降级的主路径）。
    Missing(String),
    /// blake3 校验不符（缓存被替换，按缺失处理并告警语义）。
    VerifyFailed(String),
}

impl core::fmt::Display for ModelError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ModelError::Unknown(n) => write!(f, "unknown-model: {n}"),
            ModelError::Missing(n) => write!(
                f,
                "model-missing: {n}（未冒烟安装，见 SPEC M4-WP01 裁定 5）"
            ),
            ModelError::VerifyFailed(n) => write!(f, "model-verify-failed: {n}（blake3 不符）"),
        }
    }
}

fn fatal(what: &str, msg: String) -> PartisyError {
    PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("{what}: {msg}").into()),
    }
}

/// 本地模型缓存管理器（判定可用性 / 校验钉版哈希）。
#[derive(Debug, Clone)]
pub struct ModelManager {
    cache_dir: PathBuf,
}

impl ModelManager {
    /// 显式缓存目录（测试用）。
    #[must_use]
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        ModelManager {
            cache_dir: cache_dir.into(),
        }
    }

    /// 生产入口：`PARTISYNC_MODEL_CACHE_DIR` 优先，默认
    /// `~/.cache/partisync/models`。
    #[must_use]
    pub fn from_env() -> Self {
        if let Some(dir) = std::env::var_os("PARTISYNC_MODEL_CACHE_DIR") {
            return ModelManager::new(dir);
        }
        let home = std::env::var_os("HOME").map(PathBuf::from);
        ModelManager::new(
            home.unwrap_or_else(|| PathBuf::from("."))
                .join(".cache/partisync/models"),
        )
    }

    #[must_use]
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// 模型目录：`<cache_dir>/<name>`。
    #[must_use]
    pub fn model_dir(&self, name: &str) -> PathBuf {
        self.cache_dir.join(name)
    }

    /// 清单条目。
    ///
    /// # Errors
    /// 清单无此模型 → [`ModelError::Unknown`]。
    pub fn spec(&self, name: &str) -> Result<ModelSpec, ModelError> {
        MANIFEST
            .iter()
            .find(|s| s.name == name)
            .cloned()
            .ok_or_else(|| ModelError::Unknown(name.to_owned()))
    }

    /// 可用性判定（清单口径）：标记存在 + （钉住 blake3 时）权重哈希校验。
    ///
    /// # Errors
    /// [`ModelError::Missing`] / [`ModelError::VerifyFailed`] / IO。
    pub fn verify(&self, name: &str) -> Result<(), ModelError> {
        self.verify_model(&self.spec(name)?)
    }

    /// 按显式 spec 判定（清单外 spec 供测试钉哈希校验路径）。
    ///
    /// # Errors
    /// [`ModelError::Missing`] / [`ModelError::VerifyFailed`] / IO。
    pub fn verify_model(&self, spec: &ModelSpec) -> Result<(), ModelError> {
        let dir = self.model_dir(spec.name);
        if !dir.join(PRESENT_MARKER).exists() {
            return Err(ModelError::Missing(spec.name.to_owned()));
        }
        if let Some(pin) = spec.blake3 {
            let bin = dir.join(MODEL_BIN);
            let bytes = std::fs::read(&bin).map_err(|e| {
                ModelError::VerifyFailed(format!("{}: 读 {MODEL_BIN}: {e}", spec.name))
            })?;
            let got = hex(&blake3::hash(&bytes).as_bytes()[..]);
            if got != pin {
                return Err(ModelError::VerifyFailed(format!(
                    "{}: 期望 {pin} 实得 {got}",
                    spec.name
                )));
            }
        }
        Ok(())
    }

    /// 冒烟安装收尾：写可用性标记（目录自动创建）。
    ///
    /// # Errors
    /// IO 错误 → Fatal。
    pub fn mark_present(&self, name: &str) -> Result<(), PartisyError> {
        let dir = self.model_dir(name);
        std::fs::create_dir_all(&dir).map_err(|e| fatal("创建模型目录", e.to_string()))?;
        std::fs::write(dir.join(PRESENT_MARKER), b"ok\n")
            .map_err(|e| fatal("写模型标记", e.to_string()))?;
        Ok(())
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod wp06_manifest_tests {
    use super::*;

    /// SPEC M11-WP06 §2.3 单测义务：MANIFEST 条目存在性 + 哈希格式
    /// （64 hex）+ 与评估件 §2 值逐字一致的常量对照断言。
    #[test]
    fn manifest_has_bgesmallzhv15_entry_with_pinned_blake3() {
        let spec = ModelManager::new("unused")
            .spec(EMBED_MODEL_TEXT_ZH)
            .expect("BGESmallZHV15 条目在 MANIFEST");
        assert_eq!(spec.source, "Xenova/bge-small-zh-v1.5");
        let pin = spec.blake3.expect("blake3 已填实（编译面非 None）");
        assert_eq!(pin.len(), 64, "blake3 hex 64 字符");
        assert!(pin.chars().all(|c| c.is_ascii_hexdigit()));
        // 权重主文件哈希 = 五文件表中 onnx/model.onnx 行（同源）
        let weights = BGE_SMALL_ZH_V15_FILES
            .iter()
            .find(|f| f.path == "onnx/model.onnx")
            .expect("权重行在五文件表");
        assert_eq!(pin, weights.blake3);
        assert_eq!(weights.bytes, 94_851_877);
    }

    /// 常量对照断言：五文件哈希/字节数与评估件 §2 候填哈希表**逐字**
    /// 一致（M11-WP05-t05-vector-reinitiation-eval.md §2，2026-10-11）。
    #[test]
    fn bge_files_match_eval_doc_verbatim() {
        let expected: [(&str, &str, u64); 5] = [
            (
                "onnx/model.onnx",
                "f1c661f38644b20ef51d58cfb9cf92cf21ed80ed796c8994eb78cfd4f4764529",
                94_851_877,
            ),
            (
                "tokenizer.json",
                "2948cdb566aa39990222fc687d6b2b214f91c602627c7f664b8748114756afad",
                439_125,
            ),
            (
                "config.json",
                "8455defe17a2d02c705ccd02d2703a401e9a012b172041ab1dfc89ee734b081b",
                716,
            ),
            (
                "tokenizer_config.json",
                "4bd4620b7434b5da448bf9a6cd8859aec19f7bdefacf6f2dd6f0eac70ccfa259",
                367,
            ),
            (
                "special_tokens_map.json",
                "a18633786802dd5a6c0bd6b1a99ea58d6c846b73efd862830fdb86318943fdff",
                125,
            ),
        ];
        assert_eq!(BGE_SMALL_ZH_V15_FILES.len(), expected.len());
        for (got, (path, hash, bytes)) in BGE_SMALL_ZH_V15_FILES.iter().zip(expected) {
            assert_eq!(got.path, path, "文件路径逐字");
            assert_eq!(got.blake3, hash, "{path} blake3 逐字");
            assert_eq!(got.bytes, bytes, "{path} 字节数");
            assert_eq!(got.blake3.len(), 64, "{path} 64 hex");
            assert!(got.blake3.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    /// SPEC §2.3 诚实边界：bge-m3/clip-vit-b32 blake3 None 欠账不在本 WP
    /// 强制范围——保持 None（不得顺手填）。
    #[test]
    fn legacy_embed_entries_blake3_still_none() {
        for name in [EMBED_MODEL_TEXT, EMBED_MODEL_IMAGE] {
            let spec = ModelManager::new("unused").spec(name).expect("条目在");
            assert!(spec.blake3.is_none(), "{name} 欠账维持 None（SPEC §2.3）");
        }
    }
}
