//! IPC commands（T03 期 6 个， T05 期扩 `mcp_call`）。
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
//!
//! [`Stats`]: partisync_graph::store::Stats
//! [`CasStats`]: partisync_cas::CasStats

use serde::{Deserialize, Serialize};
use tauri::State;

use partisync_cas::CasStats;
use partisync_graph::jobs::{self, JobRow};
use partisync_graph::store::{DupGroup, EntryRow, Stats};
use partisync_index::Bm25Query;

use crate::error::{DesktopError, DesktopResult};
use crate::state::AppState;

/// IPC 检索命中（Bm25Hit 的 IPC DTO； 保持字段最少， `content_id`
/// 前端可走 `get_stats` / `list` / `duplicates` 反查路径）。
#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub content_id: String,
    pub score: f32,
    pub highlight: Option<String>,
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
