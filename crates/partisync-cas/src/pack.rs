//! pack v2 格式（SPEC M3-WP04 裁定 1）：头部 + EC 保护数据区。
//!
//! 布局：
//! ```text
//! [header 72B]（magic/ver/index_len/data_len/shard_len/计数）
//! [shard hashes 14×32B]（blake3(shard)；损坏检测——读取时校验，
//!   不符的分片置缺失 → 统一走 EC 重建）
//! [EC 条带流 14×shard_len]（[index_len u64][index JSON][块字节流+填充]；
//!   索引在 EC 保护区内——索引区损坏随分片重建恢复，SPEC 验收
//!   「含索引区损坏重建」）
//! ```
//! 读取：header → 分片流（缺失触发 EC 重建）→ 先解 index → 按
//! (offset,len) 切出块字节。

use crate::ec::{shards_to_data, EcCodec, DATA_SHARDS, PARITY_SHARDS, TOTAL_SHARDS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// 魔数（"PSPK"）。
pub const PACK_MAGIC: &[u8; 4] = b"PSPK";
/// pack v2 版本号（v1 未曾落地，编号跳过历史计划）。
pub const PACK_VERSION: u32 = 2;
/// 头部定长（magic4+ver4+index_len8+data_len8+stripe_len8+block_count8
/// +data_shards4+parity4+reserved24）。
pub const HEADER_LEN: usize = 72;
/// 分片哈希区长度（14 × blake3 32B）。
pub const SHARD_HASH_LEN: usize = crate::ec::TOTAL_SHARDS * 32;

/// pack 头部（EC 之外）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackHeader {
    /// 索引 JSON 字节长度（数据区起始处）。
    pub index_len: u64,
    /// 数据区总长（含索引段与填充）。
    pub data_len: u64,
    /// 条带分片长度。
    pub stripe_len: u64,
    /// 块数。
    pub block_count: u64,
}

/// 索引条目：块 hash → 数据区内 (offset, len)（offset 自数据区起点起算）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    /// 块哈希（blake3 hex）。
    pub hash: String,
    /// 数据区内偏移。
    pub offset: u64,
    /// 字节长度。
    pub len: u64,
}

/// pack 索引（serde JSON 存于数据区起始段，EC 保护）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackIndex {
    /// 索引条目（按 hash 升序——构建确定性）。
    pub entries: Vec<IndexEntry>,
}

/// 构建错误。
#[derive(Debug)]
pub enum PackError {
    /// 重复块哈希。
    DuplicateHash(String),
    /// 头部非法（magic/版本/长度）。
    BadHeader(&'static str),
    /// EC 编解码错误。
    Ec(reed_solomon_erasure::Error),
    /// 索引中找不到块。
    UnknownBlock(String),
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateHash(h) => write!(f, "pack: duplicate hash {h}"),
            Self::BadHeader(w) => write!(f, "pack: bad header ({w})"),
            Self::Ec(e) => write!(f, "pack: ec: {e}"),
            Self::UnknownBlock(h) => write!(f, "pack: unknown block {h}"),
        }
    }
}

impl std::error::Error for PackError {}

