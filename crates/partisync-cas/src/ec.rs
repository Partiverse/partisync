//! RS(10,4) 纠删编解码（SPEC M3-WP04 裁定 2，ADR-0014）。
//!
//! 条带模型：数据区按 [`DATA_SHARDS`]（10）分片为等长 shard（末片零填充），
//! 编码出 [`PARITY_SHARDS`]（4）校验片——单 pack 损坏 ≤4 分片可重建
//! （reed-solomon-erasure 6.x，ADR-0014；分片数变更 = pack 版本升级）。

use reed_solomon_erasure::galois_8::ReedSolomon;

/// 数据分片数。
pub const DATA_SHARDS: usize = 10;
/// 校验分片数。
pub const PARITY_SHARDS: usize = 4;
/// 总分片数。
pub const TOTAL_SHARDS: usize = DATA_SHARDS + PARITY_SHARDS;

/// RS(10,4) 编解码器（线程安全；按 shard_len 缓存矩阵由 crate 内部处理）。
pub struct EcCodec {
    codec: ReedSolomon,
}

impl EcCodec {
    /// 固定 10+4 编解码器（分片数变更 = pack 版本升级，见模块文档）。
    ///
    /// # Errors
    /// crate 参数校验失败（10+4 为合法常量，运行时不会触发）。
    pub fn new() -> Result<Self, reed_solomon_erasure::Error> {
        Ok(Self {
            codec: ReedSolomon::new(DATA_SHARDS, PARITY_SHARDS)?,
        })
    }

    /// 数据 → 14 分片（末数据分片零填充至等长；返回分片含填充）。
    ///
    /// # Errors
    /// crate 编码错误。
    pub fn encode(
        &self,
        data: &[u8],
    ) -> std::result::Result<Vec<Vec<u8>>, reed_solomon_erasure::Error> {
        let shard_len = shard_len_for(data.len());
        let mut shards: Vec<Vec<u8>> = Vec::with_capacity(TOTAL_SHARDS);
        for i in 0..DATA_SHARDS {
            let start = i * shard_len;
            let end = (start + shard_len).min(data.len());
            let mut shard = vec![0u8; shard_len];
            if start < data.len() {
                shard[..end - start].copy_from_slice(&data[start..end]);
            }
            shards.push(shard);
        }
        for _ in 0..PARITY_SHARDS {
            shards.push(vec![0u8; shard_len]);
        }
        self.codec.encode(&mut shards)?;
        Ok(shards)
    }

    /// 分片重建：`shards` 缺失处置 `None`（≤[`PARITY_SHARDS`] 个缺失），
    /// 成功后全部补齐（含校验片）。
    ///
    /// # Errors
    /// 缺失数 > 4 或分片长度不一致。
    pub fn reconstruct(
        &self,
        shards: &mut [Option<Vec<u8>>],
    ) -> std::result::Result<(), reed_solomon_erasure::Error> {
        self.codec.reconstruct(shards)
    }
}

/// 数据长度 → 等长分片尺寸（entry 数据分片等长，末片零填充）。
#[must_use]
pub fn shard_len_for(data_len: usize) -> usize {
    data_len.div_ceil(DATA_SHARDS)
}

/// 分片 → 数据（截除末分片填充；`data_len` 为原始字节数）。
#[must_use]
pub fn shards_to_data(shards: &[Vec<u8>], data_len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(data_len);
    for shard in shards.iter().take(DATA_SHARDS) {
        out.extend_from_slice(shard);
    }
    out.truncate(data_len);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_roundtrip_no_loss() {
        let codec = EcCodec::new().expect("codec");
        let data: Vec<u8> = (0..=255u8).cycle().take(10_000).collect();
        let shards = codec.encode(&data).expect("encode");
        assert_eq!(shards.len(), TOTAL_SHARDS);
        assert_eq!(shards_to_data(&shards, data.len()), data);
    }

    #[test]
    fn reconstruct_with_max_losses() {
        let codec = EcCodec::new().expect("codec");
        let data = b"pack erasure payload 0123456789".to_vec();
        let mut shards: Vec<Option<Vec<u8>>> = codec
            .encode(&data)
            .expect("encode")
            .into_iter()
            .map(Some)
            .collect();
        // 恰好 4 片丢失（含数据片与校验片混合）——上限形态
        shards[0] = None;
        shards[5] = None;
        shards[9] = None;
        shards[13] = None;
        codec.reconstruct(&mut shards).expect("reconstruct");
        let restored: Vec<Vec<u8>> = shards.into_iter().flatten().collect();
        assert_eq!(shards_to_data(&restored, data.len()), data);
    }

    #[test]
    fn reconstruct_fails_with_five_losses() {
        let codec = EcCodec::new().expect("codec");
        let data = vec![7u8; 3_333]; // 非整除长度（末片填充路径）
        let mut shards: Vec<Option<Vec<u8>>> = codec
            .encode(&data)
            .expect("encode")
            .into_iter()
            .map(Some)
            .collect();
        for s in shards.iter_mut().take(5) {
            *s = None;
        }
        assert!(codec.reconstruct(&mut shards).is_err());
    }
}
