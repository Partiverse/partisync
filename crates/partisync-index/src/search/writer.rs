//! 索引写入器（SPEC M4-WP02 §1/§5）
//!
//! 负责：
//! 1. 从 `sidecar_items` + `entries` 聚合可检索文档
//! 2. 调用 BM25 + 向量 upsert
//! 3. 增量 upsert 语义（幂等，updated_ns 标记版本）
//! 4. rebuild（全量从 sidecar_items 重读）

use std::sync::Arc;

use partisync_core::error::{PartisyError, Severity};

use super::bm25::{Bm25Index, IndexedDoc};
use super::vector::{VectorKind, VectorStore};

/// 索引写入器门面（SPEC §5）。
///
/// 持有 BM25 + 向量索引，负责从数据库聚合文档并写入。
pub struct IndexWriter {
    bm25: Arc<Bm25Index>,
    vector: Arc<VectorStore>,
}

impl IndexWriter {
    /// 新建。
    #[must_use]
    pub fn new(bm25: Arc<Bm25Index>, vector: Arc<VectorStore>) -> Self {
        Self { bm25, vector }
    }

    /// 从 `sidecar_items` 表聚合 content_ids 并 upsert。
    ///
    /// `sidecar_store` 负责查询 `sidecar_items`；
    /// `graph_store` 负责查询 `entries`（filename/tags）。
    ///
    /// # Errors
    /// 数据库查询 / 索引写入错误 → Fatal。
    pub async fn upsert_content(
        &self,
        content_id: &str,
        sidecar_store: &dyn SidecarReader,
        graph_store: &dyn GraphReader,
    ) -> Result<(), IndexError> {
        // 1. 读取 sidecar_items（OCR / transcript / embed 产物）
        let items = sidecar_store
            .get_items(content_id)
            .await
            .map_err(IndexError::Sidecar)?;

        let ocr_text = items
            .iter()
            .find(|it| it.stage == "ocr" && it.status == 2) // done
            .and_then(|it| it.detail.clone());

        let transcript_text = items
            .iter()
            .find(|it| it.stage == "transcribe" && it.status == 2)
            .and_then(|it| it.detail.clone());

        // embed 产物路径（向量文件）
        let embed_artifact = items
            .iter()
            .find(|it| it.stage == "embed" && it.status == 2)
            .and_then(|it| it.artifact.clone());

        // 2. 读取 entries（filename/tags）
        let (filename, tags) = graph_store
            .get_filename_and_tags(content_id)
            .await
            .map_err(IndexError::Graph)?;

        // 3. updated_ns 取当前时间戳（最新 sidecar item 的 updated_ns）
        let updated_ns = items.iter().map(|it| it.updated_ns).max().unwrap_or(0);

        // 4. 写入 BM25
        let doc = IndexedDoc {
            content_id: content_id.to_string(),
            filename,
            tags,
            ocr_text,
            transcript_text,
            updated_ns,
        };
        self.bm25.upsert(doc).map_err(IndexError::Bm25)?;

        // 5. 写入向量索引（如有 embed 产物）
        if let Some(artifact) = embed_artifact {
            // artifact 格式："fs:<content_id>/embed_text_dense.bin"
            if let Some(path_str) = artifact.strip_prefix("fs:") {
                Self::load_and_upsert_vectors(&self.vector, content_id, path_str)?;
            }
        }

        Ok(())
    }

    /// 批量 upsert（减少 commit 次数）。
    ///
    /// # Errors
    /// 任意写入错误 → Fatal。
    pub async fn upsert_batch(
        &self,
        content_ids: &[String],
        sidecar_store: &dyn SidecarReader,
        graph_store: &dyn GraphReader,
    ) -> Result<(), IndexError> {
        let mut docs = Vec::with_capacity(content_ids.len());

        for cid in content_ids {
            let items = sidecar_store
                .get_items(cid)
                .await
                .map_err(IndexError::Sidecar)?;

            let ocr_text = items
                .iter()
                .find(|it| it.stage == "ocr" && it.status == 2)
                .and_then(|it| it.detail.clone());
            let transcript_text = items
                .iter()
                .find(|it| it.stage == "transcribe" && it.status == 2)
                .and_then(|it| it.detail.clone());
            let embed_artifact = items
                .iter()
                .find(|it| it.stage == "embed" && it.status == 2)
                .and_then(|it| it.artifact.clone());
            let updated_ns = items.iter().map(|it| it.updated_ns).max().unwrap_or(0);

            let (filename, tags) = graph_store
                .get_filename_and_tags(cid)
                .await
                .map_err(IndexError::Graph)?;

            docs.push((
                IndexedDoc {
                    content_id: cid.clone(),
                    filename,
                    tags,
                    ocr_text,
                    transcript_text,
                    updated_ns,
                },
                embed_artifact,
            ));
        }

        // 批量 BM25
        let doc_batch: Vec<_> = docs.iter().map(|(d, _)| d.clone()).collect();
        self.bm25
            .upsert_batch(doc_batch)
            .map_err(IndexError::Bm25)?;

        // 批量向量
        let mut vector_items = Vec::new();
        for (doc, artifact) in &docs {
            if let Some(ref art) = artifact {
                if let Some(path_str) = art.strip_prefix("fs:") {
                    if let Ok(vecs) = Self::load_vector_from_artifact(path_str) {
                        for (kind, vec) in vecs {
                            vector_items.push((doc.content_id.clone(), kind, vec));
                        }
                    }
                }
            }
        }
        if !vector_items.is_empty() {
            self.vector
                .upsert_batch(&vector_items)
                .map_err(IndexError::Vector)?;
        }

        // 提交
        self.commit().map_err(IndexError::Bm25)?;
        Ok(())
    }