/// 从块集构建 pack 文件字节（确定性：按 hash 升序排布）。
///
/// # Errors
/// 重复块哈希或 EC 编码错误。
pub fn build_pack(blocks: &[(String, Vec<u8>)]) -> Result<Vec<u8>, PackError> {
    let mut sorted: Vec<&(String, Vec<u8>)> = blocks.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut index = PackIndex::default();
    let mut body: Vec<u8> = Vec::new();
    let mut body_offsets: Vec<u64> = Vec::with_capacity(sorted.len());
    for (hash, bytes) in &sorted {
        if index.entries.iter().any(|e| e.hash == *hash) {
            return Err(PackError::DuplicateHash((*hash).clone()));
        }
        // 占位 offset；最终写入「数据区起点 + 数据头长度 + body 内偏移」
        //（数据头 = [index_len u64][index json]）。
        index.entries.push(IndexEntry {
            hash: (*hash).clone(),
            offset: body.len() as u64,
            len: bytes.len() as u64,
        });
        body_offsets.push(body.len() as u64);
        body.extend_from_slice(bytes);
    }
    // 数据区 = [index_len u64][index json][body]（EC 保护区内）
    // 两遍序列化：offset 需要数据头长度，但数据头长度由 index json 长度决定。
    let mut index_json =
        serde_json::to_vec(&index).map_err(|_e| PackError::BadHeader("index encode"))?;
    // 迭代到 fixed point：offset 变长可能让 json 变长；固定点通常 1-2 轮。
    // 每次重序列化，反映当前 entry.offset；stable 时收敛。
    loop {
        let data_header_len = 8 + index_json.len() as u64;
        let mut changed = false;
        for (i, entry) in index.entries.iter_mut().enumerate() {
            let new_off = data_header_len + body_offsets[i];
            if entry.offset != new_off {
                entry.offset = new_off;
                changed = true;
            }
        }
        // 始终用最新 entry 重序列化；稳定（无变化 或 长度已稳）即收敛。
        let new_json =
            serde_json::to_vec(&index).map_err(|_e| PackError::BadHeader("index encode"))?;
        if !changed || new_json.len() == index_json.len() {
            index_json = new_json;
            break;
        }
        index_json = new_json;
    }
    let data_header_len = 8 + index_json.len() as u64;
    let mut data: Vec<u8> = Vec::with_capacity(data_header_len as usize + body.len());
    data.extend_from_slice(&(index_json.len() as u64).to_be_bytes());
    data.extend_from_slice(&index_json);
    data.extend_from_slice(&body);

    let codec = EcCodec::new().map_err(PackError::Ec)?;
    let stripe_len = crate::ec::shard_len_for(data.len());
    let shards = codec.encode(&data).map_err(PackError::Ec)?;

    let mut out = Vec::with_capacity(HEADER_LEN + SHARD_HASH_LEN + data.len());
    out.extend_from_slice(PACK_MAGIC);
    out.extend_from_slice(&PACK_VERSION.to_be_bytes());
    out.extend_from_slice(&(index_json.len() as u64).to_be_bytes());
    out.extend_from_slice(&(data.len() as u64).to_be_bytes());
    out.extend_from_slice(&(stripe_len as u64).to_be_bytes());
    out.extend_from_slice(&(blocks.len() as u64).to_be_bytes());
    out.extend_from_slice(&(DATA_SHARDS as u32).to_be_bytes());
    out.extend_from_slice(&(PARITY_SHARDS as u32).to_be_bytes());
    out.extend_from_slice(&[0u8; 24]); // 保留
    debug_assert_eq!(out.len(), HEADER_LEN);
    for shard in &shards {
        out.extend_from_slice(blake3::hash(shard).as_bytes());
    }
    for shard in &shards {
        out.extend_from_slice(shard);
    }
    Ok(out)
}

/// 单个分片的可选视图（`None` 表示该分片缺失或损坏——走 EC 重建）。
pub type ShardView = Vec<Option<Vec<u8>>>;

