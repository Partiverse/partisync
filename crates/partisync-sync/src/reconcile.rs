//! Merkle 对账协议（SPEC M2-WP03 契约 §3）：快路径声音性 + 慢路径状态修复。
//!
//! 快路径：全 origin 覆盖相等时直接跳过（对账协议的根本可节省面）。
//! 慢路径：根比对 → 分歧桶下钻 → 桶内按键差分修复；反复迭代至根相等或 max_rounds。
//!
//! 修复语义：
//! - 仅单侧存在的叶 → 直接应用另一侧的全量字段；
//! - 双侧同 key 不同值 → 属主权威（owner 一致 ⇒ 以本地行权威，对方改；属主不同 ⇒ 冲突，
//!   沿用 P11 规则：来方改挂 `.conflict-{owner}`，血缘落入 `sync_conflict`）；
//! - 收敛后合并水位（origin 覆盖相等 ⇒ 状态收敛成立）。

use std::collections::{BTreeMap, BTreeSet};

use blake3::Hasher;
use partisync_core::PartisyError;
use partisync_graph::merkle::{entry_leaf, link_leaf, merkle_root, tag_leaf, Leaf};
use partisync_graph::store::{ApplyOutcome, EntryKind, Store};
use std::future::Future;

/// 对账统计。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReconcileStats {
    pub rounds: u32,
    pub fast_path: bool,
    pub converged: bool,
    pub repaired_a: u64,
    pub repaired_b: u64,
    pub conflicts: u64,
}

/// 对账选项。
#[derive(Debug, Clone, Copy)]
pub struct ReconcileOpts {
    pub max_rounds: u32,
}

impl Default for ReconcileOpts {
    fn default() -> Self {
        Self { max_rounds: 16 }
    }
}

/// 单节点的可观察状态叶（key + 哈希 + 完整字段集——对账修复需要）。
#[derive(Debug, Clone)]
pub struct StateLeaf {
    pub key: String,
    pub hash: String,
    pub kind: LeafKind,
}

#[derive(Debug, Clone)]
pub enum LeafKind {
    Entry {
        path: String,
        kind: i64,
        content: Option<String>,
        owner: Option<String>,
        size: i64,
        mtime_ns: i64,
    },
    Tag {
        id: String,
        name: String,
        color: Option<String>,
        deleted: bool,
    },
    Link {
        tag_id: String,
        entry_path: String,
        deleted: bool,
    },
}

/// 对账叶源抽象（SPEC M3-WP03 裁定 2）：协议（[`reconcile`]）只依赖此面——
/// 设备端 = `partisync_graph::Store` 实现，hub 端 = raft 面适配器（T06 接线）。
/// 方法签名与 `Store` 既有面一致（协议语义不变，回归线 = M2-WP03 测试原样过）。
#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)] // apply_remote_entry 与 Store 既有面签名一致
pub trait LeafSource {
    /// 本店设备 id（水位表键）。
    fn device_id(&self) -> impl Future<Output = Result<String, PartisyError>> + Send;
    /// 水位表（origin → hlc）。
    fn watermarks(
        &self,
    ) -> impl Future<Output = Result<Vec<(String, String)>, PartisyError>> + Send;
    /// 本店时钟顶。
    fn clock_top(&self) -> impl Future<Output = Result<Option<String>, PartisyError>> + Send;
    /// 记录「已应用对端水位」。
    fn note_applied(
        &self,
        origin: &str,
        hlc: &str,
    ) -> impl Future<Output = Result<(), PartisyError>> + Send;
    /// entry 状态叶。
    fn entry_state_leaves(
        &self,
    ) -> impl Future<
        Output = Result<Vec<(String, i64, Option<String>, Option<String>, i64, i64)>, PartisyError>,
    > + Send;
    /// tag 状态叶。
    fn tag_state_leaves(
        &self,
    ) -> impl Future<Output = Result<Vec<(String, String, Option<String>, i64)>, PartisyError>> + Send;
    /// link 状态叶。
    fn link_state_leaves(
        &self,
    ) -> impl Future<Output = Result<Vec<(String, String, i64)>, PartisyError>> + Send;
    /// 远端 entry 应用（修复 sink；含 P11 冲突改挂）。
    fn apply_remote_entry(
        &self,
        path: &str,
        name: &str,
        kind: EntryKind,
        size: u64,
        mtime_ns: u64,
        content: Option<(&str, u64)>,
        chunk_root: Option<&str>,
        owner_device: &str,
    ) -> impl Future<Output = Result<ApplyOutcome, PartisyError>> + Send;
    /// 远端 tag 应用（LWW）。
    fn apply_remote_tag(
        &self,
        id: &str,
        name: &str,
        color: Option<&str>,
        deleted: bool,
        hlc_key: &str,
    ) -> impl Future<Output = Result<bool, PartisyError>> + Send;
    /// 远端 tag-link 应用（LWW）。
    fn apply_remote_tag_link(
        &self,
        tag_id: &str,
        entry_path: &str,
        deleted: bool,
        hlc_key: &str,
    ) -> impl Future<Output = Result<bool, PartisyError>> + Send;
    /// 冲突血缘落档。
    fn record_conflict(
        &self,
        space_id: &str,
        base_path: &str,
        local_path: &str,
        incoming_path: &str,
        origin_device: &str,
        detected_hlc: &str,
    ) -> impl Future<Output = Result<String, PartisyError>> + Send;
}

