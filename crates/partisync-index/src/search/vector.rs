//! usearch HNSW 向量索引（SPEC M4-WP02 §3）
//!
//! 向量来源：
//! - 文本稠密向量：BGE-M3 768d（`stage=embed, artifact=fs:<id>/embed_text_dense.bin`）
//! - 图像稠密向量：CLIP 512d（`stage=embed, artifact=fs:<id>/embed_image_dense.bin`）
//!
//! 索引路径：`~/.partisync/vectors.usearch`（本地），Hub 分片路由归 M4+。
//!
//! usearch 2.26 API 口径（源码核实）：
//! - key 为 `u64`（`usearch::Key`）——content_id 经 blake3 前 8 字节映射为 u64；
//!   碰撞空间 2⁶⁴，条目规模 ≤10⁹ 时碰撞概率 < 2.7%（生日界），可接受且在
//!   upsert 路径以「remove + add」保证幂等覆盖
//! - `MetricKind::Cos` 返回余弦**距离**（1 − cos_sim）：完全相同向量 → 距离 0
//! - `ScalarKind::F32` 量化避免默认 BF16 的精度损失（KPI 口径：F32 存储层）

use std::path::Path;
use std::sync::RwLock;

use partisync_core::error::{PartisyError, Severity};

/// content_id → u64 key（blake3 前 8 字节，大端）。
fn content_key(content_id: &str) -> u64 {
    let h = blake3::hash(content_id.as_bytes());
    let bytes = h.as_bytes();
    u64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

/// 向量种类（SPEC §3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorKind {
    /// 文本稠密向量（BGE-M3，768d）。
    TextDense,
    /// 图像稠密向量（CLIP，512d）。
    ImageDense,
}

impl VectorKind {
    /// 返回 usearch 维度和度量。
    pub fn dims_and_metric(self) -> (usize, usearch::MetricKind) {
        match self {
            VectorKind::TextDense => (768, usearch::MetricKind::Cos),
            VectorKind::ImageDense => (512, usearch::MetricKind::Cos),
        }
    }

    /// 产物文件名（不含路径）。
    pub fn artifact_filename(self) -> &'static str {
        match self {
            VectorKind::TextDense => "embed_text_dense.bin",
            VectorKind::ImageDense => "embed_image_dense.bin",
        }
    }
}

fn index_options(dims: usize, metric: usearch::MetricKind) -> usearch::IndexOptions {
    usearch::IndexOptions {
        dimensions: dims,
        metric,
        quantization: usearch::ScalarKind::F32,
        ..Default::default()
    }
}

/// usearch 索引持有器：每个 `VectorKind` 一个 `usearch::Index` 实例。
pub struct VectorStore {
    text_dense: RwLock<usearch::Index>,
    image_dense: RwLock<usearch::Index>,
}

