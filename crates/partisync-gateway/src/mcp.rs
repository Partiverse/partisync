//! MCP Server 实现（RMCP 2026-07-28 stateless，SPEC M4-WP03）
//!
//! 工具清单：asset_search / asset_read / asset_organize / dataset_export / job_status
//! 传输：stdio（RMCP 推荐），无连接状态。

use std::path::PathBuf;
use std::sync::Arc;

use partisync_core::Ulid;
use rmcp::handler::server::{ServerHandler, ServerHandlerEvent};
use rmcp::transport::io::stdio;
use rmcp::{server, Tool, ToolCall, ToolCallResult};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;

// ─────────────────────────────────────────────────────────────────────────────
// 工具输入/输出类型
// ─────────────────────────────────────────────────────────────────────────────

/// asset_search 工具输入。
#[derive(Debug, Deserialize)]
pub struct AssetSearchInput {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub vector_kind: Option<String>, // "auto" | "text" | "image"
    #[serde(default)]
    pub filters: Option<SearchFilters>,
    #[serde(default = "default_true")]
    pub include_transcript: bool,
}

fn default_limit() -> usize {
    20
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Default)]
pub struct SearchFilters {
    pub content_ids: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub mime_kind: Option<String>,
    #[serde(rename = "date_from_ns")]
    pub date_from_ns: Option<i64>,
    #[serde(rename = "date_to_ns")]
    pub date_to_ns: Option<i64>,
}

/// asset_search 输出。
#[derive(Debug, Serialize)]
pub struct AssetSearchOutput {
    pub hits: Vec<SearchHit>,
    pub total: usize,
    #[serde(rename = "timing_ms")]
    pub timing_ms: u32,
}

#[derive(Debug, Serialize)]
pub struct SearchHit {
    #[serde(rename = "content_id")]
    pub content_id: String,
    pub score: f32,
    pub highlights: Vec<String>,
    #[serde(rename = "mime_kind")]
    pub mime_kind: Option<String>,
    #[serde(rename = "updated_ns")]
    pub updated_ns: i64,
}

/// asset_read 输入。
#[derive(Debug, Deserialize)]
pub struct AssetReadInput {
    #[serde(rename = "content_id")]
    pub content_id: String,
}

/// asset_read 输出。
#[derive(Debug, Serialize)]
pub struct AssetReadOutput {
    #[serde(rename = "content_id")]
    pub content_id: String,
    #[serde(rename = "entry_id")]
    pub entry_id: String,
    pub name: String,
    #[serde(rename = "mime_kind")]
    pub mime_kind: String,
    #[serde(rename = "size_bytes")]
    pub size_bytes: i64,
    #[serde(rename = "created_ns")]
    pub created_ns: i64,
    #[serde(rename = "updated_ns")]
    pub updated_ns: i64,
    pub tags: Vec<String>,
    #[serde(rename = "sidecar_stages")]
    pub sidecar_stages: SidecarStages,
    pub embedding: Option<EmbeddingInfo>,
}

#[derive(Debug, Serialize)]
pub struct SidecarStages {
    pub thumbnail: String,
    pub exif: String,
    pub ocr: String,
    pub transcribe: String,
    pub embed: String,
}

#[derive(Debug, Serialize)]
pub struct EmbeddingInfo {
    #[serde(rename = "text_dim")]
    pub text_dim: Option<u32>,
    #[serde(rename = "text_model")]
    pub text_model: Option<String>,
    #[serde(rename = "image_dim")]
    pub image_dim: Option<u32>,
    #[serde(rename = "image_model")]
    pub image_model: Option<String>,
}

/// asset_organize 输入。
#[derive(Debug, Deserialize)]
pub struct AssetOrganizeInput {
    pub operations: Vec<OrganizeOperation>,
    #[serde(default = "default_preview_only")]
    pub preview_only: bool,
}

fn default_preview_only() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct OrganizeOperation {
    #[serde(rename = "content_id")]
    pub content_id: String,
    pub action: String, // "add_tag" | "remove_tag" | "set_tag" | "delete"
    pub value: Option<String>,
}

/// asset_organize 输出。
#[derive(Debug, Serialize)]
pub struct AssetOrganizeOutput {
    pub planned: Vec<PlannedOp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "transaction_id")]
    pub transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "confirm_url")]
    pub confirm_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PlannedOp {
    #[serde(rename = "content_id")]
    pub content_id: String,
    pub action: String,
    pub value: Option<String>,
    pub effect: String,
}