impl LeafSource for Store {
    fn device_id(&self) -> impl Future<Output = Result<String, PartisyError>> + Send {
        Store::device_id(self)
    }
    fn watermarks(
        &self,
    ) -> impl Future<Output = Result<Vec<(String, String)>, PartisyError>> + Send {
        Store::watermarks(self)
    }
    fn clock_top(&self) -> impl Future<Output = Result<Option<String>, PartisyError>> + Send {
        Store::clock_top(self)
    }
    fn note_applied(
        &self,
        origin: &str,
        hlc: &str,
    ) -> impl Future<Output = Result<(), PartisyError>> + Send {
        Store::note_applied(self, origin, hlc)
    }
    fn entry_state_leaves(
        &self,
    ) -> impl Future<
        Output = Result<Vec<(String, i64, Option<String>, Option<String>, i64, i64)>, PartisyError>,
    > + Send {
        Store::entry_state_leaves(self)
    }
    fn tag_state_leaves(
        &self,
    ) -> impl Future<Output = Result<Vec<(String, String, Option<String>, i64)>, PartisyError>> + Send
    {
        Store::tag_state_leaves(self)
    }
    fn link_state_leaves(
        &self,
    ) -> impl Future<Output = Result<Vec<(String, String, i64)>, PartisyError>> + Send {
        Store::link_state_leaves(self)
    }
    fn apply_remote_entry(
        &self,
        path: &str,
        name: &str,
        kind: EntryKind,
        size: u64,
        mtime_ns: u64,
        content: Option<(&str, u64)>,
        chunk_root: Option<&str>,
        owner_device: &str,
    ) -> impl Future<Output = Result<ApplyOutcome, PartisyError>> + Send {
        Store::apply_remote_entry(
            self,
            path,
            name,
            kind,
            size,
            mtime_ns,
            content,
            chunk_root,
            owner_device,
        )
    }
    fn apply_remote_tag(
        &self,
        id: &str,
        name: &str,
        color: Option<&str>,
        deleted: bool,
        hlc_key: &str,
    ) -> impl Future<Output = Result<bool, PartisyError>> + Send {
        Store::apply_remote_tag(self, id, name, color, deleted, hlc_key)
    }
    fn apply_remote_tag_link(
        &self,
        tag_id: &str,
        entry_path: &str,
        deleted: bool,
        hlc_key: &str,
    ) -> impl Future<Output = Result<bool, PartisyError>> + Send {
        Store::apply_remote_tag_link(self, tag_id, entry_path, deleted, hlc_key)
    }
    fn record_conflict(
        &self,
        space_id: &str,
        base_path: &str,
        local_path: &str,
        incoming_path: &str,
        origin_device: &str,
        detected_hlc: &str,
    ) -> impl Future<Output = Result<String, PartisyError>> + Send {
        Store::record_conflict(
            self,
            space_id,
            base_path,
            local_path,
            incoming_path,
            origin_device,
            detected_hlc,
        )
    }
}

