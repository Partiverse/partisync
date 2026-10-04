//! MCP Server 实现（RMCP 2026-07-28 stateless，SPEC M4-WP03）
//!
//! 工具清单：asset_search / asset_read / asset_organize / dataset_export / job_status
//!          + memory_write / memory_search / memory_verify（M9-WP02-T04）
//! 传输：stdio（`rmcp::transport::io::stdio()`），无连接状态。
//!
//! 错误口径（rmcp 3.4.0 `call_tool` 契约，见 `ServerHandler::call_tool` 文档）：
//! - 参数不合法 / 未知工具 → `Err(ErrorData)`（协议错误）
//! - 工具执行失败但调用有效 → `Ok(CallToolResult::error(...))`（调用方可见）

use std::path::PathBuf;
use std::sync::Arc;

use partisync_core::Ulid;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ErrorData,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::transport::io::stdio;
use rmcp::RoleServer;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;

// ─────────────────────────────────────────────────────────────────────────────
// 工具输入/输出类型（serde，与 SPEC M4-WP03 §工具规格 一致）
// ─────────────────────────────────────────────────────────────────────────────

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
fn default_false() -> bool {
    false
}

/// asset_organize 单事务批大小上限（M4-WP99-T02，DoS 加固）。
///
/// 100 ops × ~50µs/op ≈ 5ms 提交延迟预算；超过则拒收（`invalid_params`）。
/// 仍需更大批量的 caller 应改用循环分批。
const MAX_ORGANIZE_OPS: usize = 100;

#[derive(Debug, Deserialize, Default)]
pub struct SearchFilters {
    pub content_ids: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub mime_kind: Option<String>,
    pub date_from_ns: Option<i64>,
    pub date_to_ns: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AssetSearchOutput {
    pub hits: Vec<SearchHit>,
    pub total: usize,
    pub timing_ms: u32,
}

#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub content_id: String,
    pub score: f32,
    pub highlights: Vec<String>,
    pub mime_kind: Option<String>,
    pub updated_ns: i64,
}

#[derive(Debug, Deserialize)]
pub struct AssetReadInput {
    pub content_id: String,
}

