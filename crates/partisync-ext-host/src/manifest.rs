//! 扩展 capability manifest（Wassette 式，SPEC M7-WP01 §2.2）。
//!
//! 冻结规则（与 SPEC §2.2 一致）：
//! - `<name>.wasm` 同目录配同名 `<name>.json`
//! - 声明集 ⊆ 宿主白名单；FS / 网络 / 环境不在白名单（编译期拒绝）
//! - 工具名与内建 MCP 撞名 → 加载期拒绝（内建优先，§2.3）
//!
//! 本模块只定义格式 + 校验；**加载期拒绝先于任何 host function 暴露**
//! ——这是 [P14] 的核心不变量。host function 注入（`host::linker_for`）
//! 由 PR-B 提供。

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// SPEC §2.2 冻结的 capability 白名单（第一版，批准即冻结）。
///
/// 序列化形态：`{"index.read" | "clock.read"}`（与 SPEC §2.2 表对齐）。
/// serde 字符串枚举直接走字面映射——`rename_all` 必要但点号不在
/// serde 关键字集合里，需逐 variant `rename` 显式给出。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Capability {
    /// 索引只读查询（复用既有 MCP 工具语义，经 `IndexRead` trait）。
    #[serde(rename = "index.read")]
    IndexRead,
    /// 当前时间戳（`now_millis()`）。
    #[serde(rename = "clock.read")]
    ClockRead,
}

impl Capability {
    /// 声明→字符串：稳定序列化形式（manifest 文件同名差异由此消歧）。
    pub fn as_str(self) -> &'static str {
        match self {
            Capability::IndexRead => "index.read",
            Capability::ClockRead => "clock.read",
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 磁盘上的 manifest 文件形态。`tool_name` 与内建 MCP 工具的撞名
/// 由校验器检测（拒绝语义见 [`ValidateError`]）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// 扩展暴露的工具名（`partisync-mcp` 工具面前缀 `ext_`，撞名
    /// 检测在白名单侧做；manifest 字段记录原始名）。
    pub tool_name: String,
    /// 声明的 capability 集。**声明面 ∩ 宿主白名单 = 实际注入面**；
    /// 超出白名单的项在校验期拒绝（[P14] 加载即拒）。
    #[serde(default)]
    pub capabilities: Vec<Capability>,
}

/// 校验错误。**所有变体均为加载期拒绝**——实例化前调用 `validate`
/// 必须返回 Ok 才能进入 host function 暴露阶段。
#[derive(Debug, PartialEq, Eq)]
pub enum ValidateError {
    BadToolName,
    UnknownCapability(Capability),
    ToolNameCollision(&'static str),
}

impl fmt::Display for ValidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadToolName => {
                f.write_str("manifest 工具名非法：必须非空、长度 ≤128、ASCII 字母数字下划线连字符")
            }
            Self::UnknownCapability(c) => write!(
                f,
                "manifest 声明未知 capability：{c:?}（宿主白名单仅 index.read / clock.read）"
            ),
            Self::ToolNameCollision(n) => {
                write!(f, "manifest 工具名 `{n}` 与内建 MCP 工具撞名（内建优先）")
            }
        }
    }
}

impl std::error::Error for ValidateError {}

/// `tool_name` 形态约束：非空、长度 ≤128、字符 ∈ [a-zA-Z0-9_-]。
fn check_tool_name(name: &str) -> Result<(), ValidateError> {
    if name.is_empty() || name.len() > 128 {
        return Err(ValidateError::BadToolName);
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(ValidateError::BadToolName);
    }
    Ok(())
}

/// 内建 MCP 工具名白名单（撞名拒绝源）。本第一版登记的五个内建
/// 工具沿 M4-WP03 收敛判例；新增内建工具时同步追加本表 + 更新测试
/// 期望覆盖。
///
/// 撞名语义：扩展工具名**等于**任一内建工具名 → 加载拒。`ext_` 前缀
/// 列举语义在 PR-B（partisync-mcp 接线）落地，故此处只做严格相等。
const BUILTIN_TOOLS: &[&str] = &[
    "asset_read",     // M4-WP03
    "asset_organize", // M4-WP03
    "dataset_export", // M4-WP03
    "search",         // M4-WP02
    "cas_stats",      // M6-WP03（沿 PARTISYNC_MCP_BIN 暴露）
];