    /// 提交 BM25 索引。
    ///
    /// # Errors
    /// commit 错误 → Fatal。
    pub fn commit(&self) -> Result<(), PartisyError> {
        self.bm25.commit()
    }

    /// 清空并重建全部索引（rebuild_index）。
    ///
    /// # Errors
    /// clear / commit 错误 → Fatal。
    pub async fn rebuild_index(
        &self,
        sidecar_store: &dyn SidecarReader,
        graph_store: &dyn GraphReader,
    ) -> Result<RebuildReport, IndexError> {
        let start = std::time::Instant::now();

        // 清空
        self.bm25.clear().map_err(IndexError::Bm25)?;
        self.vector.clear().map_err(IndexError::Vector)?;

        // 读取全部有 embed done 的 content_ids
        let content_ids = sidecar_store
            .get_all_content_ids_with_embed_done()
            .await
            .map_err(IndexError::Sidecar)?;

        let total = content_ids.len();
        let batch_size = 1000;

        for chunk in content_ids.chunks(batch_size) {
            self.upsert_batch(chunk, sidecar_store, graph_store).await?;
        }

        let elapsed = start.elapsed().as_millis() as u32;
        Ok(RebuildReport {
            total_docs: total,
            elapsed_ms: elapsed,
        })
    }

    /// 辅助：从 artifact 路径加载向量并 upsert。
    fn load_and_upsert_vectors(
        vector: &VectorStore,
        content_id: &str,
        artifact_path: &str,
    ) -> Result<(), IndexError> {
        let vecs = Self::load_vector_from_artifact(artifact_path)?;
        for (kind, vec) in vecs {
            vector
                .upsert(content_id, kind, &vec)
                .map_err(IndexError::Vector)?;
        }
        Ok(())
    }

    /// 辅助：从 artifact 路径解析并加载向量文件。
    ///
    /// 格式：`fs:<content_id>/embed_text_dense.bin` 或 `fs:<content_id>/embed_image_dense.bin`
    fn load_vector_from_artifact(
        artifact_path: &str,
    ) -> Result<Vec<(VectorKind, Vec<f32>)>, IndexError> {
        // artifact_path 形如 "c123/embed_text_dense.bin"
        let path = std::path::Path::new(artifact_path);
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        let mut vecs = Vec::new();
        let bytes = std::fs::read(path).map_err(|e| {
            IndexError::Vector(PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("read vector file: {e}").into()),
            })
        })?;

        if filename == "embed_text_dense.bin" {
            let vec = Self::parse_f32_vector(&bytes, 768)?;
            vecs.push((VectorKind::TextDense, vec));
        } else if filename == "embed_image_dense.bin" {
            let vec = Self::parse_f32_vector(&bytes, 512)?;
            vecs.push((VectorKind::ImageDense, vec));
        }
        // ignore 未知文件
        Ok(vecs)
    }

    fn parse_f32_vector(bytes: &[u8], expected_len: usize) -> Result<Vec<f32>, IndexError> {
        let count = bytes.len() / 4;
        if count != expected_len {
            return Err(IndexError::Vector(PartisyError {
                severity: Severity::Fatal,
                source: Some(
                    format!(
                        "vector length mismatch: expected {} floats, got {} (file size {} bytes)",
                        expected_len,
                        count,
                        bytes.len()
                    )
                    .into(),
                ),
            }));
        }
        Ok(bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect())
    }
}

/// 重建报告。
#[derive(Debug, Clone)]
pub struct RebuildReport {
    pub total_docs: usize,
    pub elapsed_ms: u32,
}

