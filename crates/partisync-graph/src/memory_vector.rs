//! 记忆域向量通道底座（SPEC M11-WP06 §2.1/§2.2/§2.5；ADR-0035 决策 1/4/5）。
//!
//! 记忆域三腿（写入伴生/查询融合/生命周期同步，memory-vector-eval.md:71-76）
//! 的无模型底座件：
//! - key 派生：[`memory_vector_key`] = blake3(memory_id) 前 8 字节大端 u64
//!   （沿 content_key 判例 crates/partisync-index/src/search/vector.rs:22-28；
//!   碰撞口径 2⁶⁴ 生日界同判例 vector.rs:9-14）——memory 专享 key 域。
//! - 索引包装：[`MemoryVectorIndex`] **独立 usearch 实例**（与资产域
//!   VectorStore 物理隔离，ADR-0035 决策 5 混域出局），dim = [`MEMORY_VECTOR_DIM`]
//!   （SPEC §2.2 维度契约，BGESmallZHV15 本次实测 512），Cos 距离，F32 量化。
//!   artifact = `memory_vec.usearch`（落盘位置由宿主 partisd 决定，T04 接线）。
//! - RRF 融合：[`rrf_fuse`]（k=60，对齐 hybrid.rs:25 判例）graph 本地实现，
//!   领域层不得反向依赖 index crate；与
//!   crates/partisync-index/src/search/hybrid.rs:85-117 的 ~20 行重复为已登记
//!   债务（SPEC §6-Q6 / ADR-0035 决策 4，处置候选 (a)）。
//! - 预计算向量注入：嵌入与落库分离（SPEC §2.1）——嵌入调用面（fastembed，
//!   `graph-embed` feature 门控，T04 接线）输出 `Vec<f32>`，落库 API（本模块
//!   [`MemoryVectorIndex::upsert`]）接受预计算向量，使 P25 (b)(c) 契约可
//!   无模型测试。
//!
//! usearch 2.26.2 API 口径（pinned 源码核实：cargo registry
//! `usearch-2.26.2/rust/lib.rs`，非凭记忆）：
//! - `Key = u64`（lib.rs:47）；`Index::new(&IndexOptions)`（:1340）/`restore`
//!   （:1369）/`save`（:1714）；`add(key, &[f32])`（:1507）/`remove`（:1626，
//!   返回移除计数）/`contains`（:1659）/`search(&[f32], count)`（:1449 →
//!   `Matches { keys, distances }` :329）/`reserve`（:1555）/`size`/`capacity`
//! - `MetricKind::Cos` 返回余弦**距离**（1 − cos_sim）：完全相同向量 → 距离 0
//!   （判例 vector.rs:13-14）
//! - `ScalarKind::F32` 量化（KPI 口径，判例 vector.rs:14）
//! - 容量先行（usearch README「Reserve capacity ahead of insertions!」；
//!   判例 vector.rs:207：满则 2×（至少 1024）扩容）

use std::path::Path;
use std::sync::RwLock;

use partisync_core::error::{PartisyError, Severity};

/// 记忆域向量维度契约（SPEC M11-WP06 §2.2：BGESmallZHV15 本次实测 512，
/// 非 in-repo 判例推断；与 TextDenseZh512 判例对齐 vector.rs:39-41）。
pub const MEMORY_VECTOR_DIM: usize = 512;

/// 降级标记（SPEC M11-WP06 §2.4 fail-open）：向量腿不可用时响应载荷
/// 携带此值，不静默、不报错。ADR-0035 决策 2 落锤。
pub const DEGRADED_VECTOR_UNAVAILABLE: &str = "vector_unavailable";

/// 嵌入器初始化探针内容（SPEC §2.2 init fail-fast 的首嵌校验输入：
/// attach 时对嵌入器试嵌一次校验维度；空串对嵌入模型是合法输入）。
pub const EMBED_PROBE_CONTENT: &str = "";

/// RRF 融合 k 值（SPEC M11-WP06 §2.5：对齐 hybrid.rs:25 判例 k=60）。
const RRF_K: f32 = 60.0;

