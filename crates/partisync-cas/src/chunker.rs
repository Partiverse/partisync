//! 内容定义分块（SPEC M0-WP03 契约 §1；fastcdc v2020，ADR-0004）。
//!
//! P1–P3 不变量（properties.md）由 tests/chunker.rs 钉住。

use blake3::Hasher;

/// 生产参数：256 KiB / 1 MiB / 4 MiB（restic/borg/kopia 经验区间，调研方案 §5.9）。
pub const PRODUCTION_MIN: usize = 256 * 1024;
pub const PRODUCTION_AVG: usize = 1024 * 1024;
pub const PRODUCTION_MAX: usize = 4 * 1024 * 1024;

/// 分块配置。fastcdc v2020 要求 min/avg/max 均为 8 的倍数：
/// [`CdcConfig::new`] 将入参向下归一到 8 的倍数，故取值总是合法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CdcConfig {
    pub min: usize,
    pub avg: usize,
    pub max: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdcConfigError {
    /// min ≥ avg 或 avg ≥ max（归一化后仍不满足）。
    NotIncreasing,
}

impl CdcConfig {
    pub const PRODUCTION: CdcConfig = CdcConfig {
        min: PRODUCTION_MIN,
        avg: PRODUCTION_AVG,
        max: PRODUCTION_MAX,
    };

    /// 归一化构造：向下取 8 的倍数；min<avg<max 校验在归一化之后进行。
    ///
    /// # Errors
    /// 归一化后仍不满足 min < avg < max → [`CdcConfigError::NotIncreasing`]。
    pub fn new(min: usize, avg: usize, max: usize) -> Result<Self, CdcConfigError> {
        let round = |v: usize| v & !7;
        let (min, avg, max) = (round(min), round(avg), round(max));
        if min == 0 || min >= avg || avg >= max {
            return Err(CdcConfigError::NotIncreasing);
        }
        Ok(CdcConfig { min, avg, max })
    }
}

/// 计算分块边界序列（确定性：同输入恒同输出，P1）。
#[must_use]
pub fn chunk_boundaries(data: &[u8], cfg: CdcConfig) -> Vec<(usize, usize)> {
    fastcdc::v2020::FastCDC::new(data, cfg.min, cfg.avg, cfg.max)
        .map(|c| (c.offset, c.length))
        .collect()
}

/// 块清单根：blake3(拼接的块哈希 hex)。即 P4 的 root(chunks) 口径
/// （v1 为线性清单摘要，分级 Merkle 归 M2 delta 协议，SPEC 风险节）。
#[must_use]
pub fn chunk_root(chunk_hashes: &[&str]) -> String {
    let mut h = Hasher::new();
    for hex in chunk_hashes {
        h.update(hex.as_bytes());
    }
    h.finalize().to_hex().to_string()
}
