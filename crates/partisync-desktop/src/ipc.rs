//! IPC commands（11 个： M6-WP03 基础 6 + T01/T02/T05 增量 + M8-WP05-T03
//! 同步对）。
//!
//! 由前端 `window.__TAURI__.invoke(cmd, args)` 调用； 所有命令接收
//! `tauri::State<AppState>` 句柄， 返回 [`crate::error::DesktopResult`]。
//!
//! ## 命令清单（SPEC §2.3）
//! - `get_stats` → [`Stats`] 全局统计
//! - `list` → `Vec<EntryRow>` 指定前缀下的子条目
//! - `search` → `Vec<SearchHit>` BM25 全文检索命中
//! - `search_hybrid` (T01) → `Vec<SearchHit>` 语义混合检索命中
//! - `asset_detail` (T02) → [`AssetDetail`] 条目详情
//! - `cas_stats` → [`CasStats`] 块库统计
//! - `duplicates` → `Vec<DupGroup>` 内容级去重组
//! - `jobs` → `Vec<JobRow>` 作业列表
//! - `sync_stats` (M8-WP05-T03) → [`SyncStatsView`] 同步状态只读派生
//! - `sync_recent` (M8-WP05-T03) → `Vec<SyncRecentItem>` 最近活动时间线
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
// M8-WP05-T03： sync 域统计口径类型（只读复用， 无写路径/网络面——N2）。
use partisync_sync::session::SyncStats as SyncSessionStats;

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
    /// 转写文本通道开关（M9-WP03-T01，SPEC §2.1 N4 诚实化）：
    /// None = 维持后端常开现状（历史语义）；Some(false) = 排除转写命中。
    #[serde(default)]
    pub include_transcript: Option<bool>,
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
            include_transcript: args.include_transcript.unwrap_or(true),
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
                include_transcript: args.include_transcript.unwrap_or(true),
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

/// 条目详情 IPC（M8-WP05-T02；SPEC §2.2）：同 content_id 全部路径 +
/// 大小/时间（详情面板数据源）。空 vec = content_id 不存在。
#[derive(Debug, Serialize)]
pub struct AssetDetail {
    pub content_id: String,
    pub size: u64,
    pub copies: Vec<EntryRow>,
}

