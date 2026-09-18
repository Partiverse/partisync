//! 内容仓库（CAS）——调研方案 §5.9。
//!
//! 当前内容：内容寻址主哈希（BLAKE3）、[`chunker`]（内容定义分块，P1–P3）、
//! [`store`]（内容寻址块库 + 引用计数，P4）。pack/EC 归 M3。

pub mod chunker;
pub mod store;

pub use chunker::{chunk_boundaries, chunk_root, CdcConfig, CdcConfigError};
pub use store::{put_chunks, CasStats, ChunkStore};

use std::fs::File;
use std::io::Read;
use std::path::Path;

use partisync_core::error::{classify_io, PartisyError, Severity};

/// 字节流的内容身份（blake3 hex，64 字符小写）。
#[must_use]
pub fn content_hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// 流式计算文件内容哈希（阻塞操作经 spawn_blocking 隔离）。
///
/// # Errors
/// 文件不可读（权限/消失）→ 按表驱动分类（见 [`classify_io`]）。
pub async fn content_hash_file(path: &Path) -> Result<String, PartisyError> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut file = File::open(&path).map_err(|e| io_err("打开文件", e))?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let n = file.read(&mut buf).map_err(|e| io_err("读取文件", e))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hasher.finalize().to_hex().to_string())
    })
    .await
    .map_err(|e| PartisyError::with_source(Severity::Fatal, Box::new(e)))?
}

fn io_err(what: &'static str, e: std::io::Error) -> PartisyError {
    let severity = classify_io(e.kind());
    let source: Box<dyn std::error::Error + Send + Sync> = format!("{what}: {e}").into();
    PartisyError {
        severity,
        source: Some(source),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic_hex64() {
        let a = content_hash(b"partisync");
        let b = content_hash(b"partisync");
        let c = content_hash(b"Partisync");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|b| b.is_ascii_hexdigit()));
    }

    #[test]
    fn empty_input_is_stable() {
        assert_eq!(content_hash(b""), blake3::hash(b"").to_hex().to_string());
    }

    #[test]
    fn content_hash_file_matches_bytes_hash() {
        // D5（变异 miss：content_hash_file 读循环无覆盖）——文件路径与字节路径必须同哈希
        let dir = std::env::temp_dir().join(format!("cas-hf-{}", partisync_core::Ulid::now()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("f.bin");
        std::fs::write(&path, b"streaming hash content").unwrap();
        let h = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(content_hash_file(&path))
            .unwrap();
        assert_eq!(h, content_hash(b"streaming hash content"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn production_consts_are_documented_values() {
        // D5（变异 miss：生产常量无断言）——参数即契约（ADR-0004）
        let c = CdcConfig::PRODUCTION;
        assert_eq!(
            (c.min, c.avg, c.max),
            (256 * 1024, 1024 * 1024, 4 * 1024 * 1024)
        );
    }
}
