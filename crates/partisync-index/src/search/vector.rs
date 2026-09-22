//! usearch HNSW 向量索引（SPEC M4-WP02 §3）
//!
//! 向量来源：
//! - 文本稠密向量：BGE-M3 768d（`stage=embed, artifact=fs:<id>/embed_text_dense.bin`）
//! - 图像稠密向量：CLIP 512d（`stage=embed, artifact=fs:<id>/embed_image_dense.bin`）
//!
//! 索引路径：`~/.partisync/vectors.usearch`（本地），Hub 分片路由归 M4+。

use std::path::Path;
use std::sync::RwLock;

use partisync_core::error::{PartisyError, Severity};

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
            VectorKind::TextDense => (768, usearch::MetricKind::Cosine),
            VectorKind::ImageDense => (512, usearch::MetricKind::Cosine),
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
        let path_str = path.to_string_lossy();
        if path.exists() {
            usearch::Index::open(&path_str).map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("open usearch index: {e}").into()),
            })
        } else {
            let index = usearch::Index::new(
                &usearch::Options::default()
                    .with_dims(dims)
                    .with_metric(metric)
                    .with_expansion_add(0) // 写入时控制
                    .with_expansion_search(0),
            );
            index.save(&path_str).map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("create usearch index: {e}").into()),
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
        let index = match kind {
            VectorKind::TextDense => self.text_dense.write().map_err(|_| PartisyError {
                severity: Severity::Fatal,
                source: Some("text_dense lock poison".into()),
            })?,
            VectorKind::ImageDense => self.image_dense.write().map_err(|_| PartisyError {
                severity: Severity::Fatal,
                source: Some("image_dense lock poison".into()),
            })?,
        };
        let (expected_dims, _) = kind.dims_and_metric();
        if vector.len() != expected_dims {
            return Err(PartisyError {
                severity: Severity::Fatal,
                source: Some(
                    format!(
                        "vector dimension mismatch for {:?}: expected {}, got {}",
                        kind,
                        expected_dims,
                        vector.len()
                    )
                    .into(),
                ),
            });
        }
        // usearch 使用字符串 key，content_id 作为唯一标识
        index.add(content_id, vector).map_err(|e| PartisyError {
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
        // text_dense 和 image_dense 分开处理
        let mut text_items = Vec::new();
        let mut image_items = Vec::new();
        for (cid, kind, vec) in items {
            match kind {
                VectorKind::TextDense => text_items.push((cid.clone(), vec.clone())),
                VectorKind::ImageDense => image_items.push((cid.clone(), vec.clone())),
            }
        }

        if !text_items.is_empty() {
            let mut index = self.text_dense.write().map_err(|_| PartisyError {
                severity: Severity::Fatal,
                source: Some("text_dense lock poison".into()),
            })?;
            for (cid, vec) in text_items {
                index.add(&cid, &vec).map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("usearch add text: {e}").into()),
                })?;
            }
        }
        if !image_items.is_empty() {
            let mut index = self.image_dense.write().map_err(|_| PartisyError {
                severity: Severity::Fatal,
                source: Some("image_dense lock poison".into()),
            })?;
            for (cid, vec) in image_items {
                index.add(&cid, &vec).map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("usearch add image: {e}").into()),
                })?;
            }
        }
        Ok(())
    }

    /// 保存全部待写入到磁盘。
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
        let index = match kind {
            VectorKind::TextDense => self.text_dense.read().map_err(|_| PartisyError {
                severity: Severity::Fatal,
                source: Some("text_dense lock poison".into()),
            })?,
            VectorKind::ImageDense => self.image_dense.read().map_err(|_| PartisyError {
                severity: Severity::Fatal,
                source: Some("image_dense lock poison".into()),
            })?,
        };
        let (expected_dims, _) = kind.dims_and_metric();
        if query.len() != expected_dims {
            return Err(PartisyError {
                severity: Severity::Fatal,
                source: Some(
                    format!(
                        "query dimension mismatch for {:?}: expected {}, got {}",
                        kind,
                        expected_dims,
                        query.len()
                    )
                    .into(),
                ),
            });
        }

        let results = index.search(query, limit);
        Ok(results
            .into_iter()
            .map(|r| VectorHit {
                content_id: r.key.to_string(),
                score: r.distance,
            })
            .collect())
    }

    /// 清空向量索引（用于 rebuild）。
    pub fn clear(&self) -> Result<(), PartisyError> {
        // usearch Index 没有 clear_all，需要重新创建
        let path = std::env::temp_dir().join("usearch_temp_clear");
        let (text_dims, text_metric) = VectorKind::TextDense.dims_and_metric();
        let (image_dims, image_metric) = VectorKind::ImageDense.dims_and_metric();

        let new_text = usearch::Index::new(
            &usearch::Options::default()
                .with_dims(text_dims)
                .with_metric(text_metric),
        );
        let new_image = usearch::Index::new(
            &usearch::Options::default()
                .with_dims(image_dims)
                .with_metric(image_metric),
        );

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

    /// 返回各向量子索引的近似计数。
    #[must_use]
    pub fn approx_count(&self) -> (u64, u64) {
        let text = self.text_dense.read().map(|i| i.size()).unwrap_or(0);
        let image = self.image_dense.read().map(|i| i.size()).unwrap_or(0);
        (text, image)
    }
}

/// 向量检索结果。
#[derive(Debug, Clone)]
pub struct VectorHit {
    pub content_id: String,
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

        // 搜索：同 content 同 kind 相似度最高
        let results = store.search(&text_vec, VectorKind::TextDense, 5).unwrap();
        assert_eq!(results[0].content_id, "c1");
        assert!((results[0].score - 1.0).abs() < 1e-5); // 完全相同向量，余弦相似度 = 1

        // 维度错误
        let bad_vec = vec![0.0f32; 100];
        assert!(store.upsert("c3", VectorKind::TextDense, &bad_vec).is_err());
        assert!(store.search(&bad_vec, VectorKind::TextDense, 5).is_err());
    }
}