/// 索引进场错误（SPEC §5 裁定 3）。
#[derive(Debug)]
pub enum IndexError {
    Sidecar(PartisyError),
    Graph(PartisyError),
    Bm25(PartisyError),
    Vector(PartisyError),
}

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexError::Sidecar(e) => write!(f, "sidecar: {e}"),
            IndexError::Graph(e) => write!(f, "graph: {e}"),
            IndexError::Bm25(e) => write!(f, "bm25: {e}"),
            IndexError::Vector(e) => write!(f, "vector: {e}"),
        }
    }
}

impl std::error::Error for IndexError {}

// ─── Sidecar 数据读取接口（供 IndexWriter 使用）───────────────────────────

/// Sidecar 产物读取接口（`partisync-ai` 的 SidecarStore + 直接 SQL 两种实现）。
#[async_trait::async_trait]
pub trait SidecarReader: Send + Sync {
    async fn get_items(&self, content_id: &str) -> Result<Vec<SidecarItemView>, PartisyError>;
    async fn get_all_content_ids_with_embed_done(&self) -> Result<Vec<String>, PartisyError>;
}

/// `sidecar_items` 行视图（与 `partisync-ai::SidecarItemRow` 相同，
/// 避免引入 ai crate 依赖）。
#[derive(Debug, Clone)]
pub struct SidecarItemView {
    pub content_id: String,
    pub stage: String,
    pub status: i64,
    pub detail: Option<String>,
    pub artifact: Option<String>,
    pub updated_ns: i64,
}

/// Graph 数据读取接口。
#[async_trait::async_trait]
pub trait GraphReader: Send + Sync {
    async fn get_filename_and_tags(
        &self,
        content_id: &str,
    ) -> Result<(String, Vec<String>), PartisyError>;
}

/// SQL-sidecar 读取实现（使用 sqlx 直接查 graph + sidecar_items）。
pub struct SqlSidecarReader {
    pool: sqlx::Pool<sqlx::Sqlite>,
}

impl SqlSidecarReader {
    #[allow(dead_code)]
    pub fn new(pool: sqlx::Pool<sqlx::Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl SidecarReader for SqlSidecarReader {
    async fn get_items(&self, content_id: &str) -> Result<Vec<SidecarItemView>, PartisyError> {
        let rows = sqlx::query(
            "SELECT content_id, stage, status, detail, artifact, updated_ns \
             FROM sidecar_items WHERE content_id = ?",
        )
        .bind(content_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("sidecar get_items: {e}").into()),
        })?;

        Ok(rows
            .iter()
            .map(|r| SidecarItemView {
                content_id: sqlx::Row::get(r, "content_id"),
                stage: sqlx::Row::get(r, "stage"),
                status: sqlx::Row::get(r, "status"),
                detail: sqlx::Row::get(r, "detail"),
                artifact: sqlx::Row::get(r, "artifact"),
                updated_ns: sqlx::Row::get(r, "updated_ns"),
            })
            .collect())
    }

    async fn get_all_content_ids_with_embed_done(&self) -> Result<Vec<String>, PartisyError> {
        let rows = sqlx::query(
            "SELECT DISTINCT content_id FROM sidecar_items \
             WHERE stage = 'embed' AND status = 2",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("get_all_content_ids_with_embed_done: {e}").into()),
        })?;

        Ok(rows
            .iter()
            .map(|r| sqlx::Row::get(r, "content_id"))
            .collect())
    }
}

/// SQL-graph 读取实现。
pub struct SqlGraphReader {
    pool: sqlx::Pool<sqlx::Sqlite>,
}

impl SqlGraphReader {
    #[allow(dead_code)]
    pub fn new(pool: sqlx::Pool<sqlx::Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl GraphReader for SqlGraphReader {
    async fn get_filename_and_tags(
        &self,
        content_id: &str,
    ) -> Result<(String, Vec<String>), PartisyError> {
        // 取任意一个 entry 的 name 作为 filename
        let filename: Option<String> =
            sqlx::query_scalar("SELECT e.name FROM entry e WHERE e.content_id = ? LIMIT 1")
                .bind(content_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("get filename: {e}").into()),
                })?;

        let filename = filename.unwrap_or_else(|| content_id.to_string());

        // 取 tags（entry_tag 以 entry_path 为身份，经 entry.path 关联）
        let tag_rows = sqlx::query(
            "SELECT t.name FROM tag t \
             JOIN entry_tag et ON et.tag_id = t.id \
             JOIN entry e ON e.path = et.entry_path \
             WHERE e.content_id = ? AND et.deleted = 0 AND t.deleted = 0",
        )
        .bind(content_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("get tags: {e}").into()),
        })?;

        let tags: Vec<String> = tag_rows.iter().map(|r| sqlx::Row::get(r, "name")).collect();

        Ok((filename, tags))
    }
}
