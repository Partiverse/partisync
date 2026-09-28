//! 桌面壳错误码（SPEC M6-WP03 §2.6）。
//!
//! IPC 错误返回统一形状 `{kind: string, msg: string}`： 前端可按
//! `kind` 分支处理（toast / 降级 / 重试）， `msg` 仅供调试 / 上报。
//!
//! ## T03 / T05 边界
//! - T03 实际抛出 `Internal` / `Index` 两类（命令 panic / 索引懒加载失败）
//! - `WebviewMissing` / `DataDirUnwritable` 由 T01 / T08 启动期检查产生
//! - `Sidecar` 属 T05 范畴， 本期不抛

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// 桌面壳错误枚举（IPC 错误返回类型）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopError {
    /// WebView 运行时缺失（Linux 缺 webkit2gtk-4.1）。
    WebviewMissing(String),
    /// 数据目录不可写。
    DataDirUnwritable(String),
    /// 索引不可用（首次 search IPC 时 IndexEngine 懒加载失败）。
    Index(String),
    /// Sidecar 启动失败（T05 范畴）。
    Sidecar(String),
    /// 内部错误（IPC command panic / 数据库错 / IO 错）。
    Internal(String),
}

impl DesktopError {
    /// 序列化为 IPC 错误对象 `{kind: string, msg: string}`。
    fn serialize_parts(&self) -> (&'static str, String) {
        match self {
            Self::WebviewMissing(m) => ("WebviewMissing", m.clone()),
            Self::DataDirUnwritable(m) => ("DataDirUnwritable", m.clone()),
            Self::Index(m) => ("Index", m.clone()),
            Self::Sidecar(m) => ("Sidecar", m.clone()),
            Self::Internal(m) => ("Internal", m.clone()),
        }
    }
}

impl Serialize for DesktopError {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let (kind, msg) = self.serialize_parts();
        let mut s = ser.serialize_struct("DesktopError", 2)?;
        s.serialize_field("kind", kind)?;
        s.serialize_field("msg", &msg)?;
        s.end()
    }
}

impl std::fmt::Display for DesktopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, msg) = self.serialize_parts();
        write!(f, "{kind}: {msg}")
    }
}

impl std::error::Error for DesktopError {}

impl From<partisync_core::error::PartisyError> for DesktopError {
    fn from(e: partisync_core::error::PartisyError) -> Self {
        Self::Internal(e.to_string())
    }
}

/// IPC command 统一返回类型。
pub type DesktopResult<T> = Result<T, DesktopError>;