/// 对账协议内部扫描（hub 端测试/工具同口径复用；不校验权威性）。
pub async fn scan_leaves_pub<S: LeafSource>(
    store: &S,
) -> Result<Vec<StateLeaf>, partisync_core::error::PartisyError> {
    scan_leaves(store).await
}

async fn scan_leaves<S: LeafSource>(
    store: &S,
) -> Result<Vec<StateLeaf>, partisync_core::error::PartisyError> {
    let mut out = Vec::new();
    for (path, kind, content, owner, size, mtime_ns) in store.entry_state_leaves().await? {
        let l = entry_leaf(
            &path,
            kind,
            content.as_deref(),
            owner.as_deref(),
            u64::try_from(size).unwrap_or(0),
            u64::try_from(mtime_ns).unwrap_or(0),
        );
        out.push(StateLeaf {
            key: l.key,
            hash: l.hash,
            kind: LeafKind::Entry {
                path,
                kind,
                content,
                owner,
                size,
                mtime_ns,
            },
        });
    }
    for (id, name, color, deleted) in store.tag_state_leaves().await? {
        let l = tag_leaf(&id, &name, color.as_deref(), deleted != 0);
        out.push(StateLeaf {
            key: l.key,
            hash: l.hash,
            kind: LeafKind::Tag {
                id,
                name,
                color,
                deleted: deleted != 0,
            },
        });
    }
    for (tag_id, entry_path, deleted) in store.link_state_leaves().await? {
        let l = link_leaf(&tag_id, &entry_path, deleted != 0);
        out.push(StateLeaf {
            key: l.key,
            hash: l.hash,
            kind: LeafKind::Link {
                tag_id,
                entry_path,
                deleted: deleted != 0,
            },
        });
    }
    Ok(out)
}

fn bucket_root(leaves: &[&StateLeaf]) -> String {
    let mut h = Hasher::new();
    h.update(b"B\x1f");
    let mut hashes: Vec<&str> = leaves.iter().map(|l| l.hash.as_str()).collect();
    hashes.sort_unstable();
    h.update(hashes.join("\x1e").as_bytes());
    h.finalize().to_hex().to_string()
}

fn as_merkle_leaves(items: &[StateLeaf]) -> Vec<Leaf> {
    items
        .iter()
        .map(|s| Leaf {
            key: s.key.clone(),
            hash: s.hash.clone(),
        })
        .collect()
}