#[derive(Debug, Serialize)]
pub struct AssetReadOutput {
    pub content_id: String,
    pub entry_id: String,
    pub name: String,
    pub mime_kind: String,
    pub size_bytes: i64,
    pub created_ns: i64,
    pub updated_ns: i64,
    pub tags: Vec<String>,
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
    /// M4-WP05：管线第六 stage。
    /// M4-WP99-T04 API 一致性加固：c2pa 字段对 absent 序列化 `null`（与"stage 缺席"语义一致）；
    /// 其他 stage 维持 `String`（默认 not_started，因为 WP01 管线确保它们总是存在至少一行）。
    /// 原因：absent stage 应当字段缺失或 null，「"not_started" 字符串」会诱导调用方误判为
    /// 已入队但未开始（实际根本没入队）。
    pub c2pa: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EmbeddingInfo {
    pub text_dim: Option<u32>,
    pub text_model: Option<String>,
    pub image_dim: Option<u32>,
    pub image_model: Option<String>,
}

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
    pub content_id: String,
    pub action: String, // "add_tag" | "remove_tag" | "set_tag" | "delete"
    pub value: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AssetOrganizeOutput {
    pub planned: Vec<PlannedOp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirm_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PlannedOp {
    pub content_id: String,
    pub action: String,
    pub value: Option<String>,
    pub effect: String,
}

#[derive(Debug, Deserialize)]
pub struct DatasetExportInput {
    pub content_ids: Option<Vec<String>>,
    pub filter: Option<ExportFilter>,
    /// 是否在导出 manifest 中包含向量字段（BGE-M3 dense / CLIP image / SPLADE sparse）。
    /// **默认 false**：向量是潜在的 PII / 指纹面（可逆推出原内容），须显式 opt-in 才导出。
    /// 当前实现（`dataset_export`）实际未生成向量字段——但 schema 默认 true 易诱导调用方
    /// 假设向量已导出，构成**隐式数据外泄承诺**。修复（M4-WP99-T07）：改默认 false + 显式 opt-in，
    /// 并加测试覆盖「默认不返回任何 vector 字段」（详见 `tests/pen_test.rs` dataset_export 相关）。
    #[serde(default = "default_false")]
    pub include_vectors: bool,
    #[serde(default = "default_format")]
    pub format: String,
    /// 每分片记录数；0/缺省 = 单文件。
    #[serde(default)]
    pub shard_size: Option<usize>,
    /// 导出根目录；缺省 = 系统临时目录下 partisync_export。
    #[serde(default)]
    pub output_dir: Option<String>,
}

fn default_format() -> String {
    "jsonl".to_string()
}

#[derive(Debug, Deserialize, Default)]
pub struct ExportFilter {
    pub tags: Option<Vec<String>>,
    pub mime_kind: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DatasetExportOutput {
    /// 单文件模式：manifest 路径；分片模式：导出根目录。
    pub manifest_path: String,
    pub record_count: u64,
    pub total_bytes: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shards: Vec<ShardInfo>,
    pub expires_at_ns: i64,
}

#[derive(Debug, Serialize)]
pub struct ShardInfo {
    pub path: String,
    pub record_count: u64,
}

#[derive(Debug, Deserialize)]
pub struct JobStatusInput {
    pub job_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JobStatusOutput {
    pub jobs: Vec<JobInfo>,
}

#[derive(Debug, Serialize)]
pub struct JobInfo {
    pub job_id: String,
    pub content_id: String,
    pub pipeline: String,
    pub status: String,
    pub current_stage: String,
    pub progress_pct: u8,
    pub started_at_ns: i64,
    pub updated_at_ns: i64,
    pub error: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Graph 行类型（内部）
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct ContentRow {
    #[allow(dead_code)]
    id: String,
    size: i64,
    mime: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct EntryNameRow {
    id: String,
    name: String,
    path: String,
    mtime_ns: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct SidecarRow {
    stage: String,
    status: i64,
    detail: Option<String>,
    #[allow(dead_code)]
    updated_ns: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct JobRow {
    id: String,
    #[allow(dead_code)]
    kind: String,
    status: i64,
    root: String,
    done_files: i64,
    checkpoint: Option<String>,
    error: Option<String>,
    created_ns: i64,
    updated_ns: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct ExportRow {
    id: String,
    size: i64,
    mime: Option<String>,
    name: String,
    path: String,
    mtime_ns: i64,
}

// ─────────────────────────────────────────────────────────────────────────────
// MCP Server 状态
// ─────────────────────────────────────────────────────────────────────────────

/// MCP Server 全局状态。
pub struct McpServerState {
    /// 索引引擎（只读查询；未注入时 asset_search 返回空结果）。
    index_engine: RwLock<Option<Arc<partisync_index::search::engine::IndexEngine>>>,
    /// Graph SQLite 连接池（读写）。
    graph_pool: SqlitePool,
    /// memory 工具面 Store（M9-WP02-T04；与 graph_pool 同源共享，懒初始化）。
    memory_store: tokio::sync::OnceCell<Arc<partisync_graph::Store>>,
    /// WASM 扩展注册表（M7-WP01-T04 第 3 步；`None` = 未装载）。
    ext_registry: crate::ext::SharedRegistry,
}

impl McpServerState {
    /// 创建并初始化状态（`graph_db_path` 默认 `~/.partisync/graph.db`）。
    ///
    /// 首启即用契约（M7-WP01-T04 热修，此前首启必败 sqlite code 14）：
    /// db 文件缺失时 `mode=rwc` 自动建库、父目录缺失时逐级创建——
    /// sqlite 本身两者都不会做（无父目录 / 只读打开语义），而
    /// 桌面壳侧车与 CLI 的默认路径在首次运行前都不存在。
    pub async fn new(graph_db_path: Option<PathBuf>) -> Result<Self, sqlx::Error> {
        let graph_db_path = graph_db_path.unwrap_or_else(|| {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".partisync")
                .join("graph.db")
        });
        if let Some(parent) = graph_db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect(&format!("sqlite:{}?mode=rwc", graph_db_path.display()))
            .await?;
        Ok(Self::from_pool(pool))
    }

    /// 直接从 pool 构造（内存库测试路径）。
    #[must_use]
    pub fn from_pool(pool: SqlitePool) -> Self {
        Self {
            index_engine: RwLock::new(None),
            graph_pool: pool.clone(),
            memory_store: tokio::sync::OnceCell::new(),
            ext_registry: Arc::new(RwLock::new(None)),
        }
    }

    /// memory 工具面 Store（懒初始化：迁移幂等重跑；首用时建）。
    async fn graph_store(&self) -> Result<&Arc<partisync_graph::Store>, ErrorData> {
        self.memory_store
            .get_or_try_init(|| async {
                partisync_graph::Store::from_pool(self.graph_pool.clone())
                    .await
                    .map(Arc::new)
            })
            .await
            .map_err(|e| ErrorData::internal_error(format!("graph store 初始化失败: {e}"), None))
    }

    /// 注入索引引擎（WP02 IndexEngine）。
    pub async fn install_index_engine(&self, engine: partisync_index::search::engine::IndexEngine) {
        *self.index_engine.write().await = Some(Arc::new(engine));
    }

    /// 注入扩展注册表（M7-WP01-T04 第 3 步）。装载失败由调用方降级
    /// （stderr 告警 + 不注入），扩展目录损坏不拖垮 MCP 主服务。
    pub async fn install_ext_registry(
        &self,
        registry: partisync_ext_host::ExtRegistry,
        scan_dir: PathBuf,
    ) {
        *self.ext_registry.write().await = Some((Arc::new(registry), scan_dir));
    }

    /// 返回 Graph 连接池引用。
    #[must_use]
    pub fn graph_pool(&self) -> &SqlitePool {
        &self.graph_pool
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 工具 schema
// ─────────────────────────────────────────────────────────────────────────────

/// `serde_json::Map` 即 rmcp 的 `JsonObject`（model.rs:45）。
fn schema(v: JsonValue) -> Arc<rmcp::model::JsonObject> {
    match v {
        JsonValue::Object(m) => Arc::new(m),
        _ => Arc::new(serde_json::Map::new()),
    }
}

/// 全部工具定义（`tools/list` 载荷）。
fn all_tools() -> Vec<Tool> {
    vec![
        Tool::new(
            "asset_search",
            "Search assets by natural language query using hybrid BM25 + vector search",
            schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Natural language search query"},
                    "limit": {"type": "number", "default": 20, "description": "Max results (default 20, max 100)"},
                    "vector_kind": {"type": "string", "enum": ["auto", "text", "image"], "default": "auto"},
                    "filters": {"type": "object"},
                    "include_transcript": {"type": "boolean", "default": true}
                },
                "required": ["query"]
            })),
        ),
        Tool::new(
            "asset_read",
            "Read asset metadata (not file content) including sidecar stages and embedding info",
            schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "content_id": {"type": "string", "description": "Content ID of the asset"}
                },
                "required": ["content_id"]
            })),
        ),
        Tool::new(
            "asset_organize",
            "Preview or execute asset organization operations (add/remove tags, soft-delete)",
            schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "operations": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "content_id": {"type": "string"},
                                "action": {"type": "string", "enum": ["add_tag", "remove_tag", "set_tag", "delete"]},
                                "value": {"type": "string"}
                            },
                            "required": ["content_id", "action"]
                        }
                    },
                    "preview_only": {"type": "boolean", "default": true}
                },
                "required": ["operations"]
            })),
        ),
        Tool::new(
            "dataset_export",
            "Export a set of assets as a JSON Lines manifest file (supports sharding)",
            schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "content_ids": {"type": "array", "items": {"type": "string"}},
                    "filter": {"type": "object"},
                    "include_vectors": {"type": "boolean", "default": false},
                    "format": {"type": "string", "enum": ["jsonl"], "default": "jsonl"},
                    "shard_size": {"type": "number", "description": "Records per shard file; 0/absent = single file"},
                    "output_dir": {"type": "string", "description": "Export root directory"}
                }
            })),
        ),
        Tool::new(
            "job_status",
            "Query Sidecar pipeline job status",
            schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "job_id": {"type": "string", "description": "Specific job ID (empty = recent jobs)"}
                }
            })),
        ),
        // M9-WP02-T04：可验证记忆层三工具（SPEC M9-WP02 §2.4；
        // tools/list schema = 对外唯一权威契约）。
        Tool::new(
            "memory_write",
            "Write a verifiable memory record (content-addressed, idempotent; leaf of the RFC 6962-style proof tree)",
            schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {"type": "string", "description": "Memory content (non-empty, max 64KiB)"},
                    "tags": {"type": "array", "items": {"type": "string"}, "description": "Optional tags (max 32 items)"},
                    "metadata": {"type": "object", "description": "Optional JSON object metadata (max 16KiB)"}
                },
                "required": ["content"]
            })),
        ),
        Tool::new(
            "memory_search",
            "Search verifiable memories: SQLite full-text (FTS5 trigram) with LIKE fallback; exact tag filter; exact memory_id lookup",
            schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Full-text query over content"},
                    "tag": {"type": "string", "description": "Exact tag filter"},
                    "memory_id": {"type": "string", "description": "Exact memory lookup (overrides query)"},
                    "limit": {"type": "number", "default": 20, "description": "Max results (default 20)"},
                    "offset": {"type": "number", "default": 0, "description": "Result offset (default 0)"}
                }
            })),
        ),
        Tool::new(
            "memory_verify",
            "Verify the memory commitment: snapshot root vs recomputed root (no id) or single-leaf inclusion proof (with id). Read-only.",
            schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "memory_id": {"type": "string", "description": "Memory ID for single-leaf inclusion proof; omit for whole-store verification"}
                }
            })),
        ),
        // M7-WP01-T04：扩展列举工具（内建）。桌面壳 UI 经 mcp_call("ext_list")
        // 拿到扩展工具清单——零 IPC 扩口（SPEC §2.3 约定）。
        Tool::new(
            "ext_list",
            "List registered WASM extension tools (JSON in / JSON out tools loaded from the extensions directory)",
            schema(serde_json::json!({
                "type": "object",
                "properties": {}
            })),
        ),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// ServerHandler 实现（rmcp 3.4.0 真实签名，源码 handler/server.rs:309 宏展开核实）
// ─────────────────────────────────────────────────────────────────────────────