/// 条目详情（按 content_id 反查全部路径）。
#[tauri::command]
pub async fn asset_detail(
    state: State<'_, AppState>,
    args: ListArgs,
) -> DesktopResult<AssetDetail> {
    let rows = state.store.entries_by_content(&args.prefix).await?;
    Ok(AssetDetail {
        content_id: args.prefix,
        size: rows.first().map(|r| r.size).unwrap_or(0).max(0) as u64,
        copies: rows,
    })
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

/// 同步状态视图（M8-WP05-T03；SPEC §2.3）。
///
/// **只读派生**： 直接聚合 graph store 既有持久态（`sync_oplog` /
/// `sync_conflict` 表）， 不引入任何写路径/网络面——Hub/iroh 不进
/// desktop 进程（function-map §4-N2）。 字段口径复用 sync 域会话统计
/// 类型 `partisync_sync::session::SyncStats`（该类型无 `Serialize`，
/// IPC 层另包本视图）：
/// - `applied`： oplog 中**远端 origin** 行 = 本节点已应用的他机变更；
/// - `skipped_self`： 本机 origin 行（对端回放按回环防护跳过——
///   mockup「本机跳过（回环）」口径）；
/// - `skipped_lww`： LWW 落选行不入库（`INSERT OR IGNORE` 丢弃），
///   读侧恒 0， UI 照实呈现；
/// - `conflicts`： `sync_conflict` 血缘表行数（P11「保留两者」落档）。
#[derive(Debug, Serialize)]
pub struct SyncStatsView {
    pub applied: u64,
    pub skipped_self: u64,
    pub skipped_lww: u64,
    pub conflicts: u64,
    /// 已对账设备数（oplog 去重后的远端 origin）。
    pub devices: u64,
    /// 最近对账时间（远端 origin 行最大 `at_ns`；`None` = 尚无他机变更）。
    pub last_sync_ns: Option<i64>,
}

/// `sync_recent` IPC 入参。
#[derive(Debug, Deserialize)]
pub struct SyncRecentArgs {
    pub limit: Option<u32>,
}

/// 同步最近活动行（M8-WP05-T03 时间线；oplog 尾部 + 冲突血缘合并）。
#[derive(Debug, Serialize)]
pub struct SyncRecentItem {
    pub at_ns: i64,
    /// 文件名（oplog payload `name` / path 末段；冲突 = base_path 末段）。
    pub name: String,
    /// 所在目录（冲突行 = 空串）。
    pub dir: String,
    /// 来源设备（冲突 = 来方设备）。
    pub origin_device: String,
    /// `upsert` / `remove` / `conflict`。
    pub op: String,
    /// P11 冲突血缘行（前端琥珀标注）。
    pub conflict: bool,
    /// 内容身份（指纹色派生；冲突行 = `None`）。
    pub content_id: Option<String>,
}

/// oplog 尾部扫描深度（HLC 升序取尾；与 `limit` 合并后截断）。
const OPLOG_TAIL: usize = 200;
/// 冲突血缘统计扫描上限（桌面壳演示规模， 冲突量级远低于此）。
const CONFLICT_SCAN_LIMIT: u32 = 10_000;

/// 同步状态只读统计（M8-WP05-T03；SPEC §2.3）。
///
/// 未登记设备的空库（`device` 表空）按捕获侧同款兜底 `device-local`
/// 处理——全库 oplog 行均视为本机 origin， 前端呈现「尚未与其他设备
/// 同步」。
///
/// **F1 口径（M9-WP01-T04）**：`devices` / `last_sync_ns` 改由持久
/// `sync_watermark` 表派生（`Store::watermarks`，push 逐行
/// `note_applied` 只增、不被 ACK trim）——旧口径从 `pending_oplog`
/// 现算，push ACK 裁剪后归零（M8-WP05-ui-report D1/F1）。
/// `last_sync_ns` 从水位 HLC 键 phys 段（定宽 hex 毫秒，`hlc.rs`
/// `to_key` 契约）换算纳秒。
///
/// # Errors
/// DB 错误 → `DesktopError::Internal`（`{kind:"Internal"}`，H1）。
#[tauri::command]
pub async fn sync_stats(state: State<'_, AppState>) -> DesktopResult<SyncStatsView> {
    let self_device = state
        .store
        .device_id()
        .await
        .unwrap_or_else(|_| "device-local".into());
    let rows = state.store.pending_oplog().await?;
    let mut applied = 0u64;
    let mut skipped_self_origin = 0u64;
    for r in &rows {
        if r.origin_device == self_device {
            skipped_self_origin += 1;
            continue;
        }
        applied += 1;
    }
    // F1：设备面/最近对账时间 = 持久水位（远端 origin → last_hlc），
    // 不随 ACK trim 归零；self origin 行不经 note_applied（push 跳过
    // 分支），仍按 self_device 过滤兜底。
    let mut devices = 0u64;
    let mut last_sync_ns = None;
    for (device, last_hlc) in state.store.watermarks().await? {
        if device == self_device {
            continue;
        }
        devices += 1;
        let phys_ns = last_hlc
            .split('-')
            .next()
            .and_then(|p| u64::from_str_radix(p, 16).ok())
            .and_then(|ms| i64::try_from(ms).ok())
            .and_then(|ms| ms.checked_mul(1_000_000));
        last_sync_ns = last_sync_ns.max(phys_ns);
    }
    // 口径对齐 sync 域类型（skipped_lww 恒 0： LWW 落选行不入库）。
    let session = SyncSessionStats {
        applied,
        skipped_self_origin,
        skipped_lww: 0,
        conflicts: 0,
    };
    let conflicts = state.store.list_conflicts(CONFLICT_SCAN_LIMIT).await?.len() as u64;
    Ok(SyncStatsView {
        applied: session.applied,
        skipped_self: session.skipped_self_origin,
        skipped_lww: session.skipped_lww,
        conflicts,
        devices,
        last_sync_ns,
    })
}

/// 同步最近活动时间线（M8-WP05-T03）： oplog 尾部 + 冲突血缘按 `at_ns`
/// 降序合并截断。 空库返回空 vec（前端呈现动作邀请空态）。
///
/// # Errors
/// DB 错误 → `DesktopError::Internal`； oplog payload 非法 JSON 按空
/// 对象兜底（不 fail 整条时间线）。
#[tauri::command]
pub async fn sync_recent(
    state: State<'_, AppState>,
    args: SyncRecentArgs,
) -> DesktopResult<Vec<SyncRecentItem>> {
    let limit = args.limit.unwrap_or(30).min(100) as usize;
    let rows = state.store.pending_oplog().await?;
    let mut items: Vec<SyncRecentItem> = Vec::new();
    for r in rows.iter().rev().take(OPLOG_TAIL) {
        let payload: Value = serde_json::from_str(&r.payload).unwrap_or(Value::Null);
        let path = payload
            .get("path")
            .and_then(Value::as_str)
            .map_or_else(|| r.entity_id.clone(), str::to_string);
        let name = payload
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| basename_or(&path));
        let content_id = payload
            .get("content_id")
            .and_then(Value::as_str)
            .map(str::to_string);
        items.push(SyncRecentItem {
            at_ns: r.at_ns,
            dir: path
                .strip_suffix(&name)
                .map_or_else(String::new, str::to_string),
            name,
            origin_device: r.origin_device.clone(),
            op: r.op.clone(),
            conflict: false,
            content_id,
        });
    }
    for c in state.store.list_conflicts(limit as u32).await? {
        items.push(SyncRecentItem {
            at_ns: c.at_ns,
            dir: String::new(),
            name: basename_or(&c.base_path),
            origin_device: c.origin_device,
            op: "conflict".into(),
            conflict: true,
            content_id: None,
        });
    }
    items.sort_by(|a, b| b.at_ns.cmp(&a.at_ns).then_with(|| a.name.cmp(&b.name)));
    items.truncate(limit);
    Ok(items)
}

/// path 末段（无 `/` 时原样返回）。
fn basename_or(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
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