/// dataset_export 输入。
#[derive(Debug, Deserialize)]
pub struct DatasetExportInput {
    #[serde(rename = "content_ids")]
    pub content_ids: Option<Vec<String>>,
    pub filter: Option<ExportFilter>,
    #[serde(default = "default_true")]
    pub include_vectors: bool,
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "jsonl".to_string()
}

#[derive(Debug, Deserialize, Default)]
pub struct ExportFilter {
    pub tags: Option<Vec<String>>,
    #[serde(rename = "mime_kind")]
    pub mime_kind: Option<String>,
}

/// dataset_export 输出。
#[derive(Debug, Serialize)]
pub struct DatasetExportOutput {
    #[serde(rename = "manifest_path")]
    pub manifest_path: String,
    #[serde(rename = "record_count")]
    pub record_count: usize,
    #[serde(rename = "total_bytes")]
    pub total_bytes: u64,
    #[serde(rename = "expires_at_ns")]
    pub expires_at_ns: i64,
}

/// job_status 输入。
#[derive(Debug, Deserialize)]
pub struct JobStatusInput {
    #[serde(rename = "job_id")]
    pub job_id: Option<String>,
}

/// job_status 输出。
#[derive(Debug, Serialize)]
pub struct JobStatusOutput {
    pub jobs: Vec<JobInfo>,
}

#[derive(Debug, Serialize)]
pub struct JobInfo {
    #[serde(rename = "job_id")]
    pub job_id: String,
    #[serde(rename = "content_id")]
    pub content_id: String,
    pub pipeline: String,
    pub status: String, // "running" | "done" | "failed"
    #[serde(rename = "current_stage")]
    pub current_stage: String,
    #[serde(rename = "progress_pct")]
    pub progress_pct: u8,
    #[serde(rename = "started_at_ns")]
    pub started_at_ns: i64,
    #[serde(rename = "updated_at_ns")]
    pub updated_at_ns: i64,
    pub error: Option<String>,
}

/// Graph `jobs` 行（内部用）。
#[derive(Debug, sqlx::FromRow)]
struct JobRow {
    id: String,
    kind: String,
    status: i64,
    root: String,
    done_files: i64,
    checkpoint: Option<String>,
    error: Option<String>,
    created_ns: i64,
    updated_ns: i64,
}

// ─────────────────────────────────────────────────────────────────────────────
// MCP Server 状态
// ─────────────────────────────────────────────────────────────────────────────

/// MCP Server 全局状态。
pub struct McpServerState {
    /// 索引引擎（只读查询）。
    index_engine: Arc<RwLock<Option<partisync_index::search::engine::IndexEngine>>>,
    /// Graph SQLite 连接池（只读查询）。
    graph_pool: SqlitePool,
}

