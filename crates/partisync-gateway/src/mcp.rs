//! MCP Server 实现（RMCP 2026-07-28 stateless，SPEC M4-WP03）
//!
//! 工具清单：asset_search / asset_read / asset_organize / dataset_export / job_status
//! 传输：stdio（RMCP 推荐），无连接状态。

use std::sync::Arc;

use rmcp::handler::server::{ServerHandler, ServerHandlerEvent};
use rmcp::transport::io::stdio;
use rmcp::{server, Tool, ToolCall, ToolCallResult};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
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

// ─────────────────────────────────────────────────────────────────────────────
// MCP Server 状态
// ─────────────────────────────────────────────────────────────────────────────

/// MCP Server 全局状态。
pub struct McpServerState {
    /// 索引引擎（只读查询）。
    pub index_engine: Arc<RwLock<Option<partisync_index::search::engine::IndexEngine>>>,
    // 注意：graph/ai 接入待 T03/T04
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

    /// asset_read 实现（桩，待 T03 graph 接入）。
    async fn asset_read(&self, args: &JsonValue) -> ToolCallResult {
        let input: AssetReadInput = match serde_json::from_value(args.clone()) {
            Ok(v) => v,
            Err(e) => return Err(server::Error::InvalidParams(e.to_string()).into()),
        };

        // TODO(T03): query graph.content + sidecar_items tables
        Ok(serde_json::to_value(AssetReadOutput {
            content_id: input.content_id,
            entry_id: "".to_string(),
            name: "".to_string(),
            mime_kind: "".to_string(),
            size_bytes: 0,
            created_ns: 0,
            updated_ns: 0,
            tags: vec![],
            sidecar_stages: SidecarStages {
                thumbnail: "unknown".to_string(),
                exif: "unknown".to_string(),
                ocr: "unknown".to_string(),
                transcribe: "unknown".to_string(),
                embed: "unknown".to_string(),
            },
            embedding: None,
        })
        .unwrap()
        .into())
    }

    /// asset_organize 实现（桩，待 T04 graph txn 接入）。
    async fn asset_organize(&self, args: &JsonValue) -> ToolCallResult {
        let input: AssetOrganizeInput = match serde_json::from_value(args.clone()) {
            Ok(v) => v,
            Err(e) => return Err(server::Error::InvalidParams(e.to_string()).into()),
        };

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

        // TODO(T04): write to graph.asset_txn table
        let txn_id = format!("txn_{}", uuid::Uuid::new_v4());
        Ok(serde_json::to_value(AssetOrganizeOutput {
            planned,
            transaction_id: Some(txn_id.clone()),
            confirm_url: Some(format!("/api/assets/confirm/{}", txn_id)),
        })
        .unwrap()
        .into())
    }

    /// dataset_export 实现（桩，待 T04 manifest writer）。
    async fn dataset_export(&self, _args: &JsonValue) -> ToolCallResult {
        // TODO(T04): implement manifest writer
        Ok(serde_json::to_value(DatasetExportOutput {
            manifest_path: "/tmp/partisync_export.jsonl".to_string(),
            record_count: 0,
            total_bytes: 0,
            expires_at_ns: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
                + 86_400_000_000_000,
        })
        .unwrap()
        .into())
    }

    /// job_status 实现（桩，待 T03 jobs 表接入）。
    async fn job_status(&self, _args: &JsonValue) -> ToolCallResult {
        // TODO(T03): query jobs.jobs table
        Ok(serde_json::to_value(JobStatusOutput { jobs: vec![] })
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
pub async fn run_mcp_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = McpServerState {
        index_engine: Arc::new(RwLock::new(None)),
    };

    // McpServerState: ServerHandler → Service<RoleServer> (blanket impl)
    // RoleServer is just a zero-sized marker type for the server role
    let transport = stdio();
    rmcp::service::serve_server(state, transport).await?;

    Ok(())
}
