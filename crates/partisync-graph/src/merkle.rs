//! 分级 Merkle 树（SPEC M2-WP03 契约 §2）：状态叶哈希按前缀分桶，桶哈希再分桶。
//!
//! 设计取舍（与规格同步）：
//! - 叶哈希 = blake3(canonical 状态编码) —— 三类叶各一种编码器
//!   （entry / tag / entry_tag），均不包含水位簿记字段；
//! - entry 叶包含全部可比字段（size/mtime/content_id/owner）——对账协议需要
//!   状态完备可比才能做"分歧区间修复"（规格 §3）；
//! - 分桶位宽：4bit(16) / 8bit(256) / 12bit(4096) —— 三层足够定位到「KPI 量级」的
//!   差异区间，再回到叶面按键集合差分；
//! - 全量叶即时计算（无增量节点）—— 10⁵ 叶亚秒级，留给增量树作为 KPI 不达标时的
//!   优化项（YAGNI）。

use blake3::Hasher;
use partisync_cas::content_hash;

/// 单条叶的「key + 桶内哈希」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leaf {
    pub key: String,
    pub hash: String,
}

/// entry 状态叶（含全部可比字段；不含水位）。
pub fn entry_leaf(
    path: &str,
    kind: i64,
    content_id: Option<&str>,
    owner: Option<&str>,
    size: u64,
    mtime_ns: u64,
) -> Leaf {
    let mut h = Hasher::new();
    h.update(b"E\x1f");
    h.update(path.as_bytes());
    h.update(b"\x1f");
    h.update(&kind.to_le_bytes());
    h.update(b"\x1f");
    h.update(content_id.unwrap_or("").as_bytes());
    h.update(b"\x1f");
    h.update(owner.unwrap_or("").as_bytes());
    h.update(b"\x1f");
    h.update(&size.to_le_bytes());
    h.update(b"\x1f");
    h.update(&mtime_ns.to_le_bytes());
    Leaf {
        key: path.to_string(),
        hash: h.finalize().to_hex().to_string(),
    }
}

/// tag 状态叶——LWW 水位 `updated_hlc` 不进叶哈希（落选方根永不收敛，
/// 这是规格 §风险节的期望行为）。
pub fn tag_leaf(tag_id: &str, name: &str, color: Option<&str>, deleted: bool) -> Leaf {
    let mut h = Hasher::new();
    h.update(b"T\x1f");
    h.update(tag_id.as_bytes());
    h.update(b"\x1f");
    h.update(name.as_bytes());
    h.update(b"\x1f");
    h.update(color.unwrap_or("").as_bytes());
    h.update(b"\x1f");
    h.update(if deleted { b"1" } else { b"0" });
    Leaf {
        key: format!("tag/{tag_id}"),
        hash: h.finalize().to_hex().to_string(),
    }
}

/// entry_tag 状态叶。
pub fn link_leaf(tag_id: &str, entry_path: &str, deleted: bool) -> Leaf {
    let mut h = Hasher::new();
    h.update(b"L\x1f");
    h.update(tag_id.as_bytes());
    h.update(b"\x1f");
    h.update(entry_path.as_bytes());
    h.update(b"\x1f");
    h.update(if deleted { b"1" } else { b"0" });
    Leaf {
        key: format!("link/{tag_id}\x1f{entry_path}"),
        hash: h.finalize().to_hex().to_string(),
    }
}

/// 桶哈希：桶内叶哈希升序拼接 → blake3 hex（顺序无关性：保证双方排序后一致）。
#[must_use]
pub fn bucket_hash(leaves: &[Leaf]) -> String {
    let mut h = Hasher::new();
    h.update(b"B\x1f");
    let mut hashes: Vec<&str> = leaves.iter().map(|l| l.hash.as_str()).collect();
    hashes.sort_unstable();
    h.update(hashes.join("\x1e").as_bytes());
    h.finalize().to_hex().to_string()
}

/// 按 `bits` 把叶分配到桶（key 的 blake3 前缀取低 bits 位）。
#[must_use]
pub fn bucket_leaves(leaves: &[Leaf], bits: BucketBits) -> Vec<(u32, Vec<Leaf>)> {
    let n = 1u32 << (bits as u32);
    let mut buckets: std::collections::BTreeMap<u32, Vec<Leaf>> =
        (0..n).map(|i| (i, Vec::new())).collect();
    for l in leaves {
        let key_hash = content_hash(l.key.as_bytes());
        // hex 高位字符对应 hash 高位字节
        let nibble = key_hash.as_bytes()[0] as u32;
        let idx = nibble & bits.mask();
        buckets.get_mut(&idx).unwrap().push(l.clone());
    }
    buckets.into_iter().collect()
}

/// 桶位宽档位（用于按档下钻；bit 数越大越精确）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucketBits {
    B4 = 4,
    B8 = 8,
    B12 = 12,
}

impl BucketBits {
    #[must_use]
    pub const fn mask(self) -> u32 {
        (1u32 << (self as u32)) - 1
    }
}

/// 全树根（叶集合 → 桶哈希；单层 16 桶够用：下钻由 reconcile 内部按桶号处理）。
#[must_use]
pub fn merkle_root(leaves: &[Leaf]) -> String {
    bucket_hash(leaves)
}
