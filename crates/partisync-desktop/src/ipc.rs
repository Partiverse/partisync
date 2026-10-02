//! IPC commands（T03 期 6 个 + T05 期 1 个 `mcp_call`）。
//!
//! 由前端 `window.__TAURI__.invoke(cmd, args)` 调用； 所有命令接收
//! `tauri::State<AppState>` 句柄， 返回 [`crate::error::DesktopResult`]。
//!
//! ## 命令清单（SPEC §2.3）
//! - `get_stats` → [`Stats`] 全局统计
//! - `list` → `Vec<EntryRow>` 指定前缀下的子条目
//! - `search` → `Vec<SearchHit>` BM25 全文检索命中
//! - `cas_stats` → [`CasStats`] 块库统计
//! - `duplicates` → `Vec<DupGroup>` 内容级去重组
//! - `jobs` → `Vec<JobRow>` 作业列表
//! - `mcp_call` (T05) → `serde_json::Value` 转发到 `partisync-mcp` 侧车
//!
//! [`Stats`]: partisync_graph::store::Stats
//! [`CasStats`]: partisync_cas::CasStats

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

use partisync_cas::CasStats;
use partisync_graph::jobs::{self, JobRow};
use partisync_graph::store::{DupGroup, EntryRow, Stats};
use partisync_index::{Bm25Query, HybridQuery, HybridVectorKind, SearchFilters, VectorKind};

use crate::error::{DesktopError, DesktopResult};
use crate::state::AppState;

/// IPC 检索命中（Bm25Hit / HybridHit 的**统一** DTO；`mode` 标注检索
/// 通道——function-map §4-N3：HybridResult 的 `rrf_score` 与 BM25 的
/// `score` 量纲不同，前端渲染统一，通道差异由 `mode` 呈现）。
#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub content_id: String,
    pub score: f32,
    pub highlight: Option<String>,
    /// 检索通道（`bm25` / `hybrid`；M8-WP05-T01 起随载荷下发）。
    pub mode: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct ListArgs {
    pub prefix: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchArgs {
    pub q: String,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct DuplicatesArgs {
    pub top: u32,
}

/// `mcp_call` IPC 入参（SPEC §2.3 第 7 命令）。 `tool` 为 MCP 工具名
/// （如 `asset_search`）， `args` 为工具参数 JSON 对象。
#[derive(Debug, Deserialize)]
pub struct McpCallArgs {
    pub tool: String,
    pub args: Value,
}

/// 全局统计（图谱 / 去重节省 / 重复组数）。
#[tauri::command]
pub async fn get_stats(state: State<'_, AppState>) -> DesktopResult<Stats> {
    Ok(state.store.stats().await?)
}

/// 指定前缀下的子条目（目录浏览）。
#[tauri::command]
pub async fn list(state: State<'_, AppState>, args: ListArgs) -> DesktopResult<Vec<EntryRow>> {
    Ok(state.store.children(&args.prefix).await?)
}

/// BM25 全文检索（与 `partisync search` CLI 对齐， M4-WP02）。
///
/// `IndexEngine` 懒加载： 首次调用打开， 后续复用； 打开失败
/// → `DesktopError::Index`（SPEC §2.6 `IndexUnavailable`）。
#[tauri::command]
pub async fn search(state: State<'_, AppState>, args: SearchArgs) -> DesktopResult<Vec<SearchHit>> {
    let limit = args.limit.unwrap_or(20).min(100) as usize;
    let engine = state.index().await?;
    let result = engine
        .bm25_only(Bm25Query {
            query: args.q,
            limit,
            include_transcript: true,
        })
        .await
        .map_err(|e| DesktopError::Index(e.to_string()))?;
    Ok(result
        .hits
        .into_iter()
        .map(|h| SearchHit {
            content_id: h.content_id,
            score: h.score,
            highlight: h.highlight,
            mode: "bm25",
        })
        .collect())
}

/// 语义混合检索 IPC（M8-WP05-T01 旗舰；BM25 + 向量 RRF）。
///
/// 与 [`search`] 的差异只有**查询向量**来源：语义模式先经
/// `AppState::embedder()`（fastembed BGE-small-zh-v1.5，**懒加载**——
/// 冷启动路径零模型加载，function-map §4-N1）产出 512d 向量，再走
/// `IndexEngine::hybrid_search`（`VectorKind::TextDenseZh512`，与
/// M6-D67-T03 判例一致）。
///
/// 返回形状与 [`search`] **完全一致**（统一 SearchHit + `mode`）——
/// 前端结果渲染零分支（§4-N3）。首次语义检索需加载模型权重（秒级），
/// 前端在调用期间显示 loading 态。
///
/// # Errors
/// 嵌入模型初始化/推理失败，或 hybrid 检索失败 →
/// `DesktopError::Index`（`kind:"Index"`）。
#[tauri::command]
pub async fn search_hybrid(
    state: State<'_, AppState>,
    args: SearchArgs,
) -> DesktopResult<Vec<SearchHit>> {
    let limit = args.limit.unwrap_or(20).min(100) as usize;
    let query = args.q.clone();
    let embedder = state.embedder().await?;
    let query_vector = embedder.embed_query(&query)?;
    let engine = state.index().await?;
    let result = engine
        .hybrid_search(
            &query,
            &query_vector,
            VectorKind::TextDenseZh512,
            HybridQuery {
                query: query.clone(),
                filters: SearchFilters::default(),
                limit,
                vector_kind: HybridVectorKind::TextDense,
                include_transcript: true,
                use_reranker: false,
            },
        )
        .await
        .map_err(|e| DesktopError::Index(e.to_string()))?;
    Ok(result
        .hits
        .into_iter()
        .map(|h| SearchHit {
            content_id: h.content_id,
            score: h.rrf_score,
            highlight: h.highlight,
            mode: "hybrid",
        })
        .collect())
}

/// 块库统计（chunk-level dedup + saved_bytes）。
#[tauri::command]
pub async fn cas_stats(state: State<'_, AppState>) -> DesktopResult<CasStats> {
    Ok(state.cas.stats().await?)
}

/// 内容级去重组（top N 组， 按节省字节排序）。
#[tauri::command]
pub async fn duplicates(
    state: State<'_, AppState>,
    args: DuplicatesArgs,
) -> DesktopResult<Vec<DupGroup>> {
    Ok(state.store.duplicates(args.top).await?)
}

/// 作业列表（按 updated_ns desc， 取自 `jobs::list`）。
#[tauri::command]
pub async fn jobs(state: State<'_, AppState>) -> DesktopResult<Vec<JobRow>> {
    Ok(jobs::list(&state.store).await?)
}

/// 转发到 `partisync-mcp` 侧车进程（SPEC §2.3 第 7 命令）。
///
/// 懒 spawn 机制在 [`crate::mcp_sidecar::McpSidecar::call`] 内：
/// 首次调用时拉起 `partisync-mcp --db <db> --index-root <index>` 子进程，
/// stdio JSON-RPC 2.0 传输， 后续调用复用同进程。 子进程崩溃由 reader
/// task EOF 检测 → 下次 call 自动重启（`child.is_none()` 重 spawn）。
///
/// # Errors
/// 侧车 spawn / stdin 写 / JSON-RPC `error` 字段 / reader 异常退出 →
/// `DesktopError::Sidecar`（对应 SPEC §2.6 `SidecarSpawnFailed`）。
#[tauri::command]
pub async fn mcp_call(state: State<'_, AppState>, args: McpCallArgs) -> DesktopResult<Value> {
    state.mcp_sidecar.call(&args.tool, args.args).await
}