/// 嵌入调用面接口（SPEC M11-WP06 §2.1：嵌入与落库分离——嵌入器输出
/// `Vec<f32>`，落库走 [`MemoryVectorIndex::upsert`] 预计算向量注入）。
/// fastembed 实现随 `graph-embed` feature 门控落地（默认关）；本 trait
/// 使 P25 全链契约可无模型测试（测试注入合成嵌入器）。
pub trait MemoryEmbedder: Send + Sync {
    /// 嵌入一段文本；维度必须 == [`MEMORY_VECTOR_DIM`]（attach 时首嵌
    /// fail-fast 校验，SPEC §2.2；运行期偏差由查询腿 fail-open 兜底）。
    ///
    /// # Errors
    /// 模型/运行时故障 → Err（查询腿降级，写入腿记伴表 failed）。
    fn embed(&self, content: &str) -> Result<Vec<f32>, PartisyError>;
}

/// 记忆域向量通道：索引实例 + 可选嵌入器（T04 store 三腿接线持有件）。
/// `embedder = None`（或通道未挂载）= 向量腿不可用 → 查询腿降级
/// （SPEC §2.4）；通道缺位 = 派生数据不在本机，可随时回填重建
/// （ADR-0035 决策 3）。
pub struct MemoryVectorChannel {
    pub index: std::sync::Arc<MemoryVectorIndex>,
    pub embedder: Option<std::sync::Arc<dyn MemoryEmbedder>>,
}

// 通道随 Store 进异步上下文（sqlx 并发面），Send+Sync 是硬前提：
// RwLock<usearch::Index> 的线程安全性与 index crate VectorStore 判例同构
// （vector.rs:74-80）；此处编译期钉死，漂移即编译错。
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MemoryVectorIndex>();
    assert_send_sync::<MemoryVectorChannel>();
};

