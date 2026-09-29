//! 扩展系统接线（SPEC M7-WP01 §2.3 调用面，M7-WP01-T04 第 3 步）。
//!
//! 职责：
//! 1. [`IndexEngineReader`]——gateway 侧的 [`IndexRead`] 适配器，把
//!    `IndexEngine` 的 BM25 只读查询桥接给扩展宿主（SPEC §2.1 trait
//!    注入方向的 gateway 侧落地；ext-host 不直连 index crate 的契约
//!    由此保持）
//! 2. 扩展目录装载：`<data_root>/extensions/*.json`（SPEC §2.2 发现
//!    约定），启动期一次
//!
//! ## 同步桥接说明
//!
//! [`IndexRead::search`] 是同步 trait（wasmtime host function 无
//! async），而 `IndexEngine::bm25_only` 是 async——用
//! `tokio::task::block_in_place` + `Handle::block_on` 桥接。**要求
//! multi-thread tokio runtime**（`partisync-mcp` bin 的
//! `#[tokio::main]` 默认满足；current-thread runtime 下会 panic）。
//! 桥接代价记入 T05 基准复测观察项。

use std::path::PathBuf;
use std::sync::Arc;

use partisync_ext_host::{ExtRegistry, HostState, IndexError, IndexRead};
use partisync_index::search::bm25::Bm25Query;
use partisync_index::search::engine::IndexEngine;
use tokio::sync::RwLock;

/// gateway 侧索引只读适配器（组装期构造，见 [`build_index_reader`]）。
pub struct IndexEngineReader {
    engine: Arc<IndexEngine>,
}

impl IndexRead for IndexEngineReader {
    fn search(&self, query: &str) -> Result<String, IndexError> {
        let engine = self.engine.clone();
        let q = query.to_owned();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let result = engine
                    .bm25_only(Bm25Query {
                        query: q,
                        limit: 20,
                        include_transcript: false,
                    })
                    .await;
                match result {
                    // Bm25Hit 无 Serialize（index crate 侧未 derive）——手工
                    // 映射三字段（content_id / score / highlight）
                    Ok(res) => {
                        let hits: Vec<serde_json::Value> = res
                            .hits
                            .iter()
                            .map(|h| {
                                serde_json::json!({
                                    "content_id": h.content_id,
                                    "score": h.score,
                                    "highlight": h.highlight,
                                })
                            })
                            .collect();
                        serde_json::to_string(&hits)
                            .map_err(|e| IndexError::Backend(format!("serialize: {e}")))
                    }
                    Err(e) => {
                        // 消息契约（trait doc）：原样进 guest 可见错误
                        // JSON——索引错误消息不含路径（tantivy 报错为
                        // 索引内部语义），保留主要诊断信息。
                        Err(IndexError::Backend(format!("bm25: {e}")))
                    }
                }
            })
        })
    }
}

/// 从已安装的索引引擎构造适配器。
#[must_use]
pub fn build_index_reader(engine: Arc<IndexEngine>) -> IndexEngineReader {
    IndexEngineReader { engine }
}

/// 扩展目录默认路径：`~/.partisync/extensions`。
///
/// 用 `home_dir` 而非 `data_local_dir`（macOS 后者为
/// `~/Library/Application Support`）：与安装脚本、桌面壳 UI 文案、
/// 用户直觉一致——T04 第 3 步端到端验证暴露的不一致（装错目录 =
/// 空注册表）。
#[must_use]
pub fn default_extensions_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".partisync")
        .join("extensions")
}

/// 装载扩展注册表。
///
/// 目录不存在 → 空注册表（扩展是可选增强，不阻塞主服务）；扫描 /
/// 装载失败 → `Err`（fail-closed，由调用方决定降级策略——`run_mcp_server`
/// 降级为空注册表 + stderr 告警，扩展目录损坏不拖垮 MCP 主服务）。
/// 返回 (注册表, 实际扫描目录)——目录随 `ext_list` 载荷回带（可诊断性：
/// 桌面壳端直接核对 partisync-mcp 视角的路径，T04 端到端实测曾因
/// data_local_dir vs home_dir 不一致装错目录）。
pub fn load_registry(
    dir: &PathBuf,
    index: Option<Arc<IndexEngine>>,
) -> Result<(ExtRegistry, PathBuf), String> {
    if !dir.exists() {
        return Ok((ExtRegistry::empty(), dir.clone()));
    }
    let state = match index {
        Some(engine) => HostState::with_index(Arc::new(build_index_reader(engine))),
        None => HostState::without_index(),
    };
    ExtRegistry::scan(dir, &state)
        .map(|r| (r, dir.clone()))
        .map_err(|e| e.to_string())
}

/// 组装期工具：供 `McpServerState::install_ext_registry` 使用的共享形态
/// （注册表 + 实际扫描目录，后者随 `ext_list` 载荷回带）。
pub type SharedRegistry = Arc<RwLock<Option<(Arc<ExtRegistry>, PathBuf)>>>;
