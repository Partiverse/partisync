//! 可验证记忆层（SPEC M9-WP02 §2.2/§2.3；ADR-0029）：memory 实体身份 + 独立二叉证明树。
//!
//! 双树分离：[`crate::merkle`] 分桶树是对账语义（P7），本模块是承诺语义
//! （P20）：RFC 6962 式 MTH，叶哈希 = blake3(0x00 ‖ 叶编码)，内部节点 =
//! blake3(0x01 ‖ 左 ‖ 右)，叶按 memory_id 升序，空树 = blake3("")。
//! 审计路径 O(log n)，验证自包含（不需数据库）。
//!
//! 叶编码不含 hlc/deleted（簿记水位不进承诺，SPEC §2.2）：同内容双端独立
//! 写由 HLC LWW 收敛为同一胜者行后，叶集与根双端一致。

use blake3::Hasher;
use partisync_cas::content_hash;
use serde::Serialize;
use serde_json::Value;

/// memory 行（graph store `memory` 表视图）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct MemoryRow {
    pub memory_id: String,
    pub content: String,
    pub content_hash: String,
    pub tags: String,
    pub metadata: String,
    pub created_ns: i64,
    pub origin_device: String,
    pub hlc: Option<String>,
    pub deleted: i64,
}

/// 一次写入的结果（SPEC §2.2：同 (content, tags, metadata) 二次写幂等）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MemoryWriteOutcome {
    pub memory_id: String,
    pub deduplicated: bool,
}

/// 证明树根快照（`memory_root` 表视图；根为派生值，可全量重算）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct MemoryRootSnapshot {
    pub root: String,
    pub memory_count: i64,
    pub updated_ns: i64,
}

/// `memory_verify` 报告（SPEC §2.4）：快照根 vs 重算根 + 逐行列级校验。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyReport {
    pub snapshot_root: Option<String>,
    pub recomputed_root: String,
    pub memory_count: usize,
    /// 列级 content_hash 与 content 不符的行（篡改/损坏检出面）。
    pub content_mismatches: Vec<String>,
    pub ok: bool,
}

/// 确定性 JSON 序列化：对象键递归按字节序排序——不依赖 serde_json 的 Map
/// 实现（即使 preserve_order 被间接启用，canonical 面仍确定，SPEC §6-R2）。
#[must_use]
pub fn canonical_json(v: &Value) -> String {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
            keys.sort_unstable();
            let inner: Vec<String> = keys
                .iter()
                .map(|k| {
                    let val = map.get(*k).unwrap_or(&Value::Null);
                    format!(
                        "{}:{}",
                        Value::String((*k).to_string()),
                        canonical_json(val)
                    )
                })
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(canonical_json).collect();
            format!("[{}]", inner.join(","))
        }
        other => other.to_string(),
    }
}

/// tags 参数 → canonical JSON array 字符串。
#[must_use]
pub fn canonical_tags(tags: &[String]) -> String {
    canonical_json(&Value::Array(
        tags.iter().map(|t| Value::String(t.clone())).collect(),
    ))
}

/// 记忆内容身份：blake3("M\x1f" ‖ content ‖ tags/metadata canonical) hex
/// （SPEC §2.2；内容寻址 ⇒ 同身份二次写幂等）。
#[must_use]
pub fn memory_identity(content: &str, tags_canonical: &str, metadata_canonical: &str) -> String {
    let mut h = Hasher::new();
    h.update(b"M\x1f");
    h.update(content.as_bytes());
    h.update(b"\x1f");
    h.update(tags_canonical.as_bytes());
    h.update(b"\x1f");
    h.update(metadata_canonical.as_bytes());
    h.finalize().to_hex().to_string()
}

/// 列级校验面（verify 的 per-row 依据，独立于身份哈希）。
#[must_use]
pub fn content_digest(content: &str) -> String {
    content_hash(content.as_bytes())
}

/// 叶编码（canonical；不含 hlc/deleted——簿记水位不进承诺，SPEC §2.2）。
fn leaf_data(row: &MemoryRow) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(row.memory_id.as_bytes());
    b.push(0x1f);
    b.extend_from_slice(row.content_hash.as_bytes());
    b.push(0x1f);
    b.extend_from_slice(row.tags.as_bytes());
    b.push(0x1f);
    b.extend_from_slice(row.metadata.as_bytes());
    b.push(0x1f);
    b.extend_from_slice(&row.created_ns.to_le_bytes());
    b.push(0x1f);
    b.extend_from_slice(row.origin_device.as_bytes());
    b
}

/// 叶哈希 = blake3(0x00 ‖ 叶编码)（RFC 6962 叶前缀域分离）。
#[must_use]
pub fn leaf_hash(row: &MemoryRow) -> [u8; 32] {
    let mut h = Hasher::new();
    h.update(&[0x00]);
    h.update(&leaf_data(row));
    h.finalize().into()
}

/// RFC 6962 MTH（输入为叶哈希序列，n=1 恒等——叶哈希已含 0x00 前缀）。
fn mth(leaves: &[[u8; 32]]) -> [u8; 32] {
    match leaves.len() {
        0 => blake3::hash(b"").into(),
        1 => leaves[0],
        n => {
            let k = largest_power_of_two_lt(n);
            let mut h = Hasher::new();
            h.update(&[0x01]);
            h.update(mth(&leaves[..k]).as_slice());
            h.update(mth(&leaves[k..]).as_slice());
            h.finalize().into()
        }
    }
}