impl VectorStore {
    /// 打开或新建向量索引。
    ///
    /// # Errors
    /// usearch 初始化错误 → Fatal。
    pub fn open_or_create(path: impl AsRef<Path>) -> Result<Self, PartisyError> {
        let path = path.as_ref();
        std::fs::create_dir_all(path).map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("create vector index dir: {e}").into()),
        })?;

        let text_path = path.join("text_dense.usearch");
        let image_path = path.join("image_dense.usearch");

        let (text_dims, text_metric) = VectorKind::TextDense.dims_and_metric();
        let (image_dims, image_metric) = VectorKind::ImageDense.dims_and_metric();

        let text_dense = Self::open_or_build_index(&text_path, text_dims, text_metric)?;
        let image_dense = Self::open_or_build_index(&image_path, image_dims, image_metric)?;

        Ok(Self {
            text_dense: RwLock::new(text_dense),
            image_dense: RwLock::new(image_dense),
        })
    }

    fn open_or_build_index(
        path: &Path,
        dims: usize,
        metric: usearch::MetricKind,
    ) -> Result<usearch::Index, PartisyError> {
        if path.exists() {
            // restore：读 header 重建 options（含维度/度量）再 load
            usearch::Index::restore(&path.to_string_lossy()).map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("open usearch index: {e}").into()),
            })
        } else {
            let index =
                usearch::Index::new(&index_options(dims, metric)).map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("create usearch index: {e}").into()),
                })?;
            index
                .save(&path.to_string_lossy())
                .map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("save usearch index: {e}").into()),
                })?;
            Ok(index)
        }
    }

    /// 写入一条向量（幂等：同 content_id + 同 kind 覆盖）。
    ///
    /// `vector` 长度必须与 `kind.dims()` 一致。
    ///
    /// # Errors
    /// 维度不匹配 / usearch 错误 → Fatal。
    pub fn upsert(
        &self,
        content_id: &str,
        kind: VectorKind,
        vector: &[f32],
    ) -> Result<(), PartisyError> {
        let mut index = self.lock_for_write(kind)?;
        Self::upsert_on(&mut index, content_id, kind, vector)
    }

    /// 预分配容量（bulk-load 前调用；避免 2× 扩容路径反复重排）。
    ///
    /// # Errors
    /// usearch reserve 错误 → Fatal。
    pub fn reserve(&self, kind: VectorKind, capacity: u64) -> Result<(), PartisyError> {
        let index = self.lock_for_write(kind)?;
        index.reserve(capacity as usize).map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("usearch reserve: {e}").into()),
        })
    }

    /// 新键快速写入（bulk-load 口径）：跳过 contains/remove 成员检查——
    /// usearch `contains`/`remove` 均随规模超线性（实测 10⁴ 规模
    /// ~30ms/次，10⁶ 写入不可用，M5-WP05-T02 基准发现）。键新鲜性由
    /// 调用方保证（索引管道 = fresh content_id 主路径）；同 key 重写
    /// 走 [`Self::upsert`]（幂等覆盖语义）。
    ///
    /// # Errors
    /// 维度不匹配 / usearch 错误 → Fatal。
    pub fn add_new(
        &self,
        content_id: &str,
        kind: VectorKind,
        vector: &[f32],
    ) -> Result<(), PartisyError> {
        let mut index = self.lock_for_write(kind)?;
        Self::add_new_on(&mut index, content_id, kind, vector)
    }

    fn add_new_on(
        index: &mut usearch::Index,
        content_id: &str,
        kind: VectorKind,
        vector: &[f32],
    ) -> Result<(), PartisyError> {
        let (expected_dims, _) = kind.dims_and_metric();
        if vector.len() != expected_dims {
            return Err(PartisyError {
                severity: Severity::Fatal,
                source: Some(
                    format!(
                        "vector dimension mismatch for {kind:?}: expected {expected_dims}, got {}",
                        vector.len()
                    )
                    .into(),
                ),
            });
        }
        let key = content_key(content_id);
        // usearch 2.26 要求容量先行（"Reserve capacity ahead of insertions!"）
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

    fn upsert_on(
        index: &mut usearch::Index,
        content_id: &str,
        kind: VectorKind,
        vector: &[f32],
    ) -> Result<(), PartisyError> {
        let (expected_dims, _) = kind.dims_and_metric();
        if vector.len() != expected_dims {
            return Err(PartisyError {
                severity: Severity::Fatal,
                source: Some(
                    format!(
                        "vector dimension mismatch for {kind:?}: expected {expected_dims}, got {}",
                        vector.len()
                    )
                    .into(),
                ),
            });
        }
        let key = content_key(content_id);
        // 幂等覆盖：仅已存在时先摘除。usearch HNSW remove 代价随规模
        // 超线性（实测 10⁴ 规模 ~30ms/次，10⁶ 写入不可用——M5-WP05 T02
        // 基准发现）；fresh key 直接 add（多数写入场景），覆盖路径保留
        // remove 语义。
        if index.contains(key) {
            let _ = index.remove(key);
        }
        // usearch 2.26 要求容量先行（"Reserve capacity ahead of insertions!"）：
        // 满则按 2×（至少 1024）扩容
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

    /// 批量写入向量。
    ///
    /// # Errors
    /// 任意写入错误 → Fatal。
    pub fn upsert_batch(
        &self,
        items: &[(String, VectorKind, Vec<f32>)],
    ) -> Result<(), PartisyError> {
        for (cid, kind, vec) in items {
            self.upsert(cid, *kind, vec)?;
        }
        Ok(())
    }

    /// 保存全部子索引到磁盘。
    ///
    /// # Errors
    /// save 错误 → Fatal。
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), PartisyError> {
        let path = path.as_ref();
        std::fs::create_dir_all(path).map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("create vector save dir: {e}").into()),
        })?;

        let text_path = path.join("text_dense.usearch");
        let image_path = path.join("image_dense.usearch");

        {
            let index = self.text_dense.read().map_err(|_| PartisyError {
                severity: Severity::Fatal,
                source: Some("text_dense lock poison".into()),
            })?;
            index
                .save(&text_path.to_string_lossy())
                .map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("save text_dense: {e}").into()),
                })?;
        }
        {
            let index = self.image_dense.read().map_err(|_| PartisyError {
                severity: Severity::Fatal,
                source: Some("image_dense lock poison".into()),
            })?;
            index
                .save(&image_path.to_string_lossy())
                .map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("save image_dense: {e}").into()),
                })?;
        }
        Ok(())
    }

    /// 向量检索。
    ///
    /// # Errors
    /// usearch 错误 → Fatal。
    pub fn search(
        &self,
        query: &[f32],
        kind: VectorKind,
        limit: usize,
    ) -> Result<Vec<VectorHit>, PartisyError> {
        let index = self.lock_for_read(kind)?;
        let (expected_dims, _) = kind.dims_and_metric();
        if query.len() != expected_dims {
            return Err(PartisyError {
                severity: Severity::Fatal,
                source: Some(
                    format!(
                        "query dimension mismatch for {kind:?}: expected {expected_dims}, got {}",
                        query.len()
                    )
                    .into(),
                ),
            });
        }

        let matches = index.search(query, limit).map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("usearch search: {e}").into()),
        })?;
        Ok(matches
            .keys
            .into_iter()
            .zip(matches.distances)
            .map(|(key, distance)| VectorHit {
                // u64 key 无法反解 content_id（blake3 单向）——返回 key 的
                // 十六进制形式；调用方经 content_key(cid) 比对还原。
                content_id: format!("{key:016x}"),
                score: distance,
            })
            .collect())
    }

    /// 向量检索（返回原始 key，供 key↔content_id 映射层使用）。
    ///
    /// # Errors
    /// usearch 错误 → Fatal。
    pub fn search_keys(
        &self,
        query: &[f32],
        kind: VectorKind,
        limit: usize,
    ) -> Result<Vec<(u64, f32)>, PartisyError> {
        let index = self.lock_for_read(kind)?;
        let matches = index.search(query, limit).map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("usearch search: {e}").into()),
        })?;
        Ok(matches.keys.into_iter().zip(matches.distances).collect())
    }

    /// 清空向量索引（用于 rebuild）：重建两个空索引。
    ///
    /// # Errors
    /// usearch 初始化错误 → Fatal。
    pub fn clear(&self) -> Result<(), PartisyError> {
        let (text_dims, text_metric) = VectorKind::TextDense.dims_and_metric();
        let (image_dims, image_metric) = VectorKind::ImageDense.dims_and_metric();

        let new_text =
            usearch::Index::new(&index_options(text_dims, text_metric)).map_err(|e| {
                PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("recreate text index: {e}").into()),
                }
            })?;
        let new_image =
            usearch::Index::new(&index_options(image_dims, image_metric)).map_err(|e| {
                PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("recreate image index: {e}").into()),
                }
            })?;

        *self.text_dense.write().map_err(|_| PartisyError {
            severity: Severity::Fatal,
            source: Some("text_dense lock poison".into()),
        })? = new_text;
        *self.image_dense.write().map_err(|_| PartisyError {
            severity: Severity::Fatal,
            source: Some("image_dense lock poison".into()),
        })? = new_image;

        Ok(())
    }

    /// 返回各向量子索引的计数。
    #[must_use]
    pub fn approx_count(&self) -> (u64, u64) {
        let text = self.text_dense.read().map(|i| i.size() as u64).unwrap_or(0);
        let image = self
            .image_dense
            .read()
            .map(|i| i.size() as u64)
            .unwrap_or(0);
        (text, image)
    }

    fn lock_for_write(
        &self,
        kind: VectorKind,
    ) -> Result<std::sync::RwLockWriteGuard<'_, usearch::Index>, PartisyError> {
        match kind {
            VectorKind::TextDense => self.text_dense.write().map_err(|_| PartisyError {
                severity: Severity::Fatal,
                source: Some("text_dense lock poison".into()),
            }),
            VectorKind::ImageDense => self.image_dense.write().map_err(|_| PartisyError {
                severity: Severity::Fatal,
                source: Some("image_dense lock poison".into()),
            }),
        }
    }

    fn lock_for_read(
        &self,
        kind: VectorKind,
    ) -> Result<std::sync::RwLockReadGuard<'_, usearch::Index>, PartisyError> {
        match kind {
            VectorKind::TextDense => self.text_dense.read().map_err(|_| PartisyError {
                severity: Severity::Fatal,
                source: Some("text_dense lock poison".into()),
            }),
            VectorKind::ImageDense => self.image_dense.read().map_err(|_| PartisyError {
                severity: Severity::Fatal,
                source: Some("image_dense lock poison".into()),
            }),
        }
    }
}

