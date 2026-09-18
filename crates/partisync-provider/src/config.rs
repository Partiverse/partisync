//! Provider 配置（scheme + 参数字典 → caps）。
//!
//! caps 判定规则（SPEC M1-WP01：Default 保守，P9 前提——未声明即无能力）：
//! - s3：MPU/预签名/服务端 copy 能力 true；哈希 caps 按 S3 兼容面给 md5（常规）；
//!   mtime 精度 Millis（Last-Modified 毫秒）；事件流暂无（webhook 适配归后续）；
//! - webdav：全保守（ETag 服务端差异大——调研方案 §3.2 备案的兼容缝隙）；
//! - fs：本地全能力，mtime Nanos。

use partisync_core::caps::{HashCaps, MtimePrecision, ProviderCaps};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderScheme {
    Fs,
    S3,
    Webdav,
}

/// Provider 连接配置（params 以 JSON 字典承载，加密落库归后续——明文仅内存态）。
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    pub scheme: ProviderScheme,
    #[serde(default)]
    pub params: serde_json::Map<String, serde_json::Value>,
}

impl ProviderConfig {
    #[must_use]
    pub fn str_param(&self, key: &str) -> Option<&str> {
        self.params.get(key).and_then(serde_json::Value::as_str)
    }

    /// 能力位判定（P9 的静态来源；动态探测归 M1-WP08 互操作矩阵）。
    #[must_use]
    pub fn caps(&self) -> ProviderCaps {
        match self.scheme {
            ProviderScheme::Fs => ProviderCaps::new(
                HashCaps {
                    md5: false,
                    sha256: false,
                    crc64nvme: false,
                    blake3: true, // 本地文件可自算 BLAKE3（PartiSync 口径）
                },
                MtimePrecision::Nanos,
                true,
                true,
                false,
                false, // fs 事件走本地 notify，非 provider 事件
                false,
            ),
            ProviderScheme::S3 => ProviderCaps::new(
                HashCaps {
                    md5: true, // 兼容面最广（调研方案 §3.1：按交集编码）
                    sha256: true,
                    crc64nvme: true,
                    blake3: false,
                },
                MtimePrecision::Millis,
                false, // S3 无 rename（copy+delete 模拟）
                true,
                true,
                false, // SQS/webhook 适配归后续 SPEC
                true,
            ),
            ProviderScheme::Webdav => ProviderCaps::default(), // 全保守
        }
    }
}
