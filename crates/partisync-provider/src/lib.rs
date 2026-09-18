//! Provider SPI（SPEC M1-WP01）：统一存储消费面——OpenDAL 适配 + caps 能力协商。
//!
//! 设计原则（调研方案 §5.5）：哈希/mtime/原子性异构性用显式能力位表达（P9），
//! 同步策略按 caps 集中降级；下游只见 [`Provider`] 面，OpenDAL 细节被隔离。
//! 路径约定：provider 内部路径 **不带前导 /**（OpenDAL 惯例），图谱层负责转换。

pub mod config;

pub use config::{ProviderConfig, ProviderScheme};

use partisync_core::caps::ProviderCaps;
use partisync_core::error::{PartisyError, Severity};

/// 远端条目（目录树投影，供索引器消费）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderEntry {
    pub path: String, // provider 内部路径（无前导 /）
    pub is_dir: bool,
    pub size: u64,
}

/// 统一存储 Provider：OpenDAL Operator 的薄封装。
#[derive(Clone)]
pub struct Provider {
    op: opendal::Operator,
    caps: ProviderCaps,
    scheme: ProviderScheme,
}

fn perr(what: &str, e: opendal::Error) -> PartisyError {
    // OpenDAL 错误分类：NotFound 系瞬态可重试，其余保守 Fatal（core::classify_io 对齐）
    let severity = if e.kind() == opendal::ErrorKind::NotFound {
        partisync_core::error::Severity::Retryable
    } else {
        Severity::Fatal
    };
    PartisyError {
        severity,
        source: Some(format!("{what}: {e}").into()),
    }
}

impl Provider {
    /// 按配置构造（fs/s3；webdav 随 M1 后半启用）。
    ///
    /// # Errors
    /// 未知 scheme 或 builder 失败 → Fatal。
    pub fn from_config(cfg: &ProviderConfig) -> Result<Self, PartisyError> {
        let op = match cfg.scheme {
            ProviderScheme::Fs => {
                let mut b = opendal::services::Fs::default();
                if let Some(root) = cfg.params.get("root").and_then(serde_json::Value::as_str) {
                    b = b.root(root);
                }
                opendal::Operator::new(b).map_err(|e| perr("构造 fs provider", e))?
            }
            ProviderScheme::S3 => {
                let b = opendal::services::S3::default()
                    .bucket(cfg.str_param("bucket").unwrap_or_default())
                    .root(cfg.str_param("root").unwrap_or("/"))
                    .endpoint(cfg.str_param("endpoint").unwrap_or(""))
                    .region(cfg.str_param("region").unwrap_or(""))
                    .access_key_id(cfg.str_param("access_key_id").unwrap_or(""))
                    .secret_access_key(cfg.str_param("secret_access_key").unwrap_or(""));
                opendal::Operator::new(b).map_err(|e| perr("构造 s3 provider", e))?
            }
            ProviderScheme::Webdav => {
                let b = opendal::services::Webdav::default()
                    .endpoint(cfg.str_param("endpoint").unwrap_or(""))
                    .root(cfg.str_param("root").unwrap_or("/"))
                    .username(cfg.str_param("username").unwrap_or(""))
                    .password(cfg.str_param("password").unwrap_or(""));
                opendal::Operator::new(b).map_err(|e| perr("构造 webdav provider", e))?
            }
        };
        Ok(Provider {
            op,
            caps: cfg.caps(),
            scheme: cfg.scheme,
        })
    }

    /// 能力位（P9：调用点不得绕过 caps 走能力路径）。
    #[must_use]
    pub fn caps(&self) -> &ProviderCaps {
        &self.caps
    }

    #[must_use]
    pub const fn scheme(&self) -> ProviderScheme {
        self.scheme
    }

    /// 列单层子项（目录前置由调用方排序）。
    ///
    /// # Errors
    /// OpenDAL 错误按分类透传。
    pub async fn list_children(&self, dir: &str) -> Result<Vec<ProviderEntry>, PartisyError> {
        // OpenDAL 目录列表约定：以 / 结尾（fs 后端 list("sub") ≠ list("sub/")）
        let dir = dir.trim_start_matches('/');
        let dir_prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        let entries = self
            .op
            .list(&dir_prefix)
            .await
            .map_err(|e| perr("list", e))?;
        Ok(entries
            .into_iter()
            .map(|e| {
                let meta = e.metadata();
                let is_dir = meta.is_dir();
                let path = e.path().trim_end_matches('/').to_string();
                ProviderEntry {
                    path,
                    is_dir,
                    size: meta.content_length(),
                }
            })
            .filter(|e| !e.path.is_empty() && e.path != dir)
            .collect())
    }

    /// 读文件全部字节（大文件流式归 M1-WP02 后续）。
    ///
    /// # Errors
    /// OpenDAL 错误按分类透传。
    pub async fn read_file(&self, path: &str) -> Result<Vec<u8>, PartisyError> {
        let buf = self.op.read(path).await.map_err(|e| perr("read", e))?;
        Ok(buf.to_vec())
    }

    /// 写文件。
    ///
    /// # Errors
    /// OpenDAL 错误按分类透传。
    pub async fn write_file(&self, path: &str, data: Vec<u8>) -> Result<(), PartisyError> {
        self.op
            .write(path, data)
            .await
            .map(|_| ())
            .map_err(|e| perr("write", e))
    }

    /// 删除。
    ///
    /// # Errors
    /// OpenDAL 错误按分类透传。
    pub async fn delete(&self, path: &str) -> Result<(), PartisyError> {
        self.op.delete(path).await.map_err(|e| perr("delete", e))
    }

    /// 存在性检查。
    ///
    /// # Errors
    /// OpenDAL 错误按分类透传。
    pub async fn exists(&self, path: &str) -> Result<bool, PartisyError> {
        self.op.exists(path).await.map_err(|e| perr("exists", e))
    }
}