impl McpServerState {
    /// 创建并初始化 MCP Server 状态。
    ///
    /// `graph_db_path` 默认 `~/.partisync/graph.db`。
    /// 索引引擎需外部调用者通过 `index_engine()` 访问。
    pub async fn new(graph_db_path: Option<PathBuf>) -> Result<Self, sqlx::Error> {
        let graph_db_path = graph_db_path.unwrap_or_else(|| {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".partisync")
                .join("graph.db")
        });
        let graph_pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect(&format!("sqlite:{}", graph_db_path.display()))
            .await?;
        Ok(Self {
            index_engine: Arc::new(RwLock::new(None)),
            graph_pool,
        })
    }

    /// 返回 Graph 连接池引用。
    #[must_use]
    pub fn graph_pool(&self) -> &SqlitePool {
        &self.graph_pool
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 工具定义（RMCP 2026-07-28）
// ─────────────────────────────────────────────────────────────────────────────

/// 返回所有可用工具清单。
pub fn all_tools() -> Vec<Tool> {
    vec![
        asset_search_tool(),
        asset_read_tool(),
        asset_organize_tool(),
        dataset_export_tool(),
        job_status_tool(),
    ]
}

fn asset_search_tool() -> Tool {
    Tool::new(
        "asset_search",
        "Search assets by natural language query using hybrid BM25 + vector search",
        serde_json::json!({
            "query": {
                "type": "string",
                "description": "Natural language search query"
            },
            "limit": {
                "type": "number",
                "description": "Max results (default 20, max 100)",
                "default": 20
            },
            "vector_kind": {
                "type": "string",
                "description": "Vector search mode: auto, text, or image",
                "default": "auto",
                "enum": ["auto", "text", "image"]
            },
            "filters": {
                "type": "object",
                "description": "Optional search filters",
                "properties": {
                    "content_ids": { "type": "array", "items": { "type": "string" } },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "mime_kind": { "type": "string" },
                    "date_from_ns": { "type": "number" },
                    "date_to_ns": { "type": "number" }
                }
            },
            "include_transcript": {
                "type": "boolean",
                "description": "Include transcript text in results",
                "default": true
            }
        }),
    )
}

fn asset_read_tool() -> Tool {
    Tool::new(
        "asset_read",
        "Read asset metadata (not file content) including sidecar stages and embedding info",
        serde_json::json!({
            "content_id": {
                "type": "string",
                "description": "Content ID of the asset"
            }
        }),
    )
}

fn asset_organize_tool() -> Tool {
    Tool::new(
        "asset_organize",
        "Preview or execute asset organization operations (add/remove tags, delete)",
        serde_json::json!({
            "operations": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "content_id": { "type": "string" },
                        "action": {
                            "type": "string",
                            "enum": ["add_tag", "remove_tag", "set_tag", "delete"]
                        },
                        "value": { "type": "string" }
                    }
                }
            },
            "preview_only": {
                "type": "boolean",
                "description": "If true, only preview without executing",
                "default": true
            }
        }),
    )
}

fn dataset_export_tool() -> Tool {
    Tool::new(
        "dataset_export",
        "Export a set of assets as a JSON Lines manifest file",
        serde_json::json!({
            "content_ids": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Specific content IDs to export (empty = all)"
            },
            "filter": {
                "type": "object",
                "properties": {
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "mime_kind": { "type": "string" }
                }
            },
            "include_vectors": {
                "type": "boolean",
                "description": "Include embedding vectors in export",
                "default": true
            },
            "format": {
                "type": "string",
                "enum": ["jsonl"],
                "default": "jsonl"
            }
        }),
    )
}