impl ServerHandler for McpServerState {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "PartiSync asset tools: asset_search / asset_read / asset_organize / \
                 dataset_export / job_status. Verifiable memory tools (M9-WP02): \
                 memory_write / memory_search / memory_verify. All calls are stateless.",
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        // SEP-2549（2026-07-28 spec）：ttlMs/cacheScope 为必填字段，严格 client
        //（ZCode 宿主 zod schema）会对省略值报 invalid_type/invalid_value——
        // rmcp 对 None 做 skip_serializing_if，必须显式带上。
        // M7-WP01-T04：内建工具 + 已注册扩展工具（`ext_` 前缀）合并列举。
        let mut tools = all_tools();
        let registry = self.ext_registry.read().await;
        if let Some((reg, _dir)) = registry.as_ref() {
            for m in reg.manifests() {
                let caps = m
                    .capabilities
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>()
                    .join(",");
                tools.push(Tool::new(
                    format!("ext_{}", m.tool_name),
                    format!("WASM extension tool (capabilities: {})", caps),
                    schema(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "input": {"type": "string", "description": "JSON string passed to the extension"}
                        }
                    })),
                ));
            }
        }
        let result = ListToolsResult::with_all_items(tools)
            .with_ttl_ms(300_000)
            .with_cache_scope(rmcp::model::CacheScope::Private);
        Ok(result)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let args = request
            .arguments
            .map(JsonValue::Object)
            .unwrap_or_else(|| JsonValue::Object(serde_json::Map::new()));

        let result = match request.name.as_ref() {
            "asset_search" => self.asset_search(&args).await,
            "asset_read" => self.asset_read(&args).await,
            "asset_organize" => self.asset_organize(&args).await,
            "dataset_export" => self.dataset_export(&args).await,
            "job_status" => self.job_status(&args).await,
            "memory_write" => self.memory_write(&args).await,
            "memory_search" => self.memory_search(&args).await,
            "memory_verify" => self.memory_verify(&args).await,
            "ext_list" => self.ext_list().await,
            name if name.starts_with("ext_") => self.call_extension(name, &args).await,
            other => {
                return Err(ErrorData::invalid_params(
                    format!("unknown tool: {other}"),
                    None,
                ))
            }
        };
        // CallToolResult → CallToolResponse::Complete
        match result {
            Ok(r) => Ok(CallToolResponse::Complete(r)),
            Err(e) => Err(e),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// memory 工具面（M9-WP02-T04，SPEC M9-WP02 §2.4）
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct MemoryWriteInput {
    content: String,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    metadata: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
struct MemorySearchInput {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    memory_id: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct MemoryVerifyInput {
    #[serde(default)]
    memory_id: Option<String>,
}

impl McpServerState {
    /// `memory_write`：内容寻址幂等写 + 根快照刷新（写入口在 graph
    /// Store：oplog + 行 + 根一次完成）。限界（SPEC §2.4）：content
    /// 非空 ≤64KiB / tags ≤32 项 / metadata object ≤16KiB——超限工具级
    /// 错误，不截断、不静默。
    async fn memory_write(&self, args: &JsonValue) -> Result<CallToolResult, ErrorData> {
        let input: MemoryWriteInput = match serde_json::from_value(args.clone()) {
            Ok(v) => v,
            Err(e) => return Err(ErrorData::invalid_params(e.to_string(), None)),
        };
        if input.content.is_empty() {
            return tool_err("memory content 不能为空");
        }
        if input.content.len() > 64 * 1024 {
            return tool_err(format!(
                "memory content 超限: {} bytes > 64KiB",
                input.content.len()
            ));
        }
        if input.tags.as_ref().is_some_and(|t| t.len() > 32) {
            return tool_err(format!(
                "memory tags 超限: {} 项 > 32",
                input.tags.as_ref().map_or(0, Vec::len)
            ));
        }
        let metadata = input
            .metadata
            .unwrap_or_else(|| JsonValue::Object(serde_json::Map::new()));
        if !metadata.is_object() {
            return tool_err("memory metadata 必须为 JSON object");
        }
        if metadata.to_string().len() > 16 * 1024 {
            return tool_err(format!(
                "memory metadata 超限: {} bytes > 16KiB",
                metadata.to_string().len()
            ));
        }
        let store = self.graph_store().await?;
        let outcome = match store
            .memory_write(
                &input.content,
                input.tags.as_deref().unwrap_or(&[]),
                &metadata,
            )
            .await
        {
            Ok(o) => o,
            Err(e) => return tool_err(format!("memory_write 失败: {e}")),
        };
        let root = match store.memory_root_snapshot().await {
            Ok(s) => s.map(|snap| snap.root),
            Err(e) => return tool_err(format!("memory_write 读根失败: {e}")),
        };
        ok_json(&serde_json::json!({
            "memory_id": outcome.memory_id,
            "deduplicated": outcome.deduplicated,
            "root": root,
        }))
    }

    /// `memory_search`：FTS5 trigram / LIKE 兜底 + tag 精确过滤 + id 精确查。
    async fn memory_search(&self, args: &JsonValue) -> Result<CallToolResult, ErrorData> {
        let input: MemorySearchInput = match serde_json::from_value(args.clone()) {
            Ok(v) => v,
            Err(e) => return Err(ErrorData::invalid_params(e.to_string(), None)),
        };
        let store = self.graph_store().await?;
        let report = store
            .memory_search(
                input.query.as_deref(),
                input.tag.as_deref(),
                input.memory_id.as_deref(),
                input.limit.unwrap_or(20),
                input.offset.unwrap_or(0),
            )
            .await;
        let report = match report {
            Ok(r) => r,
            Err(e) => return tool_err(format!("memory_search 失败: {e}")),
        };
        ok_json(&report)
    }

    /// `memory_verify`（只读）：无 id = 全库快照根 vs 重算根 + 列级校验；
    /// 有 id = 单叶包含证明（自包含验证，不需数据库）。
    async fn memory_verify(&self, args: &JsonValue) -> Result<CallToolResult, ErrorData> {
        let input: MemoryVerifyInput = match serde_json::from_value(args.clone()) {
            Ok(v) => v,
            Err(e) => return Err(ErrorData::invalid_params(e.to_string(), None)),
        };
        let store = self.graph_store().await?;
        match input.memory_id {
            None => {
                let report = match store.verify_memory().await {
                    Ok(r) => r,
                    Err(e) => return tool_err(format!("memory_verify 失败: {e}")),
                };
                ok_json(&serde_json::json!({
                    "root": report.snapshot_root,
                    "memory_count": report.memory_count,
                    "recomputed_root": report.recomputed_root,
                    "ok": report.ok,
                }))
            }
            Some(id) => {
                let rows = match store.memory_rows().await {
                    Ok(r) => r,
                    Err(e) => return tool_err(format!("memory_verify 失败: {e}")),
                };
                match partisync_graph::memory::inclusion_proof(&rows, &id) {
                    Some(proof) => ok_json(&proof),
                    None => tool_err(format!("memory 不存在: {id}")),
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 扩展工具面（M7-WP01-T04 第 3 步）
// ─────────────────────────────────────────────────────────────────────────────

impl McpServerState {
    /// `ext_list`：列已注册扩展工具（名 + capability 声明面）。
    async fn ext_list(&self) -> Result<CallToolResult, ErrorData> {
        let registry = self.ext_registry.read().await;
        let scan_dir = registry.as_ref().map(|(_, d)| d.display().to_string());
        let tools: Vec<serde_json::Value> = match registry.as_ref().map(|(reg, _)| reg) {
            Some(reg) => reg
                .manifests()
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "name": format!("ext_{}", m.tool_name),
                        "capabilities": m.capabilities.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
                    })
                })
                .collect(),
            None => vec![],
        };
        ok_json(&serde_json::json!({
            "tools": tools,
            "count": tools.len(),
            "dir": scan_dir,
        }))
    }

    /// `ext_<name>`：调用扩展工具。
    ///
    /// **超时兜底**（PR #35 审查 P1-1 处置，选项 b+c）：guest 无终止
    /// 保障（同步 wasmtime 调用无 epoch/fuel deadline，无限循环 guest
    /// 会钉死 worker + 永久持锁）——gateway 侧 `spawn_blocking` +
    /// [`EXT_CALL_TIMEOUT`] 包装，超时返回工具级错误 JSON，UI/客户端
    /// 立即恢复；被超时的扩展实例仍持有锁（后续调用超时），不拖垮
    /// 其余工具面。epoch interruption 正解登记 SPEC §6，T05 后立项。
    async fn call_extension(
        &self,
        mcp_name: &str,
        args: &JsonValue,
    ) -> Result<CallToolResult, ErrorData> {
        let tool_name = mcp_name.strip_prefix("ext_").unwrap_or(mcp_name);
        let input = match args.get("input") {
            Some(JsonValue::String(s)) => s.clone(),
            // 顶层对象即入参（桌面壳直传 JSON 对象时自动序列化）
            Some(other) => other.to_string(),
            None => "{}".to_owned(),
        };
        let registry = self.ext_registry.read().await;
        let Some((reg, _dir)) = registry.as_ref() else {
            return Err(ErrorData::invalid_params(
                "extension registry not loaded",
                None,
            ));
        };
        let Some(call) = reg.call(tool_name, &input) else {
            return Err(ErrorData::invalid_params(
                format!("unknown extension tool: {mcp_name}"),
                None,
            ));
        };
        drop(registry); // 调用期间不持 registry 读锁（实例锁独立）

        let joined = tokio::task::spawn_blocking(move || call);
        // M8-WP06：内层 epoch/fuel 为终止主机制（真终止，线程释放）；
        // 本 timeout 保留为二层防御（tick 线程异常时的兜底），错误文本
        // 与 epoch deadline / fuel 路径可区分。
        match tokio::time::timeout(EXT_CALL_TIMEOUT, joined).await {
            Ok(Ok(Ok(out))) => {
                // source 标注（SPEC §2.3）：扩展结果不隐式获得内建工具信任
                let parsed: JsonValue = serde_json::from_str(&out).unwrap_or_else(|_| {
                    serde_json::json!({ "raw": out })
                });
                ok_json(&serde_json::json!({
                    "source": "extension",
                    "tool": mcp_name,
                    "result": parsed,
                }))
            }
            Ok(Ok(Err(e))) => Err(ErrorData::internal_error(
                format!("extension call failed: {e}"),
                None,
            )),
            Ok(Err(e)) => Err(ErrorData::internal_error(
                format!("extension task join failed: {e}"),
                None,
            )),
            Err(_) => Err(ErrorData::internal_error(
                format!(
                    "extension call timed out after {}s (guest has no termination guarantee; the tool instance stays busy)",
                    EXT_CALL_TIMEOUT.as_secs()
                ),
                None,
            )),
        }
    }
}

/// 扩展调用超时（PR #35 审查 P1-1 处置）。
const EXT_CALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

// ─────────────────────────────────────────────────────────────────────────────
// 工具结果辅助
// ─────────────────────────────────────────────────────────────────────────────

fn ok_json<T: Serialize>(v: &T) -> Result<CallToolResult, ErrorData> {
    match serde_json::to_value(v) {
        Ok(val) => Ok(CallToolResult::structured(val)),
        Err(e) => Err(ErrorData::internal_error(format!("serialize: {e}"), None)),
    }
}

/// 工具级错误（调用方可见，`is_error: true`）。
fn tool_err(msg: impl Into<String>) -> Result<CallToolResult, ErrorData> {
    Ok(CallToolResult::error(vec![ContentBlock::text(msg.into())]))
}

// ─────────────────────────────────────────────────────────────────────────────
// 工具处理器
// ─────────────────────────────────────────────────────────────────────────────

impl McpServerState {
    /// asset_search：BM25 全文检索。
    ///
    /// WP02 `hybrid_search` 需调用方预算查询向量，MCP 面无向量输入，
    /// 走 `bm25_only`；hit 元数据（mime/mtime）由 graph JOIN 补全。
    async fn asset_search(&self, args: &JsonValue) -> Result<CallToolResult, ErrorData> {
        let input: AssetSearchInput = match serde_json::from_value(args.clone()) {
            Ok(v) => v,
            Err(e) => return Err(ErrorData::invalid_params(e.to_string(), None)),
        };
        let limit = input.limit.clamp(1, 100);

        let guard = self.index_engine.read().await;
        let Some(engine) = guard.as_ref() else {
            return ok_json(&AssetSearchOutput {
                hits: vec![],
                total: 0,
                timing_ms: 0,
            });
        };

        let bm25_result = engine
            .bm25_only(partisync_index::search::bm25::Bm25Query {
                query: input.query.clone(),
                limit,
                include_transcript: input.include_transcript,
            })
            .await
            .map_err(|e| ErrorData::internal_error(format!("bm25: {e}"), None))?;

        let mut hits = Vec::with_capacity(bm25_result.hits.len());
        for h in bm25_result.hits {
            let meta: Option<(Option<String>, i64)> = sqlx::query_as(
                "SELECT c.mime, e.mtime_ns FROM entry e JOIN content c ON c.id = e.content_id \
                 WHERE e.content_id = ? LIMIT 1",
            )
            .bind(&h.content_id)
            .fetch_optional(&self.graph_pool)
            .await
            .ok()
            .flatten();
            let (mime_kind, updated_ns) = meta.unwrap_or((None, 0));
            hits.push(SearchHit {
                content_id: h.content_id,
                score: h.score,
                highlights: h.highlight.into_iter().collect(),
                mime_kind,
                updated_ns,
            });
        }

        ok_json(&AssetSearchOutput {
            hits,
            total: bm25_result.total,
            timing_ms: bm25_result.timing_ms,
        })
    }

    /// asset_read：content + entry + sidecar_items + tag 聚合。
    async fn asset_read(&self, args: &JsonValue) -> Result<CallToolResult, ErrorData> {
        let input: AssetReadInput = match serde_json::from_value(args.clone()) {
            Ok(v) => v,
            Err(e) => return Err(ErrorData::invalid_params(e.to_string(), None)),
        };

        let pool = self.graph_pool();

        let content: Option<ContentRow> =
            sqlx::query_as("SELECT id, size, mime FROM content WHERE id = ?")
                .bind(&input.content_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| ErrorData::internal_error(format!("content query: {e}"), None))?;

        let Some(content) = content else {
            return tool_err(format!("content_id '{}' not found", input.content_id));
        };

        let entry: Option<EntryNameRow> = sqlx::query_as(
            "SELECT id, name, path, mtime_ns FROM entry WHERE content_id = ? LIMIT 1",
        )
        .bind(&input.content_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ErrorData::internal_error(format!("entry query: {e}"), None))?;

        let (entry_id, name, entry_path, updated_ns) = match entry {
            Some(e) => (e.id, e.name, e.path, e.mtime_ns),
            None => (String::new(), String::new(), String::new(), 0),
        };

        let stages: Vec<SidecarRow> = sqlx::query_as(
            "SELECT stage, status, detail, updated_ns FROM sidecar_items WHERE content_id = ?",
        )
        .bind(&input.content_id)
        .fetch_all(pool)
        .await
        .map_err(|e| ErrorData::internal_error(format!("sidecar query: {e}"), None))?;

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
        // M4-WP99-T04：c2pa absent → null（不是 "not_started"）。
        // 语义区分：其他 5 个 stage 由 WP01 管线确保总是存在至少一行（默认
        // not_started 也合理——管线启动会入队）；c2pa（M4-WP05 引入）走独立触发，
        // 调用方需明确区分「管线跑了但 c2pa 还没轮到」vs「完全没启用 c2pa」。
        let optional_stage_status = |s: &str| -> Option<String> {
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
                .map(|s| s.to_string())
        };

        let sidecar_stages = SidecarStages {
            thumbnail: stage_status("thumbnail"),
            exif: stage_status("exif"),
            ocr: stage_status("ocr"),
            transcribe: stage_status("transcribe"),
            embed: stage_status("embed"),
            // M4-WP99-T04：absent c2pa → null（不是 "not_started" 字符串）
            c2pa: optional_stage_status("c2pa"),
        };

        let tags: Vec<String> = if entry_path.is_empty() {
            vec![]
        } else {
            sqlx::query_scalar::<_, String>(
                "SELECT t.name FROM tag t JOIN entry_tag et ON et.tag_id = t.id \
                 WHERE et.entry_path = ? AND et.deleted = 0 AND t.deleted = 0",
            )
            .bind(&entry_path)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        };

        // embed detail 口径（WP01 T04）："dim=N model=X"
        let embedding = stages
            .iter()
            .find(|r| r.stage == "embed" && r.status == 2 && r.detail.is_some())
            .map(|r| {
                let detail = r.detail.as_deref().unwrap_or("");
                let text_dim = detail
                    .split_whitespace()
                    .find(|w| w.starts_with("dim="))
                    .and_then(|w| w.get(4..))
                    .and_then(|s| s.parse::<u32>().ok());
                let text_model = detail
                    .split_whitespace()
                    .find(|w| w.starts_with("model="))
                    .and_then(|w| w.get(6..))
                    .map(|s| s.to_string());
                EmbeddingInfo {
                    text_dim,
                    text_model,
                    image_dim: None,
                    image_model: None,
                }
            });

        ok_json(&AssetReadOutput {
            content_id: input.content_id,
            entry_id,
            name,
            mime_kind: content.mime.unwrap_or_default(),
            size_bytes: content.size,
            created_ns: 0,
            updated_ns,
            tags,
            sidecar_stages,
            embedding,
        })
    }

    /// asset_organize：标签管理 + 软删除。
    ///
    /// preview_only=true：返回操作计划，不写库。
    /// preview_only=false：单事务执行（任一操作失败整体回滚——set_tag 原子性）。
    ///
    /// **批大小上限 [`MAX_ORGANIZE_OPS`]**（M4-WP99-T02）：单事务 >100 ops 拒收，
    /// 防止长事务阻塞 DB + 大内存压力。错误码 = `invalid_params`。
    async fn asset_organize(&self, args: &JsonValue) -> Result<CallToolResult, ErrorData> {
        let input: AssetOrganizeInput = match serde_json::from_value(args.clone()) {
            Ok(v) => v,
            Err(e) => return Err(ErrorData::invalid_params(e.to_string(), None)),
        };

        if input.operations.len() > MAX_ORGANIZE_OPS {
            return Err(ErrorData::invalid_params(
                format!(
                    "operations batch too large: {} > {} (DoS protection)",
                    input.operations.len(),
                    MAX_ORGANIZE_OPS
                ),
                None,
            ));
        }

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
                    "delete" => format!("asset {} will be soft-deleted", op.content_id),
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
            return ok_json(&AssetOrganizeOutput {
                planned,
                transaction_id: None,
                confirm_url: None,
            });
        }

        let mut tx = self
            .graph_pool
            .begin()
            .await
            .map_err(|e| ErrorData::internal_error(format!("begin tx: {e}"), None))?;

        for op in &input.operations {
            if let Err(e) = apply_operation(&mut tx, op).await {
                let _ = tx.rollback().await;
                return Err(e);
            }
        }

        tx.commit()
            .await
            .map_err(|e| ErrorData::internal_error(format!("commit tx: {e}"), None))?;

        let txn_id = format!("txn_{}", uuid::Uuid::new_v4());
        ok_json(&AssetOrganizeOutput {
            planned,
            transaction_id: Some(txn_id.clone()),
            confirm_url: Some(format!("/api/assets/confirm/{txn_id}")),
        })
    }

    /// dataset_export：JSONL manifest（支持分片）。
    async fn dataset_export(&self, args: &JsonValue) -> Result<CallToolResult, ErrorData> {
        let input: DatasetExportInput = match serde_json::from_value(args.clone()) {
            Ok(v) => v,
            Err(e) => return Err(ErrorData::invalid_params(e.to_string(), None)),
        };

        if input.format != "jsonl" {
            return Err(ErrorData::invalid_params(
                format!("unsupported format '{}'", input.format),
                None,
            ));
        }

        let pool = self.graph_pool();

        // WHERE 只拼静态片段；所有值走 bind
        let mut conditions: Vec<String> = Vec::new();
        if let Some(ref ids) = input.content_ids {
            if !ids.is_empty() {
                let ph = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                conditions.push(format!("c.id IN ({ph})"));
            }
        }
        if let Some(ref filter) = input.filter {
            if filter.mime_kind.is_some() {
                conditions.push("c.mime LIKE ?".to_string());
            }
            if let Some(ref tags) = filter.tags {
                if !tags.is_empty() {
                    let ph = tags.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                    conditions.push(format!(
                        "e.path IN (SELECT et.entry_path FROM entry_tag et \
                         JOIN tag t ON t.id = et.tag_id \
                         WHERE t.name IN ({ph}) AND et.deleted = 0 AND t.deleted = 0)"
                    ));
                }
            }
        }
        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT c.id, c.size, c.mime, e.name, e.path, e.mtime_ns \
             FROM content c JOIN entry e ON e.content_id = c.id {where_clause}"
        );
        // 动态片段只含白名单占位符（IN (?,?,...) / LIKE ?），全部值走 bind——
        // 无用户输入拼接进 SQL 文本，AssertSqlSafe 审计成立（sqlx 0.9 SqlSafeStr 机制）。
        let mut q = sqlx::query_as::<_, ExportRow>(sqlx::AssertSqlSafe(sql));
        if let Some(ref ids) = input.content_ids {
            for id in ids {
                q = q.bind(id);
            }
        }
        if let Some(ref filter) = input.filter {
            if let Some(ref mime) = filter.mime_kind {
                q = q.bind(format!("{}%", mime.trim_end_matches('*')));
            }
            if let Some(ref tags) = filter.tags {
                for t in tags {
                    q = q.bind(t);
                }
            }
        }

        let rows: Vec<ExportRow> = q
            .fetch_all(pool)
            .await
            .map_err(|e| ErrorData::internal_error(format!("export query: {e}"), None))?;

        let output_dir = input.output_dir.clone().unwrap_or_else(|| {
            std::env::temp_dir()
                .join("partisync_export")
                .to_string_lossy()
                .to_string()
        });
        tokio::fs::create_dir_all(&output_dir)
            .await
            .map_err(|e| ErrorData::internal_error(format!("mkdir: {e}"), None))?;

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let shard_size = input.shard_size.unwrap_or(0).max(0) as u64;

        let mut record_count = 0u64;
        let mut total_bytes = 0u64;
        let mut shards: Vec<ShardInfo> = Vec::new();
        let mut file: Option<File> = None;
        let mut cur_shard_records = 0u64;
        let mut cur_path = String::new();
        let mut shard_idx = 0usize;

        for row in &rows {
            // 需要换片：未开文件，或已到分片上限
            if file.is_none() || (shard_size > 0 && cur_shard_records >= shard_size) {
                if let Some(mut f) = file.take() {
                    f.flush()
                        .await
                        .map_err(|e| ErrorData::internal_error(format!("flush: {e}"), None))?;
                    shards.push(ShardInfo {
                        path: cur_path.clone(),
                        record_count: cur_shard_records,
                    });
                    shard_idx += 1;
                }
                cur_path =
                    format!("{output_dir}/partisync_export_{timestamp}_{shard_idx:04}.jsonl");
                file =
                    Some(File::create(&cur_path).await.map_err(|e| {
                        ErrorData::internal_error(format!("create file: {e}"), None)
                    })?);
                cur_shard_records = 0;
            }

            let record = serde_json::json!({
                "content_id": row.id,
                "name": row.name,
                "path": row.path,
                "mime_kind": row.mime,
                "size_bytes": row.size,
                "mtime_ns": row.mtime_ns,
            });
            let line = serde_json::to_string(&record)
                .map_err(|e| ErrorData::internal_error(format!("serialize: {e}"), None))?;
            let f = file.as_mut().expect("shard file opened above");
            f.write_all(line.as_bytes())
                .await
                .map_err(|e| ErrorData::internal_error(format!("write: {e}"), None))?;
            f.write_all(b"\n")
                .await
                .map_err(|e| ErrorData::internal_error(format!("write newline: {e}"), None))?;

            record_count += 1;
            cur_shard_records += 1;
            total_bytes += row.size.max(0) as u64;
        }

        if let Some(mut f) = file.take() {
            f.flush()
                .await
                .map_err(|e| ErrorData::internal_error(format!("flush: {e}"), None))?;
            shards.push(ShardInfo {
                path: cur_path.clone(),
                record_count: cur_shard_records,
            });
        }

        let manifest_path = if shards.len() == 1 {
            shards[0].path.clone()
        } else {
            output_dir
        };
        let expires_at_ns =
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) + 86_400_000_000_000;

        ok_json(&DatasetExportOutput {
            manifest_path,
            record_count,
            total_bytes,
            shards,
            expires_at_ns,
        })
    }

    /// job_status：查询 graph.jobs（kind='sidecar'）。
    ///
    /// checkpoint 由 partisync-ai `jobs.rs` 落盘（JSON 对象）；此处只做
    /// 宽松读取（stage/next 字段），格式演进不破坏本工具。
    async fn job_status(&self, args: &JsonValue) -> Result<CallToolResult, ErrorData> {
        let input: JobStatusInput = match serde_json::from_value(args.clone()) {
            Ok(v) => v,
            Err(e) => return Err(ErrorData::invalid_params(e.to_string(), None)),
        };

        let pool = self.graph_pool();

        let jobs: Vec<JobRow> = if let Some(job_id) = &input.job_id {
            sqlx::query_as(
                "SELECT id, kind, status, root, done_files, checkpoint, error, created_ns, updated_ns \
                 FROM jobs WHERE id = ? AND kind = 'sidecar'",
            )
            .bind(job_id)
            .fetch_all(pool)
            .await
            .map_err(|e| ErrorData::internal_error(format!("job query: {e}"), None))?
        } else {
            sqlx::query_as(
                "SELECT id, kind, status, root, done_files, checkpoint, error, created_ns, updated_ns \
                 FROM jobs WHERE kind = 'sidecar' ORDER BY created_ns DESC LIMIT 20",
            )
            .fetch_all(pool)
            .await
            .map_err(|e| ErrorData::internal_error(format!("jobs query: {e}"), None))?
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
                let current_stage = j
                    .checkpoint
                    .as_deref()
                    .and_then(|c| serde_json::from_str::<JsonValue>(c).ok())
                    .and_then(|v| {
                        v.get("stage").or_else(|| v.get("next")).and_then(|x| {
                            x.as_str()
                                .map(|s| s.to_string())
                                .or_else(|| x.as_u64().map(|n| n.to_string()))
                        })
                    })
                    .unwrap_or_else(|| "unknown".to_string());
                let progress_pct = if j.status == 3 {
                    100
                } else if j.done_files > 0 {
                    ((j.done_files * 20).min(99)) as u8
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

        ok_json(&JobStatusOutput { jobs: infos })
    }
}

/// 单条 organize 操作（事务内执行；失败由调用方回滚整个事务）。
async fn apply_operation(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    op: &OrganizeOperation,
) -> Result<(), ErrorData> {
    // 通用前置校验（M4-WP99-T03 AuthZ 加固）：所有写操作必须先确认 content_id 存在
    // （先 SELECT content 表 + entry 表关联），否则静默 UPDATE 会产生 0 rows affected
    // 假成功（写假数据 / 越权 / 探测 content_id 命名空间）。
    // 加固：content_id 必须真实存在（关联 content + entry 都有行）→ 不存在拒收。
    let entry_path: Option<String> = sqlx::query_scalar(
        "SELECT e.path FROM entry e \
                              JOIN content c ON c.id = e.content_id \
                              WHERE e.content_id = ? LIMIT 1",
    )
    .bind(&op.content_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| ErrorData::internal_error(format!("entry/content probe: {e}"), None))?;
    let Some(path) = entry_path else {
        return Err(ErrorData::invalid_params(
            format!(
                "content_id '{}' not found in this library (cross-DB / unknown)",
                op.content_id
            ),
            None,
        ));
    };

    match op.action.as_str() {
        "add_tag" | "set_tag" | "remove_tag" => {
            let tag_name = op.value.as_deref().unwrap_or("").trim().to_string();
            if tag_name.is_empty() {
                return Err(ErrorData::invalid_params(
                    format!("action '{}' requires non-empty value", op.action),
                    None,
                ));
            }
            // 上面预检已验过 entry/content；此处直接用 path

            if op.action == "set_tag" {
                sqlx::query(
                    "UPDATE entry_tag SET deleted = 1 WHERE entry_path = ? AND deleted = 0",
                )
                .bind(&path)
                .execute(&mut **tx)
                .await
                .map_err(|e| ErrorData::internal_error(format!("clear tags: {e}"), None))?;
            }

            if op.action == "remove_tag" {
                sqlx::query(
                    "UPDATE entry_tag SET deleted = 1 WHERE entry_path = ? \
                     AND tag_id = (SELECT id FROM tag WHERE name = ? AND deleted = 0) \
                     AND deleted = 0",
                )
                .bind(&path)
                .bind(&tag_name)
                .execute(&mut **tx)
                .await
                .map_err(|e| ErrorData::internal_error(format!("remove tag: {e}"), None))?;
                return Ok(());
            }

            // add_tag / set_tag：查找或创建 tag
            let tag_id: Option<String> =
                sqlx::query_scalar("SELECT id FROM tag WHERE name = ? AND deleted = 0 LIMIT 1")
                    .bind(&tag_name)
                    .fetch_optional(&mut **tx)
                    .await
                    .map_err(|e| ErrorData::internal_error(format!("tag lookup: {e}"), None))?;
            let tag_id = match tag_id {
                Some(id) => id,
                None => {
                    let new_id = Ulid::now().to_string();
                    sqlx::query(
                        "INSERT INTO tag (id, space_id, name, deleted) VALUES (?, 'default', ?, 0)",
                    )
                    .bind(&new_id)
                    .bind(&tag_name)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| ErrorData::internal_error(format!("create tag: {e}"), None))?;
                    new_id
                }
            };

            // 已有墓碑行则复活，否则插入（幂等）
            sqlx::query(
                "INSERT INTO entry_tag (tag_id, entry_path, deleted) VALUES (?, ?, 0) \
                 ON CONFLICT(tag_id, entry_path) DO UPDATE SET deleted = 0",
            )
            .bind(&tag_id)
            .bind(&path)
            .execute(&mut **tx)
            .await
            .map_err(|e| ErrorData::internal_error(format!("link tag: {e}"), None))?;
            Ok(())
        }
        "delete" => {
            sqlx::query("UPDATE entry SET state = 1 WHERE content_id = ?")
                .bind(&op.content_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| ErrorData::internal_error(format!("delete: {e}"), None))?;
            Ok(())
        }
        other => Err(ErrorData::invalid_params(
            format!("unknown action: {other}"),
            None,
        )),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 运行入口
// ─────────────────────────────────────────────────────────────────────────────

/// 运行 MCP Server（RMCP stdio 传输）。
///
/// `McpServerState` 实现 `ServerHandler`，经 blanket impl
/// `impl<H: ServerHandler> Service<RoleServer> for H`（handler/server.rs:50）
/// 满足 `Service<RoleServer>`，故直接传 state 给 `serve_server`。
///
/// `index_root`：检索引擎索引目录（与 CLI `search --index-root` 同约定）；
/// `None` 用默认 `~/.partisync/index`。索引打开失败不致命——`asset_search`
/// 退化为空结果，其余四工具不受影响。
pub async fn run_mcp_server(
    graph_db_path: Option<PathBuf>,
    index_root: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = McpServerState::new(graph_db_path)
        .await
        .map_err(|e| format!("failed to open graph.db: {e}"))?;

    let root = index_root.unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".partisync")
            .join("index")
    });
    match partisync_index::search::engine::IndexEngine::open_or_create(
        partisync_index::IndexEngineConfig {
            index_root: root,
            enable_reranker: false,
            reranker_model_dir: None,
        },
    ) {
        Ok(engine) => state.install_index_engine(engine).await,
        Err(e) => eprintln!("partisync-mcp: index engine unavailable, asset_search disabled: {e}"),
    }

    // M7-WP01-T04 第 3 步：装载扩展注册表（SPEC §2.2 发现约定目录）。
    // 扩展是可选增强：目录不存在 → 空；装载失败 → 告警 + 不注入
    // （扩展目录损坏不拖垮 MCP 主服务；fail-closed 语义保留在
    // `ExtRegistry::scan` 层面——要么全量装载成功要么不装载）。
    let ext_dir = crate::ext::default_extensions_dir();
    let index_reader = state.index_engine.read().await.clone();
    match crate::ext::load_registry(&ext_dir, index_reader) {
        Ok((registry, scanned_dir)) if !registry.is_empty() => {
            // 必须走 stderr：stdout 是 MCP stdio JSON-RPC 协议通道，
            // 非 JSON 行混入依赖客户端 reader 容错（M7-WP01-T05 报告
            // §5 登记）；规范化到 stderr 消除该耦合。
            eprintln!(
                "partisync-mcp: loaded {} extension tool(s) from {}",
                registry.len(),
                scanned_dir.display()
            );
            state.install_ext_registry(registry, scanned_dir).await;
        }
        Ok(_) => {}
        Err(e) => eprintln!(
            "partisync-mcp: extension registry load failed ({}), extensions disabled: {e}",
            ext_dir.display()
        ),
    }

    // RunningService drop 即 shutdown——必须 waiting 到客户端断开（stdio EOF）
    let service = rmcp::service::serve_server(state, stdio()).await?;
    service.waiting().await?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 测试（离线内存库；调用 handler 方法，不经 stdio 传输）
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 内存 SQLite + 最小 schema（与 graph schema.sql 相应表同构）。
    async fn test_state() -> McpServerState {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for ddl in [
            "CREATE TABLE content (id TEXT PRIMARY KEY, size INTEGER NOT NULL, mime TEXT, kind TEXT)",
            "CREATE TABLE entry (id TEXT PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL, \
             content_id TEXT, size INTEGER NOT NULL DEFAULT 0, mtime_ns INTEGER NOT NULL DEFAULT 0, \
             state INTEGER NOT NULL DEFAULT 0)",
            "CREATE TABLE tag (id TEXT PRIMARY KEY, space_id TEXT NOT NULL DEFAULT 'default', \
             name TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0)",
            "CREATE TABLE entry_tag (tag_id TEXT NOT NULL, entry_path TEXT NOT NULL, \
             deleted INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (tag_id, entry_path))",
            "CREATE TABLE sidecar_items (content_id TEXT NOT NULL, stage TEXT NOT NULL, \
             status INTEGER NOT NULL DEFAULT 0, detail TEXT, artifact TEXT, \
             updated_ns INTEGER NOT NULL, PRIMARY KEY (content_id, stage))",
            "CREATE TABLE jobs (id TEXT PRIMARY KEY, kind TEXT NOT NULL, status INTEGER NOT NULL, \
             root TEXT NOT NULL, checkpoint TEXT, done_files INTEGER NOT NULL DEFAULT 0, \
             error TEXT, created_ns INTEGER NOT NULL, updated_ns INTEGER NOT NULL)",
        ] {
            sqlx::query(ddl).execute(&pool).await.unwrap();
        }
        McpServerState::from_pool(pool)
    }

    async fn seed(state: &McpServerState) {
        let pool = state.graph_pool();
        sqlx::query("INSERT INTO content (id, size, mime) VALUES ('c1', 100, 'image/jpeg')")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO entry (id, name, path, content_id, size, mtime_ns) \
             VALUES ('e1', 'IMG_1.jpg', '/photos/IMG_1.jpg', 'c1', 100, 111)",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO sidecar_items (content_id, stage, status, detail, updated_ns) VALUES \
             ('c1','embed',2,'dim=768 model=bge-m3',1), \
             ('c1','ocr',2,NULL,1), ('c1','transcribe',3,NULL,1), \
             ('c1','c2pa',2,'state=Valid label=urn:uuid:test bytes=456',1)",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO jobs (id, kind, status, root, checkpoint, done_files, created_ns, updated_ns) \
             VALUES ('j1','sidecar',4,'c1','{\"stage\":\"ocr\"}',3,10,20), \
                    ('j2','sidecar',3,'c1',NULL,5,30,40)",
        )
        .execute(pool)
        .await
        .unwrap();
    }

    /// M7-WP01-T04 热修回归：首启即用——db 文件与父目录均缺失时
    /// [`McpServerState::new`] 应建库成功（此前 `sqlite:{path}` 无
    /// mode=rwc 也不建目录，首启必败 sqlite code 14，桌面壳侧车与
    /// CLI 默认路径首次运行即触发）。
    #[tokio::test]
    async fn new_creates_missing_db_and_parent_dirs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("nested/deeper/graph.db");
        let state = McpServerState::new(Some(db.clone()))
            .await
            .expect("首启路径应自动建库");
        assert!(db.is_file(), "db 文件应已创建: {}", db.display());
        // 连接真实可用（非仅文件落盘）
        let one: i64 = sqlx::query_scalar("SELECT 1")
            .fetch_one(state.graph_pool())
            .await
            .expect("建库后应可执行查询");
        assert_eq!(one, 1);
        state.graph_pool().close().await;
    }

    #[tokio::test]
    async fn asset_read_returns_full_metadata() {
        let s = test_state().await;
        seed(&s).await;
        let r = s.asset_read(&json!({"content_id": "c1"})).await.unwrap();
        let v = r.structured_content.unwrap();
        assert_eq!(v["content_id"], "c1");
        assert_eq!(v["name"], "IMG_1.jpg");
        assert_eq!(v["mime_kind"], "image/jpeg");
        assert_eq!(v["sidecar_stages"]["embed"], "done");
        assert_eq!(v["sidecar_stages"]["transcribe"], "skipped");
        assert_eq!(v["sidecar_stages"]["c2pa"], "done");
        assert_eq!(v["embedding"]["text_dim"], 768);
        assert_eq!(v["embedding"]["text_model"], "bge-m3");
    }

    #[tokio::test]
    async fn asset_read_missing_returns_tool_error() {
        let s = test_state().await;
        let r = s.asset_read(&json!({"content_id": "nope"})).await.unwrap();
        assert_eq!(r.is_error, Some(true));
    }

    #[tokio::test]
    async fn asset_organize_add_tag_and_read_back() {
        let s = test_state().await;
        seed(&s).await;
        let r = s
            .asset_organize(&json!({
                "operations": [{"content_id": "c1", "action": "add_tag", "value": "vacation"}],
                "preview_only": false
            }))
            .await
            .unwrap();
        assert!(r.structured_content.as_ref().unwrap()["transaction_id"].is_string());
        let r = s.asset_read(&json!({"content_id": "c1"})).await.unwrap();
        assert_eq!(r.structured_content.unwrap()["tags"], json!(["vacation"]));
    }

    #[tokio::test]
    async fn asset_organize_preview_writes_nothing() {
        let s = test_state().await;
        seed(&s).await;
        s.asset_organize(&json!({
            "operations": [{"content_id": "c1", "action": "add_tag", "value": "vacation"}],
            "preview_only": true
        }))
        .await
        .unwrap();
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entry_tag")
            .fetch_one(s.graph_pool())
            .await
            .unwrap();
        assert_eq!(n, 0);
    }

    #[tokio::test]
    async fn asset_organize_set_tag_atomic_replaces() {
        let s = test_state().await;
        seed(&s).await;
        s.asset_organize(&json!({
            "operations": [{"content_id": "c1", "action": "add_tag", "value": "old"}],
            "preview_only": false
        }))
        .await
        .unwrap();
        s.asset_organize(&json!({
            "operations": [{"content_id": "c1", "action": "set_tag", "value": "new"}],
            "preview_only": false
        }))
        .await
        .unwrap();
        let names: Vec<String> = sqlx::query_scalar(
            "SELECT t.name FROM tag t JOIN entry_tag et ON et.tag_id = t.id \
             WHERE et.entry_path = '/photos/IMG_1.jpg' AND et.deleted = 0",
        )
        .fetch_all(s.graph_pool())
        .await
        .unwrap();
        assert_eq!(names, vec!["new".to_string()]);
    }

    #[tokio::test]
    async fn asset_organize_bad_content_rolls_back_tx() {
        let s = test_state().await;
        seed(&s).await;
        let r = s
            .asset_organize(&json!({
                "operations": [
                    {"content_id": "c1", "action": "add_tag", "value": "good"},
                    {"content_id": "ghost", "action": "add_tag", "value": "bad"}
                ],
                "preview_only": false
            }))
            .await;
        assert!(r.is_err(), "second op on unknown content must fail");
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entry_tag")
            .fetch_one(s.graph_pool())
            .await
            .unwrap();
        assert_eq!(n, 0, "first op must be rolled back with the tx");
    }

    #[tokio::test]
    async fn asset_organize_delete_soft_deletes_entry() {
        let s = test_state().await;
        seed(&s).await;
        s.asset_organize(&json!({
            "operations": [{"content_id": "c1", "action": "delete"}],
            "preview_only": false
        }))
        .await
        .unwrap();
        let st: i64 = sqlx::query_scalar("SELECT state FROM entry WHERE id = 'e1'")
            .fetch_one(s.graph_pool())
            .await
            .unwrap();
        assert_eq!(st, 1);
    }

    #[tokio::test]
    async fn job_status_maps_rows() {
        let s = test_state().await;
        seed(&s).await;
        let r = s.job_status(&json!({})).await.unwrap();
        let v = r.structured_content.unwrap();
        let jobs = v["jobs"].as_array().unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0]["job_id"], "j2"); // created_ns DESC
        assert_eq!(jobs[0]["status"], "done");
        assert_eq!(jobs[1]["job_id"], "j1");
        assert_eq!(jobs[1]["status"], "failed");
        assert_eq!(jobs[1]["current_stage"], "ocr");

        let r = s.job_status(&json!({"job_id": "j1"})).await.unwrap();
        assert_eq!(
            r.structured_content.unwrap()["jobs"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn dataset_export_writes_jsonl_shards() {
        let s = test_state().await;
        seed(&s).await;
        let dir = std::env::temp_dir().join(format!("psexport_{}", uuid::Uuid::new_v4()));
        let r = s
            .dataset_export(&json!({
                "content_ids": ["c1"],
                "format": "jsonl",
                "shard_size": 1,
                "output_dir": dir.to_string_lossy()
            }))
            .await
            .unwrap();
        let v = r.structured_content.unwrap();
        assert_eq!(v["record_count"], 1);
        let shard_path = v["shards"][0]["path"].as_str().unwrap().to_string();
        let content = std::fs::read_to_string(&shard_path).unwrap();
        assert!(content.contains("\"content_id\":\"c1\""), "got: {content}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn list_tools_returns_nine() {
        let tools = all_tools();
        assert_eq!(tools.len(), 9);
        let names: Vec<_> = tools.iter().map(|t| t.name.to_string()).collect();
        for n in [
            "asset_search",
            "asset_read",
            "asset_organize",
            "dataset_export",
            "job_status",
            "memory_write",
            "memory_search",
            "memory_verify",
            "ext_list",
        ] {
            assert!(names.contains(&n.to_string()), "missing tool {n}");
        }
    }

    // ── 扩展接线（M7-WP01-T04 第 3 步）─────────────────────────────

    /// 示例扩展 fixture（复制到临时目录装载）。
    async fn state_with_demo_ext() -> McpServerState {
        let state = test_state().await;
        let dir = tempfile::tempdir().unwrap();
        std::fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../partisync-ext-host/tests/fixtures/demo_tool.wasm"),
            dir.path().join("demo_ext.wasm"),
        )
        .unwrap();
        std::fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../partisync-ext-host/tests/fixtures/demo_tool.minisig"),
            dir.path().join("demo_ext.minisig"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("demo_ext.json"),
            r#"{"tool_name":"demo_echo","capabilities":[]}"#,
        )
        .unwrap();
        // [P21]（SPEC M9-WP04）：fixture 签名出自 test-only 钥
        // （ext-host tests/fixtures/test-signing.pub，注释显式标注非生产
        // 钥，base64 原样拷贝；产品锚不信任它）——测试经 ext-host 参数
        // 化锚集入口装载（SPEC §4「测试钥签名路径」）；产品路径
        // `crate::ext::load_registry` 恒走内嵌发布双钥，不在测试改动面。
        let anchors = [partisync_ext_host::pubkey_from_base64(
            "RWQVdtZEx/IGjqq0d6egWWBl2+3QtiS0aNQrpc/F6Tqrt5TAaOdIc72B",
        )
        .expect("test-only pubkey base64 合法（fixtures/test-signing.pub 原样）")];
        let registry = partisync_ext_host::ExtRegistry::scan_with_anchors(
            dir.path(),
            &partisync_ext_host::HostState::without_index(),
            &anchors,
        )
        .expect("demo 扩展装载成功");
        state
            .install_ext_registry(registry, dir.path().to_path_buf())
            .await;
        state
    }

    #[tokio::test]
    async fn ext_list_lists_registered_extensions() {
        let state = state_with_demo_ext().await;
        let r = state.ext_list().await.expect("ext_list 成功");
        let v = r.structured_content.expect("structured 输出");
        assert_eq!(v["count"], 1);
        assert_eq!(v["tools"][0]["name"], "ext_demo_echo");
        assert_eq!(v["tools"][0]["capabilities"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn ext_list_empty_registry() {
        let state = test_state().await;
        let r = state.ext_list().await.expect("ext_list 成功");
        let v = r.structured_content.expect("structured 输出");
        assert_eq!(v["count"], 0);
    }

    #[tokio::test]
    async fn call_extension_demo_echo_round_trip() {
        let state = state_with_demo_ext().await;
        let r = state
            .call_extension(
                "ext_demo_echo",
                &serde_json::json!({ "input": r#"{"k":"v"}"# }),
            )
            .await
            .expect("扩展调用成功");
        let v = r.structured_content.expect("structured 输出");
        assert_eq!(v["source"], "extension", "SPEC §2.3 来源标注");
        assert_eq!(v["tool"], "ext_demo_echo");
        assert_eq!(v["result"]["input_bytes"], 9);
    }

    #[tokio::test]
    async fn call_extension_unknown_tool_rejected() {
        let state = state_with_demo_ext().await;
        let err = state
            .call_extension("ext_nonexistent", &serde_json::json!({}))
            .await
            .expect_err("未知扩展工具必须拒");
        assert!(err.to_string().contains("unknown extension tool"));
    }

    /// `ext_` 前缀但 registry 未装载 → 明确错误（非 unknown tool 混淆）。
    #[tokio::test]
    async fn call_extension_without_registry_rejected() {
        let state = test_state().await;
        let err = state
            .call_extension("ext_demo_echo", &serde_json::json!({}))
            .await
            .expect_err("registry 未装载必须拒");
        assert!(err.to_string().contains("registry not loaded"));
    }
}