/// memory 专享 key 派生（ADR-0035 决策 5）：blake3(memory_id) 前 8 字节大端
/// u64。key 仅由 memory_id 决定（P25-b）；与资产域 content_key（vector.rs:22-28）
/// 同式但**物理隔离**——独立 usearch 实例，两域 key 空间不交叉。
#[must_use]
pub fn memory_vector_key(memory_id: &str) -> u64 {
    let h = blake3::hash(memory_id.as_bytes());
    let bytes = h.as_bytes();
    u64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

/// 向量近邻命中。key 经 [`memory_vector_key`] 由调用方比对还原 memory_id
/// （blake3 单向，索引内不存 id；id ↔ key 映射在查询腿回表时承担，T04 接线）。
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryVectorHit {
    /// usearch key（= `memory_vector_key(memory_id)`）。
    pub key: u64,
    /// 余弦距离（1 − cos_sim，越小越相似；同向量 → 0）。
    pub distance: f32,
}

/// 记忆域 usearch 索引包装：单实例（独立于资产域 VectorStore），dim =
/// [`MEMORY_VECTOR_DIM`]，Cos 距离，F32 量化。线程模型沿 vector.rs:74-80
/// 判例（`RwLock<usearch::Index>`，读写分锁）。
pub struct MemoryVectorIndex {
    index: RwLock<usearch::Index>,
}

impl MemoryVectorIndex {
    /// 打开或新建记忆域向量索引 artifact（`memory_vec.usearch`）。
    ///
    /// 已存在 → `restore`（usearch 自 header 重建 options，含维度/度量）；
    /// 不存在 → `new`（dim 512 / Cos / F32）并落盘空索引。
    ///
    /// # Errors
    /// usearch 初始化/IO 错误 → Fatal。
    pub fn open_or_create(path: impl AsRef<Path>) -> Result<Self, PartisyError> {
        let path = path.as_ref();
        if path.exists() {
            let index =
                usearch::Index::restore(&path.to_string_lossy()).map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("restore memory_vec.usearch: {e}").into()),
                })?;
            return Ok(Self {
                index: RwLock::new(index),
            });
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("create memory_vec artifact dir: {e}").into()),
            })?;
        }
        let index = usearch::Index::new(&Self::index_options()).map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("create memory_vec.usearch: {e}").into()),
        })?;
        index
            .save(&path.to_string_lossy())
            .map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("save memory_vec.usearch: {e}").into()),
            })?;
        Ok(Self {
            index: RwLock::new(index),
        })
    }

    /// 写入一条预计算向量（幂等覆盖：同 memory_id 重写不增条目，P25-b）。
    ///
    /// 嵌入与落库分离（SPEC §2.1）：`vector` 由嵌入调用面（graph-embed 门控，
    /// T04 接线）或探针预计算注入，长度必须 == [`MEMORY_VECTOR_DIM`]。
    ///
    /// # Errors
    /// 维度不匹配 / usearch 错误 → Fatal。
    pub fn upsert(&self, memory_id: &str, vector: &[f32]) -> Result<(), PartisyError> {
        Self::upsert_on(&mut *self.lock_for_write()?, memory_id, vector)
    }

    fn upsert_on(
        index: &mut usearch::Index,
        memory_id: &str,
        vector: &[f32],
    ) -> Result<(), PartisyError> {
        check_dims(vector)?;
        let key = memory_vector_key(memory_id);
        // 幂等覆盖（P25-b：同 id upsert 不增条目）：仅已存在时先摘除。
        // usearch HNSW remove 代价随规模超线性（M5-WP05 T02 基准发现，
        // 判例 vector.rs:243-246）；fresh key 直接 add（写入伴生主路径 =
        // 新 memory），覆盖路径保留 remove 语义。
        if index.contains(key) {
            let _ = index.remove(key);
        }
        // usearch 2.26 容量先行：满则按 2×（至少 1024）扩容（判例 vector.rs:250-258）
        if index.size() >= index.capacity() {
            index
                .reserve(std::cmp::max(index.capacity() * 2, 1024))
                .map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("usearch reserve: {e}").into()),
                })?;
        }
        index.add(key, vector).map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("usearch add: {e}").into()),
        })?;
        Ok(())
    }

    /// 摘除一条向量（生命周期腿原语，P25-c：tombstone/GC 后不可召回）。
    ///
    /// 返回 `true` = 确有条目被摘除；`false` = 该 memory_id 本就不在索引
    /// （幂等——生命周期腿重复触发/向量 artifact 重建后重放均安全）。
    ///
    /// # Errors
    /// usearch 错误 → Fatal。
    pub fn remove(&self, memory_id: &str) -> Result<bool, PartisyError> {
        let index = self.lock_for_write()?;
        let key = memory_vector_key(memory_id);
        if !index.contains(key) {
            return Ok(false);
        }
        index.remove(key).map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("usearch remove: {e}").into()),
        })?;
        Ok(true)
    }

    /// 该 memory_id 的向量是否在索引（测试与生命周期腿判定位）。
    ///
    /// # Errors
    /// 锁中毒 → Fatal。
    pub fn contains(&self, memory_id: &str) -> Result<bool, PartisyError> {
        Ok(self.lock_for_read()?.contains(memory_vector_key(memory_id)))
    }

    /// 向量近邻检索（查询腿原语）：`query` 长度必须 == [`MEMORY_VECTOR_DIM`]。
    ///
    /// # Errors
    /// 维度不匹配 / usearch 错误 → Fatal。
    pub fn search(
        &self,
        query: &[f32],
        limit: usize,
    ) -> Result<Vec<MemoryVectorHit>, PartisyError> {
        check_dims(query)?;
        let index = self.lock_for_read()?;
        let matches = index.search(query, limit).map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("usearch search: {e}").into()),
        })?;
        Ok(matches
            .keys
            .into_iter()
            .zip(matches.distances)
            .map(|(key, distance)| MemoryVectorHit { key, distance })
            .collect())
    }

    /// 索引条目数（P25-b 幂等断言面 + 回填器对账口径）。
    #[must_use]
    pub fn size(&self) -> u64 {
        self.index.read().map(|i| i.size() as u64).unwrap_or(0)
    }

    /// 保存索引 artifact（嵌入伴生批量后落盘；宿主决定落盘节奏，T04 接线）。
    ///
    /// # Errors
    /// usearch save / IO 错误 → Fatal。
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), PartisyError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("create memory_vec artifact dir: {e}").into()),
            })?;
        }
        self.lock_for_read()?
            .save(&path.to_string_lossy())
            .map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("save memory_vec.usearch: {e}").into()),
            })?;
        Ok(())
    }

    fn index_options() -> usearch::IndexOptions {
        usearch::IndexOptions {
            dimensions: MEMORY_VECTOR_DIM,
            metric: usearch::MetricKind::Cos,
            quantization: usearch::ScalarKind::F32,
            ..Default::default()
        }
    }

    fn lock_for_write(
        &self,
    ) -> Result<std::sync::RwLockWriteGuard<'_, usearch::Index>, PartisyError> {
        self.index.write().map_err(|_| PartisyError {
            severity: Severity::Fatal,
            source: Some("memory_vec index lock poison".into()),
        })
    }

    fn lock_for_read(
        &self,
    ) -> Result<std::sync::RwLockReadGuard<'_, usearch::Index>, PartisyError> {
        self.index.read().map_err(|_| PartisyError {
            severity: Severity::Fatal,
            source: Some("memory_vec index lock poison".into()),
        })
    }
}