/// 全树根（叶哈希须按 memory_id 升序；确定性：同集同根，与插入顺序无关）。
#[must_use]
pub fn tree_root(sorted_leaf_hashes: &[[u8; 32]]) -> [u8; 32] {
    mth(sorted_leaf_hashes)
}

/// < n 的最大 2 的幂（RFC 6962 k 值）。
fn largest_power_of_two_lt(n: usize) -> usize {
    let k = usize::BITS - (n - 1).leading_zeros();
    (1usize << (k - 1)).max(1)
}

/// 审计路径（RFC 6962 §2.1.1 PATH 算法）：`leaf_hashes` 须按 key 升序。
/// 返回 None = index 越界。
#[must_use]
pub fn audit_path(leaf_hashes: &[[u8; 32]], index: usize) -> Option<Vec<[u8; 32]>> {
    if index >= leaf_hashes.len() {
        return None;
    }
    let mut path = Vec::new();
    walk_path(leaf_hashes, index, &mut path);
    Some(path)
}

fn walk_path(leaves: &[[u8; 32]], index: usize, path: &mut Vec<[u8; 32]>) {
    if leaves.len() <= 1 {
        return;
    }
    let k = largest_power_of_two_lt(leaves.len());
    if index < k {
        walk_path(&leaves[..k], index, path);
        path.push(mth(&leaves[k..]));
    } else {
        walk_path(&leaves[k..], index - k, path);
        path.push(mth(&leaves[..k]));
    }
}

/// 包含性验证（自包含：叶哈希 + 路径 + 叶索引 + 叶总数 + 根；不需数据库）。
///
/// RFC 9162 §2.1.3.2 双游标 (fn, sn) 算法：k 分裂与索引位非对齐（n=40 时
/// idx=32 落顶层右子树但为偶数），单凭奇偶判左右不成立——`fn == sn`
/// （奇右界）情形须按 RFC 走「sibling 在左 + 额外右移」路径。
#[must_use]
pub fn verify_inclusion(
    leaf: &[u8; 32],
    path: &[[u8; 32]],
    index: usize,
    leaf_count: usize,
    root: &[u8; 32],
) -> bool {
    if leaf_count == 0 || index >= leaf_count {
        return false;
    }
    let mut node = *leaf;
    let mut fnr = index;
    let mut sn = leaf_count - 1;
    for sibling in path {
        if fnr.is_multiple_of(2) && fnr != sn {
            // 偶索引且非奇右界：sibling 在右
            node = combine(&node, sibling);
            fnr >>= 1;
            sn >>= 1;
        } else {
            // LSB 置位（奇叶，sibling 在左）或 fn == sn（奇右界，左子树整块在左）
            node = combine(sibling, &node);
            if fnr.is_multiple_of(2) {
                // RFC 9162：fn == sn 且 LSB 未置位 → 右移至 LSB 置位或 0，再右移一位
                fnr >>= 1;
                sn >>= 1;
                while fnr != 0 && fnr.is_multiple_of(2) {
                    fnr >>= 1;
                    sn >>= 1;
                }
                fnr >>= 1;
                sn >>= 1;
            } else {
                fnr >>= 1;
                sn >>= 1;
            }
        }
    }
    fnr == 0 && sn == 0 && node == *root
}

fn combine(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut h = Hasher::new();
    h.update(&[0x01]);
    h.update(left);
    h.update(right);
    h.finalize().into()
}

/// `memory_search` 单条命中（SPEC §2.4：score 为匹配秩，非语义分）。
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
pub struct MemorySearchHit {
    pub memory_id: String,
    pub content: String,
    pub tags: String,
    pub metadata: String,
    pub created_ns: i64,
    pub origin_device: String,
    pub score: f64,
}

/// `memory_search` 报告（results 按 score 降序、次键 created_ns 降序）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemorySearchReport {
    pub results: Vec<MemorySearchHit>,
    pub total: i64,
}

/// `memory_verify(memory_id)` 单叶包含证明（SPEC §2.3/§2.4 有 id 分支；
/// 自包含：不需数据库即可验证，hex 编码）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MemoryInclusionProof {
    pub memory_id: String,
    pub leaf_hash: String,
    pub audit_path: Vec<String>,
    pub root: String,
    /// 重算即验：`verify_inclusion(leaf, path, idx, n, root)` 恒真
    /// （构造即验证；字段保留给不信任本进程的下游复核者）。
    pub ok: bool,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 单叶包含证明（SPEC §2.3 `inclusion_proof`）：rows 按 memory_id 升序
/// （[`crate::store::Store::memory_rows`] 口径），目标不存在 → None。
#[must_use]
pub fn inclusion_proof(rows: &[MemoryRow], memory_id: &str) -> Option<MemoryInclusionProof> {
    let idx = rows.iter().position(|r| r.memory_id == memory_id)?;
    let hashes: Vec<[u8; 32]> = rows.iter().map(leaf_hash).collect();
    let path = audit_path(&hashes, idx)?;
    let root = tree_root(&hashes);
    Some(MemoryInclusionProof {
        memory_id: memory_id.to_string(),
        leaf_hash: hex(&hashes[idx]),
        audit_path: path.iter().map(|p| hex(p)).collect(),
        root: hex(&root),
        ok: verify_inclusion(&hashes[idx], &path, idx, rows.len(), &root),
    })
}