impl Manifest {
    /// 加载期校验：**先于任何 host function 暴露**（[P14]）。
    pub fn validate(&self) -> Result<(), ValidateError> {
        check_tool_name(&self.tool_name)?;
        // 重复声明去重（保序）：manifest 容忍相同 capability 多写，
        // validate 后视为同一项——不视为错误，仅语义等价。
        let _seen: BTreeSet<&Capability> = self.capabilities.iter().collect();
        let _ = _seen; // 抑制 unused：保留为去重证据供 PR-B 注入面取交集参考
                       // 注：本第一版 Capability 枚举即白名单（FS / 网络 / 环境不在
                       // 枚举内）；未知 capability 已被 serde 反序列化期拒在门外。
                       // 若 §2.2 演化（白名单表与 Capability 分离），此处加白名单
                       // 二次过滤；本 PR 不预占。
        for builtin in BUILTIN_TOOLS {
            if self.tool_name == *builtin {
                return Err(ValidateError::ToolNameCollision(builtin));
            }
        }
        Ok(())
    }

    /// 加载 manifest JSON 文件。校验失败 = `Err`，调用方不得实例化。
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, ManifestError> {
        let bytes = std::fs::read(&path).map_err(ManifestError::Io)?;
        let m: Self =
            serde_json::from_slice(&bytes).map_err(|e| ManifestError::Parse(e.to_string()))?;
        m.validate().map_err(ManifestError::Validate)?;
        Ok(m)
    }
}

/// manifest 加载期失败。**所有变体均导致扩展被拒绝**（不进 linker）。
#[derive(Debug)]
pub enum ManifestError {
    Io(std::io::Error),
    Parse(String),
    Validate(ValidateError),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "manifest IO 失败：{e}"),
            Self::Parse(e) => write!(f, "manifest JSON 解析失败：{e}"),
            Self::Validate(e) => write!(f, "manifest 校验失败：{e}"),
        }
    }
}

impl std::error::Error for ManifestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Validate(e) => Some(e),
            Self::Parse(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cap(c: Capability) -> Vec<Capability> {
        vec![c]
    }

    #[test]
    fn validate_accepts_minimal_manifest() {
        let m = Manifest {
            tool_name: "demo".into(),
            capabilities: cap(Capability::ClockRead),
        };
        assert_eq!(m.validate(), Ok(()));
    }

    #[test]
    fn validate_rejects_empty_tool_name() {
        let m = Manifest {
            tool_name: "".into(),
            capabilities: vec![],
        };
        assert_eq!(m.validate(), Err(ValidateError::BadToolName));
    }

    #[test]
    fn validate_rejects_oversize_tool_name() {
        let m = Manifest {
            tool_name: "a".repeat(129),
            capabilities: vec![],
        };
        assert_eq!(m.validate(), Err(ValidateError::BadToolName));
    }

    #[test]
    fn validate_rejects_non_ascii_tool_name() {
        let m = Manifest {
            tool_name: "bad name".into(),
            capabilities: vec![],
        };
        assert_eq!(m.validate(), Err(ValidateError::BadToolName));
    }

    #[test]
    fn validate_rejects_builtin_collision() {
        for builtin in BUILTIN_TOOLS {
            let m = Manifest {
                tool_name: (*builtin).into(),
                capabilities: cap(Capability::ClockRead),
            };
            assert_eq!(
                m.validate(),
                Err(ValidateError::ToolNameCollision(builtin)),
                "内建工具 {builtin} 应触发撞名拒绝"
            );
        }
    }

    #[test]
    fn deserialize_rejects_unknown_capability() {
        let bytes = br#"{"tool_name":"demo","capabilities":["fs.read"]}"#;
        let err = serde_json::from_slice::<Manifest>(bytes)
            .expect_err("fs.read 不在白名单，serde 必须拒绝");
        assert!(err.to_string().contains("fs.read"));
    }

    #[test]
    fn deserialize_accepts_repeated_capability() {
        let bytes = br#"{"tool_name":"demo","capabilities":["clock.read","clock.read"]}"#;
        let m: Manifest = serde_json::from_slice(bytes).unwrap();
        assert_eq!(m.capabilities.len(), 2); // 反序列化保序
        assert_eq!(m.validate(), Ok(())); // 重复不致命
    }
}
