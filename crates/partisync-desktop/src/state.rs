//! PartiSync 桌面壳运行时状态（SPEC M6-WP03 §3 T03）
//!
//! [`AppState`] 持有桌面壳启动期打开的 3 个底层句柄， 由
//! `tauri::Builder::manage` 注入 IPC command：
//! - `Store`（partisync-graph）： 持久化的资产图谱
//! - `ChunkStore`（partisync-cas）： 块库 + 去重统计
//! - `IndexEngine`（partisync-index， `OnceCell` 懒加载）： BM25 +
//!   向量索引， 仅在首次 `search` IPC 时打开， 失败返回
//!   `DesktopError::Index`（SPEC §2.6 IndexUnavailable）

use std::path::PathBuf;
use std::sync::Arc;

use partisync_cas::ChunkStore;
use partisync_graph::store::Store;
use partisync_index::{IndexEngine, IndexEngineConfig};

use crate::error::DesktopError;
use crate::mcp_sidecar::McpSidecar;

/// 应用状态： 仓储句柄 + 块库 + 索引（懒加载）+ MCP 侧车。
#[derive(Clone)]
pub struct AppState {
    /// PartiGraph 仓储（持久化的资产图谱 + 作业列表）。
    pub store: Arc<Store>,
    /// 块库（chunk-level dedup + saved_bytes 统计）。
    pub cas: Arc<ChunkStore>,
    /// 索引根目录（`IndexEngine` 懒加载的输入）。
    pub index_root: PathBuf,
    /// MCP 侧车（懒 spawn `partisync-mcp` stdio JSON-RPC 子进程）。
    pub mcp_sidecar: Arc<McpSidecar>,
    /// 索引引擎（仅在首次 `search` IPC 时 `get_or_try_init`）。
    index: tokio::sync::OnceCell<Arc<IndexEngine>>,
}

impl AppState {
    /// 打开仓储 + 块库 + MCP 侧车； 索引留空， 首次 search IPC 时再懒加载。
    ///
    /// # Errors
    /// `Store::open` 或 `ChunkStore::open` 失败 → 返回 `DesktopError::Internal`。
    pub async fn open(
        db_path: PathBuf,
        cas_dir: PathBuf,
        index_root: PathBuf,
    ) -> Result<Self, DesktopError> {
        let store = Store::open(&db_path).await?;
        let cas = ChunkStore::open(&cas_dir).await?;
        let mcp_sidecar = Arc::new(
            McpSidecar::for_app_state(db_path.clone(), index_root.clone()).map_err(
                |e| match e {
                    DesktopError::Sidecar(msg) => DesktopError::Sidecar(msg),
                    other => other,
                },
            )?,
        );
        Ok(Self {
            store: Arc::new(store),
            cas: Arc::new(cas),
            index_root,
            mcp_sidecar,
            index: tokio::sync::OnceCell::new(),
        })
    }

    /// 索引句柄懒加载： 首次调用打开 `IndexEngine`， 后续调用复用。
    ///
    /// # Errors
    /// `IndexEngine::open_or_create` 失败（无 tantivy / 索引目录不可写 /
    /// 索引损坏） → 返回 `DesktopError::Index`， 对应 SPEC §2.6
    /// `IndexUnavailable`。
    pub async fn index(&self) -> Result<Arc<IndexEngine>, DesktopError> {
        let cfg = IndexEngineConfig {
            index_root: self.index_root.clone(),
            enable_reranker: false,
            reranker_model_dir: None,
        };
        let engine: Arc<IndexEngine> = self
            .index
            .get_or_try_init(|| async move { IndexEngine::open_or_create(cfg).map(Arc::new) })
            .await?
            .clone();
        Ok(engine)
    }
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("store", &"<Store>")
            .field("cas", &"<ChunkStore>")
            .field("index_root", &self.index_root)
            .field("mcp_sidecar", &"<McpSidecar>")
            .field("index_loaded", &self.index.initialized())
            .finish()
    }
}
