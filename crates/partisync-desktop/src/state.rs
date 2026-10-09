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
    /// 查询向量嵌入器（M8-WP05-T01；**独立懒加载**——`index` 与本
    /// `embedder` 是两个 OnceCell：BM25 检索不碰模型，语义检索才加载，
    /// 冷启动路径零模型加载（function-map §4-N1 硬线）。
    embedder: tokio::sync::OnceCell<Arc<QueryEmbedder>>,
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
            embedder: tokio::sync::OnceCell::new(),
        })
    }

    /// 索引句柄懒加载： 首次调用打开 `IndexEngine`， 后续调用复用。
    ///
    /// M11-WP02-T03（D5 修复）：`open_or_create` → `open_read_only`——
    /// 桌面对索引为纯读消费者，不再无条件建 tantivy `IndexWriter`（与
    /// sidecar/CLI 的写者按 D19/ADR-0032 决策 2 并存）。索引不存在
    /// （`meta.json` 缺失）→ 结构化错误，指引先建索引。
    ///
    /// # Errors
    /// `IndexEngine::open_read_only` 失败（索引不存在 / 无 tantivy /
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
            .get_or_try_init(|| async move {
                // 新装机首启（bm25/meta.json 缺失）：一次性引导空索引——
                // 短暂写者即建即弃，随后纯只读打开（桌面不持久持写者，
                // ADR-0032 决策 2）；既有索引则直接只读打开。
                if !cfg.index_root.join("bm25").join("meta.json").exists() {
                    // 建空索引后立即弃引擎（写者随之释放）——桌面不持久持写者
                    let boot = IndexEngine::open_or_create(cfg.clone())?;
                    drop(boot);
                }
                IndexEngine::open_read_only(cfg).map(Arc::new)
            })
            .await?
            .clone();
        Ok(engine)
    }

    /// 查询嵌入器懒加载（M8-WP05-T01）：首次**语义**检索时加载
    /// fastembed BGE-small-zh-v1.5（与 M6-D67-T03 EvalRunner 同模型，
    /// 512d → `VectorKind::TextDenseZh512`）；BM25 检索不触碰本方法。
    ///
    /// 首次加载需下载/读盘模型权重（秒级，缓存于
    /// `<index_root>/embed-cache`）——前端在切换语义模式的首次检索
    /// 显示 loading 态（SPEC §3 三态）。
    ///
    /// # Errors
    /// 模型初始化 / 权重缓存不可写 → `DesktopError::Index`
    /// （`kind:"Index"`，前端按审计约定显示错误态）。
    pub async fn embedder(&self) -> Result<Arc<QueryEmbedder>, DesktopError> {
        let cache_dir = self.index_root.join("embed-cache");
        let embedder: Arc<QueryEmbedder> = self
            .embedder
            .get_or_try_init(|| async move {
                std::fs::create_dir_all(&cache_dir).map_err(|e| {
                    DesktopError::Index(format!("embed cache dir 创建失败 {cache_dir:?}: {e}"))
                })?;
                let opts =
                    fastembed::TextInitOptions::new(fastembed::EmbeddingModel::BGESmallZHV15)
                        .with_cache_dir(cache_dir)
                        .with_show_download_progress(false);
                let model = fastembed::TextEmbedding::try_new(opts)
                    .map_err(|e| DesktopError::Index(format!("嵌入模型初始化失败: {e}")))?;
                Ok::<Arc<QueryEmbedder>, DesktopError>(Arc::new(QueryEmbedder {
                    model: std::sync::Mutex::new(model),
                }))
            })
            .await?
            .clone();
        Ok(embedder)
    }
}

/// 查询嵌入器（M8-WP05-T01；fastembed BGE-small-zh-v1.5 → 512d，与
/// `VectorKind::TextDenseZh512` 维度对齐——M6-D67-T03 判例：新 embedding
/// 路径不用 legacy 768d `TextDense`）。
pub struct QueryEmbedder {
    /// fastembed 推理器内部可变（`embed` 需 `&mut self`）——`Mutex`
    /// 串行化并发语义检索（桌面壳单窗口，串行度可接受）。
    model: std::sync::Mutex<fastembed::TextEmbedding>,
}

impl QueryEmbedder {
    /// 嵌入单条查询。
    ///
    /// # Errors
    /// 推理失败（输入超长 / 模型内部错）→ `DesktopError::Index`。
    pub fn embed_query(&self, query: &str) -> Result<Vec<f32>, DesktopError> {
        let vectors = self
            .model
            .lock()
            .map_err(|_| DesktopError::Index("嵌入器锁中毒".into()))?
            .embed(vec![query], None)
            .map_err(|e| DesktopError::Index(format!("查询嵌入失败: {e}")))?;
        vectors
            .into_iter()
            .next()
            .ok_or_else(|| DesktopError::Index("查询嵌入返回空向量".into()))
    }
}

impl std::fmt::Debug for QueryEmbedder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("QueryEmbedder(bge-small-zh-v1.5)")
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
            .field("embedder_loaded", &self.embedder.initialized())
            .finish()
    }
}