/// 头部解析 + 分片切分：`pack` → (`header`, 分片 `Option` 视图)。
/// 文件字节照原样分片（供模拟损坏/缺失）。
///
/// # Errors
/// magic/版本/长度非法或文件截断。
pub fn parse_shards(pack: &[u8]) -> Result<(PackHeader, ShardView), PackError> {
    if pack.len() < HEADER_LEN || &pack[..4] != PACK_MAGIC {
        return Err(PackError::BadHeader("magic"));
    }
    let be = |r: &[u8]| -> u64 { u64::from_be_bytes(r.try_into().expect("fixed slice")) };
    let ver = u32::from_be_bytes(pack[4..8].try_into().expect("fixed"));
    if ver != PACK_VERSION {
        return Err(PackError::BadHeader("version"));
    }
    let header = PackHeader {
        index_len: be(&pack[8..16]),
        data_len: be(&pack[16..24]),
        stripe_len: be(&pack[24..32]),
        block_count: be(&pack[32..40]),
    };
    let shard_len = header.stripe_len as usize;
    let stream_len = TOTAL_SHARDS
        .checked_mul(shard_len)
        .ok_or(PackError::BadHeader("stream_len"))?;
    if header.data_len == 0 || header.data_len as usize > stream_len {
        return Err(PackError::BadHeader("data_len"));
    }
    if pack.len() < HEADER_LEN + SHARD_HASH_LEN + stream_len {
        return Err(PackError::BadHeader("truncated"));
    }
    let hash_region = &pack[HEADER_LEN..HEADER_LEN + SHARD_HASH_LEN];
    let raw = &pack[HEADER_LEN + SHARD_HASH_LEN..HEADER_LEN + SHARD_HASH_LEN + stream_len];
    let mut shards: Vec<Option<Vec<u8>>> = Vec::with_capacity(TOTAL_SHARDS);
    for i in 0..TOTAL_SHARDS {
        let want = &hash_region[i * 32..(i + 1) * 32];
        let shard_bytes = &raw[i * shard_len..(i + 1) * shard_len];
        let got = blake3::hash(shard_bytes);
        if got.as_bytes() == want {
            shards.push(Some(shard_bytes.to_vec()));
        } else {
            // 静默损坏 → 置缺失（统一走 EC 重建）
            shards.push(None);
        }
    }
    Ok((header, shards))
}

/// 数据区解码：缺失分片触发 EC 重建 → 返回 (header, 数据区完整字节)。
///
/// # Errors
/// 头部非法或缺失分片 > 4。
pub fn decode_data(pack: &[u8]) -> Result<(PackHeader, Vec<u8>), PackError> {
    let (header, mut shards) = parse_shards(pack)?;
    let header_for_ret = header.clone();
    let codec = EcCodec::new().map_err(PackError::Ec)?;
    if shards.iter().any(Option::is_none) {
        codec.reconstruct(&mut shards).map_err(PackError::Ec)?;
    }
    let present: Vec<Vec<u8>> = shards.into_iter().flatten().collect();
    if present.len() != TOTAL_SHARDS {
        return Err(PackError::BadHeader("shard count"));
    }
    let data_len = header.data_len as usize;
    Ok((header_for_ret, shards_to_data(&present, data_len)))
}

/// 从 pack 读索引。
///
/// # Errors
/// 头部/EC/索引解析失败。
pub fn read_index(pack: &[u8]) -> Result<PackIndex, PackError> {
    let (_, data) = decode_data(pack)?;
    let ilen = u64::from_be_bytes(
        data[..8]
            .try_into()
            .map_err(|_| PackError::BadHeader("index_len"))?,
    ) as usize;
    if data.len() < 8 + ilen {
        return Err(PackError::BadHeader("index truncated"));
    }
    serde_json::from_slice(&data[8..8 + ilen]).map_err(|_| PackError::BadHeader("index json"))
}

/// 从 pack 读取单块（解码数据区 → 索引定位 → 切片）。
///
/// # Errors
/// 索引/EC/未知块。
pub fn read_block(pack: &[u8], index: &PackIndex, hash: &str) -> Result<Vec<u8>, PackError> {
    let (_, data) = decode_data(pack)?;
    let entry = index
        .entries
        .iter()
        .find(|e| e.hash == hash)
        .ok_or_else(|| PackError::UnknownBlock(hash.to_owned()))?;
    let start = entry.offset as usize;
    let end = start + entry.len as usize;
    if data.len() < end {
        return Err(PackError::BadHeader("block out of range"));
    }
    Ok(data[start..end].to_vec())
}

/// 索引的 map 视图（hash → entry）便利方法。
#[must_use]
pub fn index_map(index: &PackIndex) -> BTreeMap<String, IndexEntry> {
    index
        .entries
        .iter()
        .map(|e| (e.hash.clone(), e.clone()))
        .collect()
}