/// 维度契约校验（SPEC §2.2：≠512 拒绝——fail-fast；查询期 fail-open 由
/// 调用方 T04 查询腿承担，本原语层恒严格）。
fn check_dims(vector: &[f32]) -> Result<(), PartisyError> {
    if vector.len() != MEMORY_VECTOR_DIM {
        return Err(PartisyError {
            severity: Severity::Fatal,
            source: Some(
                format!(
                    "memory vector dimension mismatch: expected {MEMORY_VECTOR_DIM}, got {}",
                    vector.len()
                )
                .into(),
            ),
        });
    }
    Ok(())
}

/// RRF 融合（Reciprocal Rank Fusion，SPEC M11-WP06 §2.5）：输入 = 两通道
/// 各自按序（最优在前）的 memory_id 榜；每个 id 得分 += 1/(k + rank)
/// （k = [`RRF_K`] = 60，公式与 hybrid.rs:85-117 判例一致）；输出按融合分
/// 降序。**并列裁决 = memory_id 字典序升序**（较判例 hybrid.rs:109 的
/// Equal 透传更严：无模型探针可稳定断言；判例原样透传依赖 HashMap 迭代序）。
pub fn rrf_fuse(bm25_ranked: &[String], vector_ranked: &[String]) -> Vec<(String, f32)> {
    let mut scores: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
    for ranked in [bm25_ranked, vector_ranked] {
        for (rank, id) in ranked.iter().enumerate() {
            *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (RRF_K + rank as f32);
        }
    }
    let mut sorted: Vec<_> = scores.into_iter().collect();
    sorted.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rrf_formula_matches_hybrid_precedent() {
        // 单通道：rank r 得 1/(60+r)（hybrid.rs:94 判例公式）
        let only_bm25 = rrf_fuse(&["a".to_string(), "b".to_string()], &[]);
        assert_eq!(only_bm25.len(), 2);
        assert_eq!(only_bm25[0], ("a".to_string(), 1.0 / 60.0));
        assert_eq!(only_bm25[1], ("b".to_string(), 1.0 / 61.0));

        // 双通道交叠：两通道分相加（两通道均 rank 0 ⇒ 1/60 + 1/60）
        let fused = rrf_fuse(
            &["overlap".to_string(), "bm25_only".to_string()],
            &["overlap".to_string()],
        );
        assert_eq!(fused[0].0, "overlap");
        assert!((fused[0].1 - (1.0 / 60.0 + 1.0 / 60.0)).abs() < 1e-6);
    }

    #[test]
    fn rrf_tie_break_deterministic_by_id() {
        // 同分并列（各在己通道同 rank）→ memory_id 字典序升序（确定性）
        let fused = rrf_fuse(&["zz".to_string()], &["aa".to_string()]);
        assert_eq!(fused.len(), 2);
        assert_eq!(fused[0].0, "aa");
        assert_eq!(fused[1].0, "zz");
        assert!((fused[0].1 - fused[1].1).abs() < 1e-6);
    }

    #[test]
    fn key_derivation_matches_blake3_first8_be() {
        // 派生式 = blake3(memory_id) 前 8 字节大端 u64（判例 vector.rs:22-28 同式）
        let h = blake3::hash(b"m1");
        let expect = u64::from_be_bytes([
            h.as_bytes()[0],
            h.as_bytes()[1],
            h.as_bytes()[2],
            h.as_bytes()[3],
            h.as_bytes()[4],
            h.as_bytes()[5],
            h.as_bytes()[6],
            h.as_bytes()[7],
        ]);
        assert_eq!(memory_vector_key("m1"), expect);
    }
}