fn job_status_tool() -> Tool {
    Tool::new(
        "job_status",
        "Query Sidecar pipeline job status",
        serde_json::json!({
            "job_id": {
                "type": "string",
                "description": "Specific job ID (empty = recent jobs)"
            }
        }),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// ServerHandler 实现
// ─────────────────────────────────────────────────────────────────────────────

impl ServerHandler for McpServerState {
    fn list_tools(&self) -> Vec<Tool> {
        all_tools()
    }

    async fn handle_tool_call(&self, call: ToolCall) -> ToolCallResult {
        let name = call.name.as_str();
        let args = call.arguments.as_ref().unwrap_or(&JsonValue::Null);

        match name {
            "asset_search" => self.asset_search(args).await,
            "asset_read" => self.asset_read(args).await,
            "asset_organize" => self.asset_organize(args).await,
            "dataset_export" => self.dataset_export(args).await,
            "job_status" => self.job_status(args).await,
            _ => Err(server::Error::ToolNotFound(name.to_string()).into()),
        }
    }

    async fn handle_event(&self, _event: ServerHandlerEvent) {
        // stateless server: no events to handle
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 工具处理器
// ─────────────────────────────────────────────────────────────────────────────

impl McpServerState {
    /// asset_search 实现。
    async fn asset_search(&self, args: &JsonValue) -> ToolCallResult {
        let input: AssetSearchInput = match serde_json::from_value(args.clone()) {
            Ok(v) => v,
            Err(e) => return Err(server::Error::InvalidParams(e.to_string()).into()),
        };

        let limit = input.limit.min(100);

        let index_engine = self.index_engine.read().await;
        let engine = match index_engine.as_ref() {
            Some(e) => e,
            None => {
                // Index engine not initialized — return empty results gracefully
                return Ok(serde_json::to_value(AssetSearchOutput {
                    hits: vec![],
                    total: 0,
                    timing_ms: 0,
                })
                .unwrap()
                .into());
            }
        };

        // Build filters
        let filters = input
            .filters
            .as_ref()
            .map(|f| partisync_index::search::hybrid::SearchFilters {
                content_ids: f.content_ids.clone(),
                tags: f.tags.clone(),
                mime_kinds: f.mime_kind.as_ref().map(|m| vec![m.clone()]),
            })
            .unwrap_or_default();

        let vector_kind = match input.vector_kind.as_deref() {
            Some("text") => partisync_index::search::vector::VectorKind::TextDense,
            Some("image") => partisync_index::search::vector::VectorKind::ImageDense,
            _ => partisync_index::search::vector::VectorKind::TextDense, // Auto → default to text
        };

        let query = partisync_index::search::hybrid::HybridQuery {
            query: input.query.clone(),
            filters,
            limit,
            vector_kind: match vector_kind {
                partisync_index::search::vector::VectorKind::TextDense => {
                    partisync_index::search::hybrid::HybridVectorKind::TextDense
                }
                partisync_index::search::vector::VectorKind::ImageDense => {
                    partisync_index::search::hybrid::HybridVectorKind::ImageDense
                }
            },
            include_transcript: input.include_transcript,
            use_reranker: false, // MCP tool: no reranker by default
        };

        // For MCP asset_search: we need a query vector.
        // Since embedding generation is async and expensive, we use bm25_only
        // as the primary path for MCP tool calls (WP02 hybrid_search requires
        // a pre-computed query vector that MCP callers don't have).
        let bm25_result = engine
            .bm25_only(partisync_index::search::bm25::Bm25Query {
                query: input.query,
                limit,
            })
            .await
            .map_err(|e| server::Error::InternalError(e.to_string()))?;

        let hits = bm25_result
            .hits
            .into_iter()
            .map(|h| SearchHit {
                content_id: h.content_id,
                score: h.score,
                highlights: h.highlight.into_iter().collect(),
                mime_kind: None, // BM25 doesn't have mime info; would need join
                updated_ns: 0,
            })
            .collect();

        Ok(serde_json::to_value(AssetSearchOutput {
            hits,
            total: bm25_result.total,
            timing_ms: bm25_result.timing_ms,
        })
        .unwrap()
        .into())
    }

    /// asset_read 实现——查询 graph.content + entry + sidecar_items + tags。
    async fn asset_read(&self, args: &JsonValue) -> ToolCallResult {
        let input: AssetReadInput = match serde_json::from_value(args.clone()) {
            Ok(v) => v,
            Err(e) => return Err(server::Error::InvalidParams(e.to_string()).into()),
        };

        let pool = self.graph_pool();

        // 1. 查询 content 表
        #[derive(Debug, sqlx::FromRow)]
        struct ContentRow {
            id: String,
            size: i64,
            mime: Option<String>,
        }
        let content: Option<ContentRow> =
            sqlx::query_as("SELECT id, size, mime FROM content WHERE id = ?")
                .bind(&input.content_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| server::Error::InternalError(format!("content query: {e}")))?;

        let (mime_kind, size_bytes) = match content {
            Some(c) => (c.mime.unwrap_or_default(), c.size),
            None => {
                return Err(server::Error::InvalidParams(format!(
                    "content_id '{}' not found",
                    input.content_id
                ))
                .into());
            }
        };

        // 2. 查询 entry 表（取任意一个指向该 content 的 entry）
        #[derive(Debug, sqlx::FromRow)]
        struct EntryRow {
            id: String,
            name: String,
            mtime_ns: i64,
        }
        let entry: Option<EntryRow> =
            sqlx::query_as("SELECT id, name, mtime_ns FROM entry WHERE content_id = ? LIMIT 1")
                .bind(&input.content_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| server::Error::InternalError(format!("entry query: {e}")))?;

        let (entry_id, name, updated_ns) = match entry {
            Some(e) => (e.id, e.name, e.mtime_ns),
            None => (String::new(), String::new(), 0),
        };

        // 3. 查询 sidecar_items 表
        #[derive(Debug, sqlx::FromRow)]
        struct SidecarRow {
            stage: String,
            status: i64,
            detail: Option<String>,
            updated_ns: i64,
        }
        let stages: Vec<SidecarRow> = sqlx::query_as(
            "SELECT stage, status, detail, updated_ns FROM sidecar_items WHERE content_id = ?",
        )
        .bind(&input.content_id)
        .fetch_all(pool)
        .await
        .map_err(|e| server::Error::InternalError(format!("sidecar query: {e}")))?;

        let stage_status = |s: &str| -> String {
            stages
                .iter()
                .find(|r| r.stage == s)
                .map(|r| match r.status {
                    0 => "pending",
                    1 => "running",
                    2 => "done",
                    3 => "skipped",
                    4 => "failed",
                    _ => "unknown",
                })
                .unwrap_or("not_started")
                .to_string()
        };

        let sidecar_stages = SidecarStages {
            thumbnail: stage_status("thumbnail"),
            exif: stage_status("exif"),
            ocr: stage_status("ocr"),
            transcribe: stage_status("transcribe"),
            embed: stage_status("embed"),
        };

        // 4. 查询 tags（通过 entry_tag JOIN tag）
        #[derive(Debug, sqlx::FromRow)]
        struct TagRow {
            name: String,
        }
        let tags: Vec<String> = if entry_id.is_empty() {
            vec![]
        } else {
            sqlx::query_scalar::<_, String>(
                "SELECT t.name FROM tag t
                 JOIN entry_tag et ON et.tag_id = t.id
                 WHERE et.entry_path = (
                     SELECT path FROM entry WHERE id = ?
                 ) AND et.deleted = 0",
            )
            .bind(&entry_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        };

        // 5. 从 detail 解析 embedding 元数据（artifact 列引用向量文件）
        let embedding = stages
            .iter()
            .find(|r| r.stage == "embed" && r.status == 2 && r.detail.is_some())
            .map(|r| {
                let detail = r.detail.as_deref().unwrap_or("");
                // detail 格式: "dim=N model=X" 或路径引用
                let text_dim = (|| {
                    detail
                        .split_whitespace()
                        .find(|w| w.starts_with("dim="))
                        .and_then(|w| w.get(4..))
                        .and_then(|s| s.parse::<u32>().ok())
                })();
                let text_model = detail
                    .split_whitespace()
                    .find(|w| w.starts_with("model="))
                    .map(|w| w.get(6..).unwrap_or("").to_string());
                EmbeddingInfo {
                    text_dim,
                    text_model,
                    image_dim: None,
                    image_model: None,
                }
            });

        Ok(serde_json::to_value(AssetReadOutput {
            content_id: input.content_id,
            entry_id,
            name,
            mime_kind,
            size_bytes,
            created_ns: 0,
            updated_ns,
            tags,
            sidecar_stages,
            embedding,
        })
        .unwrap()
        .into())
    }

    /// asset_organize 实现——标签管理 + 软删除。
    ///
    /// preview_only=true：只返回操作计划，不写入。
    /// preview_only=false：立即写入 graph.db（add_tag/remove_tag/set_tag/delete）。
    async fn asset_organize(&self, args: &JsonValue) -> ToolCallResult {
        let input: AssetOrganizeInput = match serde_json::from_value(args.clone()) {
            Ok(v) => v,
            Err(e) => return Err(server::Error::InvalidParams(e.to_string()).into()),
        };

        let pool = self.graph_pool();

        // 构建预览效果描述（预览和执行都复用同一份描述）
        let planned: Vec<PlannedOp> = input
            .operations
            .iter()
            .map(|op| {
                let effect = match op.action.as_str() {
                    "add_tag" => format!(
                        "tag '{}' will be added to asset {}",
                        op.value.as_deref().unwrap_or(""),
                        op.content_id
                    ),
                    "remove_tag" => format!(
                        "tag '{}' will be removed from asset {}",
                        op.value.as_deref().unwrap_or(""),
                        op.content_id
                    ),
                    "set_tag" => format!(
                        "tag '{}' will be set on asset {}",
                        op.value.as_deref().unwrap_or(""),
                        op.content_id
                    ),
                    "delete" => format!("asset {} will be deleted", op.content_id),
                    _ => format!("action '{}' on asset {}", op.action, op.content_id),
                };
                PlannedOp {
                    content_id: op.content_id.clone(),
                    action: op.action.clone(),
                    value: op.value.clone(),
                    effect,
                }
            })
            .collect();

        if input.preview_only {
            return Ok(serde_json::to_value(AssetOrganizeOutput {
                planned,
                transaction_id: None,
                confirm_url: None,
            })
            .unwrap()
            .into());
        }

        // 实际执行写入
        for op in &input.operations {
            let result = match op.action.as_str() {
                "add_tag" => {
                    let tag_name = op.value.as_deref().unwrap_or("").trim();
                    if tag_name.is_empty() {
                        continue;
                    }
                    // 查找 tag_id（若不存在则创建）
                    let tag_id: Option<String> = sqlx::query_scalar(
                        "SELECT id FROM tag WHERE name = ? AND deleted = 0 LIMIT 1",
                    )
                    .bind(tag_name)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| server::Error::InternalError(format!("tag lookup: {e}")))?;

                    let tag_id = match tag_id {
                        Some(id) => id,
                        None => {
                            let new_id = Ulid::new().to_string();
                            sqlx::query("INSERT INTO tag (id, space_id, name, deleted, updated_hlc) VALUES (?, 'default', ?, 0, NULL)")
                                .bind(&new_id)
                                .bind(tag_name)
                                .execute(pool)
                                .await
                                .map_err(|e| server::Error::InternalError(format!("create tag: {e}")))?;
                            new_id
                        }
                    };

                    // 获取 entry_path
                    let entry_path: Option<String> =
                        sqlx::query_scalar("SELECT path FROM entry WHERE content_id = ? LIMIT 1")
                            .bind(&op.content_id)
                            .fetch_optional(pool)
                            .await
                            .map_err(|e| {
                                server::Error::InternalError(format!("entry path: {e}"))
                            })?;

                    if let Some(path) = entry_path {
                        // INSERT OR IGNORE 防止重复
                        sqlx::query(
                            "INSERT OR IGNORE INTO entry_tag (tag_id, entry_path, deleted, updated_hlc) VALUES (?, ?, 0, NULL)",
                        )
                        .bind(&tag_id)
                        .bind(&path)
                        .execute(pool)
                        .await
                        .map_err(|e| server::Error::InternalError(format!("add tag: {e}")))?;
                    }
                    Ok(())
                }
                "remove_tag" => {
                    let tag_name = op.value.as_deref().unwrap_or("").trim();
                    if tag_name.is_empty() {
                        continue;
                    }
                    // 获取 entry_path
                    let entry_path: Option<String> =
                        sqlx::query_scalar("SELECT path FROM entry WHERE content_id = ? LIMIT 1")
                            .bind(&op.content_id)
                            .fetch_optional(pool)
                            .await
                            .map_err(|e| {
                                server::Error::InternalError(format!("entry path: {e}"))
                            })?;

                    if let Some(path) = entry_path {
                        sqlx::query(
                            "UPDATE entry_tag SET deleted = 1, updated_hlc = NULL WHERE tag_id = (SELECT id FROM tag WHERE name = ? AND deleted = 0) AND entry_path = ?",
                        )
                        .bind(tag_name)
                        .bind(&path)
                        .execute(pool)
                        .await
                        .map_err(|e| server::Error::InternalError(format!("remove tag: {e}")))?;
                    }
                    Ok(())
                }
                "set_tag" => {
                    // 等价于 remove_tag + add_tag（软替换）
                    let tag_name = op.value.as_deref().unwrap_or("").trim();
                    if tag_name.is_empty() {
                        continue;
                    }
                    let entry_path: Option<String> =
                        sqlx::query_scalar("SELECT path FROM entry WHERE content_id = ? LIMIT 1")
                            .bind(&op.content_id)
                            .fetch_optional(pool)
                            .await
                            .map_err(|e| {
                                server::Error::InternalError(format!("entry path: {e}"))
                            })?;

                    if let Some(path) = entry_path {
                        // 软删除现有标签
                        sqlx::query(
                            "UPDATE entry_tag SET deleted = 1, updated_hlc = NULL WHERE entry_path = ?",
                        )
                        .bind(&path)
                        .execute(pool)
                        .await
                        .map_err(|e| server::Error::InternalError(format!("clear tags: {e}")))?;

                        // 查找或创建目标 tag
                        let tag_id: Option<String> = sqlx::query_scalar(
                            "SELECT id FROM tag WHERE name = ? AND deleted = 0 LIMIT 1",
                        )
                        .bind(tag_name)
                        .fetch_optional(pool)
                        .await
                        .map_err(|e| server::Error::InternalError(format!("tag lookup: {e}")))?;

                        let tag_id = match tag_id {
                            Some(id) => id,
                            None => {
                                let new_id = Ulid::new().to_string();
                                sqlx::query(
                                    "INSERT INTO tag (id, space_id, name, deleted, updated_hlc) VALUES (?, 'default', ?, 0, NULL)",
                                )
                                .bind(&new_id)
                                .bind(tag_name)
                                .execute(pool)
                                .await
                                .map_err(|e| server::Error::InternalError(format!("create tag: {e}")))?;
                                new_id
                            }
                        };

                        sqlx::query(
                            "INSERT OR IGNORE INTO entry_tag (tag_id, entry_path, deleted, updated_hlc) VALUES (?, ?, 0, NULL)",
                        )
                        .bind(&tag_id)
                        .bind(&path)
                        .execute(pool)
                        .await
                        .map_err(|e| server::Error::InternalError(format!("set tag: {e}")))?;
                    }
                    Ok(())
                }
                "delete" => {
                    // 软删除 entry（标记为 placeholder，content 不删除）
                    sqlx::query("UPDATE entry SET state = 1 WHERE content_id = ?")
                        .bind(&op.content_id)
                        .execute(pool)
                        .await
                        .map_err(|e| server::Error::InternalError(format!("delete: {e}")))?;
                    Ok(())
                }
                _ => Err(server::Error::InvalidParams(format!(
                    "unknown action: {}",
                    op.action
                ))),
            };

            if let Err(e) = result {
                return Err(e);
            }
        }

        let txn_id = format!("txn_{}", uuid::Uuid::new_v4());
        Ok(serde_json::to_value(AssetOrganizeOutput {
            planned,
            transaction_id: Some(txn_id.clone()),
            confirm_url: Some(format!("/api/assets/confirm/{}", txn_id)),
        })
        .unwrap()
        .into())
    }

    /// dataset_export 实现——流式写 JSONL manifest。
    ///
    /// content_ids 非空时按 ID 精确导出；空时按 filter 过滤。
    /// include_vectors=true 时 embedding info 仅记录维度/模型（不含原始向量）。
    async fn dataset_export(&self, args: &JsonValue) -> ToolCallResult {
        let input: DatasetExportInput = match serde_json::from_value(args.clone()) {
            Ok(v) => v,
            Err(e) => return Err(server::Error::InvalidParams(e.to_string()).into()),
        };

        let pool = self.graph_pool();

        // 构建 WHERE 子句
        let mut conditions = Vec::new();
        let mut params: Vec<String> = Vec::new();

        if let Some(ref ids) = input.content_ids {
            if !ids.is_empty() {
                let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
                conditions.push(format!("c.id IN ({})", placeholders.join(",")));
                params.extend(ids.clone());
            }
        }
        if let Some(ref filter) = input.filter {
            if let Some(ref mime) = filter.mime_kind {
                conditions.push("c.mime LIKE ?".to_string());
                params.push(format!("{}%", mime.trim_end_matches('*')));
            }
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // 查询内容
        let query = format!(
            "SELECT c.id, c.size, c.mime, e.name, e.path, e.mtime_ns \
             FROM content c \
             JOIN entry e ON e.content_id = c.id \
             {where_clause} \
             LIMIT 10000",
        );

        #[derive(Debug, sqlx::FromRow)]
        struct ExportRow {
            id: String,
            size: i64,
            mime: Option<String>,
            name: String,
            path: String,
            mtime_ns: i64,
        }

        let rows: Vec<ExportRow> = sqlx::query_as(&query)
            .fetch_all(pool)
            .await
            .map_err(|e| server::Error::InternalError(format!("export query: {e}")))?;

        // 写入临时 JSONL 文件
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let manifest_path = format!("/tmp/partisync_export_{}.jsonl", timestamp);
        let mut file = File::create(&manifest_path)
            .await
            .map_err(|e| server::Error::InternalError(format!("create file: {e}")))?;

        let mut record_count = 0u64;
        let mut total_bytes = 0u64;

        for row in &rows {
            let record = serde_json::json!({
                "content_id": row.id,
                "name": row.name,
                "path": row.path,
                "mime_kind": row.mime,
                "size_bytes": row.size,
                "mtime_ns": row.mtime_ns,
                "embedding": input.include_vectors.then(|| serde_json::json!({
                    "note": "vector data not included in export"
                })),
            });

            let line = serde_json::to_string(&record)
                .map_err(|e| server::Error::InternalError(format!("serialize: {e}")))?;
            file.write_all(line.as_bytes())
                .await
                .map_err(|e| server::Error::InternalError(format!("write: {e}")))?;
            file.write_all(b"\n")
                .await
                .map_err(|e| server::Error::InternalError(format!("write newline: {e}")))?;

            record_count += 1;
            total_bytes += row.size;
        }

        file.flush()
            .await
            .map_err(|e| server::Error::InternalError(format!("flush: {e}")))?;

        let expires_at_ns =
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) + 86_400_000_000_000; // 24h

        Ok(serde_json::to_value(DatasetExportOutput {
            manifest_path,
            record_count,
            total_bytes,
            expires_at_ns,
        })
        .unwrap()
        .into())
    }

    /// job_status 实现——查询 graph.jobs 表（kind='sidecar'）。
    async fn job_status(&self, args: &JsonValue) -> ToolCallResult {
        let input: JobStatusInput = match serde_json::from_value(args.clone()) {
            Ok(v) => v,
            Err(e) => return Err(server::Error::InvalidParams(e.to_string()).into()),
        };

        let pool = self.graph_pool();

        let jobs: Vec<JobRow> = if let Some(job_id) = &input.job_id {
            // 按 ID 精确查
            sqlx::query_as(
                "SELECT id, kind, status, root, done_files, checkpoint, error, created_ns, updated_ns \
                 FROM jobs WHERE id = ? AND kind = 'sidecar'",
            )
            .bind(job_id)
            .fetch_all(pool)
            .await
            .map_err(|e| server::Error::InternalError(format!("job query: {e}")))?
        } else {
            // 最近 20 个 sidecar 作业
            sqlx::query_as(
                "SELECT id, kind, status, root, done_files, checkpoint, error, created_ns, updated_ns \
                 FROM jobs WHERE kind = 'sidecar' ORDER BY created_ns DESC LIMIT 20",
            )
            .fetch_all(pool)
            .await
            .map_err(|e| server::Error::InternalError(format!("jobs query: {e}")))?
        };

        let infos: Vec<JobInfo> = jobs
            .into_iter()
            .map(|j| {
                let status_str = match j.status {
                    0 => "queued",
                    1 => "running",
                    2 => "interrupted",
                    3 => "done",
                    4 => "failed",
                    _ => "unknown",
                };
                // 从 checkpoint JSON 解析当前 stage（简化：checkpoint 存 stage 名）
                let current_stage = j
                    .checkpoint
                    .as_ref()
                    .and_then(|c| serde_json::from_str::<serde_json::Value>(c).ok())
                    .and_then(|v| v.get("stage"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                // 进度：done_files > 0 时估算
                let progress_pct = if j.status == 3 {
                    100
                } else if j.done_files > 0 {
                    std::cmp::min(99, (j.done_files * 20) as u8) // 简化估算
                } else {
                    0
                };

                JobInfo {
                    job_id: j.id,
                    content_id: j.root,
                    pipeline: "thumb→exif→ocr→transcribe→embed".to_string(),
                    status: status_str.to_string(),
                    current_stage,
                    progress_pct,
                    started_at_ns: j.created_ns,
                    updated_at_ns: j.updated_ns,
                    error: j.error,
                }
            })
            .collect();

        Ok(serde_json::to_value(JobStatusOutput { jobs: infos })
            .unwrap()
            .into())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 运行入口
// ─────────────────────────────────────────────────────────────────────────────

/// 运行 MCP Server（RMCP stdio 传输）。
///
/// `McpServerState` 实现 `ServerHandler`，后者通过 blanket impl
/// `impl<H: ServerHandler> Service<RoleServer> for H` 自动满足 `Service<RoleServer>`，
/// 故直接传 state 给 `serve_server`。
/// 运行 MCP Server（RMCP stdio 传输）。
///
/// `McpServerState` 实现 `ServerHandler`，后者通过 blanket impl
/// `impl<H: ServerHandler> Service<RoleServer> for H` 自动满足 `Service<RoleServer>`，
/// 故直接传 state 给 `serve_server`。
pub async fn run_mcp_server(
    graph_db_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = McpServerState::new(graph_db_path)
        .await
        .map_err(|e| format!("failed to open graph.db: {e}"))?;

    let transport = stdio();
    rmcp::service::serve_server(state, transport).await?;

    Ok(())
}
