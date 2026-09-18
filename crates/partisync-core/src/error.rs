//! 错误分类学（SPEC M0-WP01）：作业恢复语义的三分类。
//!
//! `Severity` 是作业系统（M0-WP05）决定「重试 / 放弃 / 挂起等恢复」的唯一依据；
//! `Interrupted` 只能由应用层显式构造（用户暂停、关机信号），绝不从 io kind 推断。

use std::fmt;

/// 错误严重度三分类（执行方案 P8/作业规格）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    /// 瞬态故障：重试可能成功（超时、连接重置、目标暂时消失）。
    Retryable,
    /// 持久故障：重试无意义（权限、数据格式、存储满）。
    Fatal,
    /// 应用层显式中断（用户暂停/关机信号）：作业挂起并保留检查点。
    Interrupted,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Retryable => "retryable",
            Severity::Fatal => "fatal",
            Severity::Interrupted => "interrupted",
        })
    }
}

/// io::ErrorKind → Severity 表驱动映射（M0 初版；扩充须改 SPEC）。
#[must_use]
pub fn classify_io(kind: std::io::ErrorKind) -> Severity {
    use std::io::ErrorKind as K;
    match kind {
        // 瞬态：网络抖动与扫描期良性竞争（NotFound = 目标在扫描间隙移动/删除，重扫即可）
        K::TimedOut
        | K::ConnectionRefused
        | K::ConnectionReset
        | K::ConnectionAborted
        | K::NetworkUnreachable
        | K::Interrupted
        | K::WouldBlock
        | K::NotFound => Severity::Retryable,
        // 持久：重试无意义
        K::PermissionDenied
        | K::InvalidData
        | K::InvalidInput
        | K::Unsupported
        | K::StorageFull
        | K::WriteZero
        | K::OutOfMemory => Severity::Fatal,
        // 保守默认：未知类型按持久处理，避免无限重试循环
        _ => Severity::Fatal,
    }
}

/// 带严重度的顶层错误（severity 是机器可判定的，source 是给人看的）。
#[derive(Debug)]
pub struct PartisyError {
    pub severity: Severity,
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl PartisyError {
    #[must_use]
    pub fn new(severity: Severity) -> Self {
        PartisyError {
            severity,
            source: None,
        }
    }

    #[must_use]
    pub fn with_source(
        severity: Severity,
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        PartisyError {
            severity,
            source: Some(source),
        }
    }
}

impl fmt::Display for PartisyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.severity)?;
        if let Some(s) = &self.source {
            write!(f, ": {s}")?;
        }
        Ok(())
    }
}

impl std::error::Error for PartisyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|b| &**b as _)
    }
}