/// 快路径声音性判定（SPEC §3）。
async fn fast_path_eligible<A: LeafSource, B: LeafSource>(
    a: &A,
    b: &B,
) -> Result<bool, partisync_core::error::PartisyError> {
    let am: BTreeMap<String, String> = a.watermarks().await?.into_iter().collect();
    let bm: BTreeMap<String, String> = b.watermarks().await?.into_iter().collect();
    let at = a.clock_top().await?;
    let bt = b.clock_top().await?;
    let a_dev = a.device_id().await?;
    let b_dev = b.device_id().await?;
    if let Some(t) = &at {
        if bm.get(&a_dev).cloned().unwrap_or_default() < *t {
            return Ok(false);
        }
    }
    if let Some(t) = &bt {
        if am.get(&b_dev).cloned().unwrap_or_default() < *t {
            return Ok(false);
        }
    }
    // 共同登记的 origin 上水位须相等（只看交集——单侧独占的 origin 由对方时钟顶覆盖
    // 条件兜底；「a 单方面登记的 origin ⇒ a 已见过该 origin 所有写入 ⇒ b 缺它即状态
    // 不等」会在慢路径根比对时被检出——不构成假阴性）
    for (d, v) in &am {
        if let Some(vb) = bm.get(d) {
            if vb != v {
                return Ok(false);
            }
        }
    }
    for (d, v) in &bm {
        if let Some(va) = am.get(d) {
            if va != v {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// Merkle 对账主体。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn reconcile<A: LeafSource, B: LeafSource>(
    a: &A,
    b: &B,
    opts: ReconcileOpts,
) -> Result<ReconcileStats, partisync_core::error::PartisyError> {
    let mut stats = ReconcileStats::default();
    if fast_path_eligible(a, b).await? {
        stats.fast_path = true;
        stats.converged = true;
        return Ok(stats);
    }
    let a_dev = a.device_id().await?;
    let b_dev = b.device_id().await?;
    for r in 1..=opts.max_rounds {
        stats.rounds = r;
        let a_leaves = scan_leaves(a).await?;
        let b_leaves = scan_leaves(b).await?;
        let ra = merkle_root(&as_merkle_leaves(&a_leaves));
        let rb = merkle_root(&as_merkle_leaves(&b_leaves));
        if ra == rb {
            stats.converged = true;
            finalize_watermarks(a, b, &a_dev, &b_dev).await?;
            return Ok(stats);
        }
        // 根不等 → 下钻（L1 4bit 桶）→ 桶内差异再下钻（L2 8bit）→ 桶内按键差分
        let buckets_a = partisync_graph::merkle::bucket_leaves(
            &as_merkle_leaves(&a_leaves),
            partisync_graph::merkle::BucketBits::B4,
        );
        let buckets_b = partisync_graph::merkle::bucket_leaves(
            &as_merkle_leaves(&b_leaves),
            partisync_graph::merkle::BucketBits::B4,
        );
        let ba: BTreeMap<u32, Vec<Leaf>> = buckets_a.into_iter().collect();
        let bb: BTreeMap<u32, Vec<Leaf>> = buckets_b.into_iter().collect();
        let a_keys_by_leaf: BTreeMap<String, &StateLeaf> =
            a_leaves.iter().map(|l| (l.key.clone(), l)).collect();
        let b_keys_by_leaf: BTreeMap<String, &StateLeaf> =
            b_leaves.iter().map(|l| (l.key.clone(), l)).collect();
        let mut repaired_a = 0u64;
        let mut repaired_b = 0u64;
        for (idx, av_leaves) in &ba {
            let bv_leaves = bb.get(idx).cloned().unwrap_or_default();
            let a_sub: Vec<&StateLeaf> = av_leaves
                .iter()
                .filter_map(|l| a_keys_by_leaf.get(&l.key).copied())
                .collect();
            let b_sub: Vec<&StateLeaf> = bv_leaves
                .iter()
                .filter_map(|l| b_keys_by_leaf.get(&l.key).copied())
                .collect();
            if bucket_root(&a_sub) == bucket_root(&b_sub) {
                continue;
            }
            // 桶内：差分 + 同 key 不同值决胜
            let a_key_set: BTreeSet<&str> = a_sub.iter().map(|l| l.key.as_str()).collect();
            let b_key_set: BTreeSet<&str> = b_sub.iter().map(|l| l.key.as_str()).collect();
            for leaf in &a_sub {
                if !b_key_set.contains(leaf.key.as_str()) {
                    apply_to(b, leaf).await?;
                    repaired_b += 1;
                } else if let Some(other) = b_keys_by_leaf.get(&leaf.key) {
                    if other.hash != leaf.hash {
                        resolve_conflict(a, b, leaf, other, &mut stats).await?;
                    }
                }
            }
            for leaf in &b_sub {
                if !a_key_set.contains(leaf.key.as_str()) {
                    apply_to(a, leaf).await?;
                    repaired_a += 1;
                }
            }
        }
        stats.repaired_a += repaired_a;
        stats.repaired_b += repaired_b;
        finalize_watermarks(a, b, &a_dev, &b_dev).await?;
    }
    Ok(stats)
}

async fn finalize_watermarks<A: LeafSource, B: LeafSource>(
    a: &A,
    b: &B,
    a_dev: &str,
    b_dev: &str,
) -> Result<(), partisync_core::error::PartisyError> {
    if let Some(t) = a.clock_top().await? {
        b.note_applied(a_dev, &t).await?;
    }
    if let Some(t) = b.clock_top().await? {
        a.note_applied(b_dev, &t).await?;
    }
    Ok(())
}

/// 把 a 持有的全量字段应用为 b 的状态行；owner = b 的本机（让对端属主行被尊重）。
async fn apply_to<S: LeafSource>(
    dst: &S,
    leaf: &StateLeaf,
) -> Result<(), partisync_core::error::PartisyError> {
    match &leaf.kind {
        LeafKind::Entry {
            path,
            kind,
            content,
            owner,
            size,
            mtime_ns,
        } => {
            let e_kind = if *kind == 1 {
                EntryKind::Dir
            } else {
                EntryKind::File
            };
            // 属主使用原 owner（这是状态全量行的真相——对端节点自己的属主行）；
            // 调用方是 a → 应用到 b；owner 应是「创建此状态的属主」，与 dst 无关。
            let outcome = dst
                .apply_remote_entry(
                    path,
                    path.trim_start_matches('/')
                        .rsplit('/')
                        .next()
                        .unwrap_or(path),
                    e_kind,
                    u64::try_from(*size).unwrap_or(0),
                    u64::try_from(*mtime_ns).unwrap_or(0),
                    content
                        .as_deref()
                        .map(|h| (h, u64::try_from(*size).unwrap_or(0))),
                    None,
                    owner.as_deref().unwrap_or("unknown"),
                )
                .await?;
            if let Some(c) = &outcome.conflict {
                // 保留两者 + 血缘落档（spec §3 P11 语义）
                let _ = dst
                    .record_conflict(
                        "default",
                        &c.base_path,
                        &c.base_path,
                        &c.incoming_path,
                        owner.as_deref().unwrap_or("unknown"),
                        dst.clock_top().await?.as_deref().unwrap_or(""),
                    )
                    .await?;
            }
        }
        LeafKind::Tag {
            id,
            name,
            color,
            deleted,
        } => {
            // 以对方时钟顶为 LWW 键（保证单调——若对方时钟顶为空，使用 ULID 兜底）
            let hlc = dst
                .clock_top()
                .await?
                .unwrap_or_else(|| partisync_core::Hlc::from_wall(0, 0).to_key());
            let _ = dst
                .apply_remote_tag(id, name, color.as_deref(), *deleted, &hlc)
                .await?;
        }
        LeafKind::Link {
            tag_id,
            entry_path,
            deleted,
        } => {
            let hlc = dst
                .clock_top()
                .await?
                .unwrap_or_else(|| partisync_core::Hlc::from_wall(0, 0).to_key());
            let _ = dst
                .apply_remote_tag_link(tag_id, entry_path, *deleted, &hlc)
                .await?;
        }
    }
    Ok(())
}

/// 同 key 不同值——按规格 §3 决胜（属主权威 + 字典序决胜）。
async fn resolve_conflict<A: LeafSource, B: LeafSource>(
    a: &A,
    b: &B,
    la: &StateLeaf,
    lb: &StateLeaf,
    stats: &mut ReconcileStats,
) -> Result<(), partisync_core::error::PartisyError> {
    match (&la.kind, &lb.kind) {
        (LeafKind::Entry { owner: oa, .. }, LeafKind::Entry { owner: ob, .. }) => {
            let oa = oa.clone().unwrap_or_else(|| "unknown".into());
            let ob = ob.clone().unwrap_or_else(|| "unknown".into());
            if oa == ob {
                // 同属主权威：以本地行（a 视角）覆盖对方
                apply_to(b, la).await?;
            } else if oa != ob && la.hash != lb.hash {
                // 属主不同：来方改挂冲突后缀——a 把自己持有的版本投递到 b 触发该路径
                apply_to(b, la).await?;
                stats.conflicts += 1;
            }
        }
        (
            LeafKind::Tag {
                id,
                name,
                color,
                deleted,
                ..
            },
            LeafKind::Tag {
                name: name2,
                color: color2,
                deleted: deleted2,
                ..
            },
        ) => {
            // Tag 双侧值不同：以字典序决胜（确定性）
            let a_str = format!(
                "{id}|{name}|{}|{}",
                color.clone().unwrap_or_default(),
                deleted
            );
            let b_str = format!(
                "{id}|{name2}|{}|{}",
                color2.clone().unwrap_or_default(),
                deleted2
            );
            if a_str >= b_str {
                apply_to(a, lb).await?; // b 的胜 → 把 b 投到 a
            } else {
                apply_to(b, la).await?; // a 的胜
            }
            let _ = a;
            let _ = b;
        }
        (LeafKind::Link { .. }, LeafKind::Link { .. }) => {
            // Link 双侧值不同：以字典序决胜（与 Tag 同）
            let a_str = format!("{:?}", la.kind);
            let b_str = format!("{:?}", lb.kind);
            if a_str >= b_str {
                apply_to(a, lb).await?;
            } else {
                apply_to(b, la).await?;
            }
        }
        _ => {}
    }
    Ok(())
}