/// 向量检索结果。
#[derive(Debug, Clone)]
pub struct VectorHit {
    /// usearch key 的 16 进制形式（u64 key 单向映射，反解经调用方比对）。
    pub content_id: String,
    /// 余弦距离（1 − cos_sim；越小越相似）。
    pub score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_basic() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::open_or_create(dir.path()).unwrap();

        // 文本向量：768d
        let text_vec: Vec<f32> = (0..768).map(|i| (i as f32) * 0.01).collect();
        store
            .upsert("c1", VectorKind::TextDense, &text_vec)
            .unwrap();

        // 图像向量：512d
        let image_vec: Vec<f32> = (0..512).map(|i| (i as f32) * 0.02).collect();
        store
            .upsert("c2", VectorKind::ImageDense, &image_vec)
            .unwrap();

        // 搜索：相同向量余弦距离 = 0（MetricKind::Cos 返回距离）
        let results = store.search(&text_vec, VectorKind::TextDense, 5).unwrap();
        assert_eq!(results.len(), 1);
        assert!(
            results[0].score.abs() < 1e-4,
            "cos distance: {}",
            results[0].score
        );

        // upsert 幂等：同 content_id 重写不产生重复
        store
            .upsert("c1", VectorKind::TextDense, &text_vec)
            .unwrap();
        let (text_n, _) = store.approx_count();
        assert_eq!(text_n, 1);

        // search_keys 返回原始 u64 key，可经 content_key 比对还原
        let keyed = store
            .search_keys(&text_vec, VectorKind::TextDense, 5)
            .unwrap();
        assert_eq!(keyed[0].0, content_key("c1"));

        // 维度错误
        let bad_vec = vec![0.0f32; 100];
        assert!(store.upsert("c3", VectorKind::TextDense, &bad_vec).is_err());
        assert!(store.search(&bad_vec, VectorKind::TextDense, 5).is_err());
    }

    #[test]
    fn content_key_stable() {
        assert_eq!(content_key("c1"), content_key("c1"));
        assert_ne!(content_key("c1"), content_key("c2"));
    }
}
