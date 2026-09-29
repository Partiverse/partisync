//! 宿主状态与注权接口抽象（SPEC M7-WP01 §2.1）。
//!
//! SPEC §2.1 契约：**扩展宿主 crate 不直连 index / graph crate**——索引
//! 只读能力经本模块的 [`IndexRead`] trait 由组装期（`partisync-gateway`）
//! 注入实现，保持 crate 地图「只允许向下依赖」。
//!
//! SPEC §2.1 预案：若 trait 注入不足以表达某 capability，实现方可回退
//! `ext-host → graph` 直依赖（合法向下），须在实施 PR 说明并同步
//! ADR-0024 修订登记。T04 落地后 trait 表达力已被证（索引只读查询是
//! 单一无状态方法），预案未启用。

use std::sync::Arc;

/// 索引只读查询能力（宿主定义，注入方向：组装期 → 宿主）。
///
/// JSON 语义：入参与出参均为 JSON 字符串，与 MCP 工具面同口径
/// （「wasm component 即 MCP tool」的核心是 JSON 进 JSON 出）。失败
/// 以 `{"error": "..."}` 形态的 JSON 返回——不 panic、不 trap，让扩展
/// 自行决定呈现方式。
pub trait IndexRead: Send + Sync + 'static {
    /// 按查询串检索索引，返回 JSON 结果串。
    fn search(&self, query: &str) -> String;
}

/// 宿主侧 [`Store`](wasmtime::Store) 的状态载荷。
///
/// `index` 为 `None` 表示组装期未注入索引——此时声明了 `index.read`
/// 的扩展仍可加载（manifest 校验通过），但 host function 在被调用时
/// 返回错误 JSON（而非让整个实例化失败）：注权面由 manifest 声明决定，
/// 组装期是否真接线是宿主侧的实现事实，两者分离。
#[derive(Clone, Default)]
pub struct HostState {
    /// 索引只读能力（组装期注入；`None` = 未接线）。
    pub index: Option<Arc<dyn IndexRead>>,
}

impl HostState {
    /// 组装无索引能力的状态。
    #[must_use]
    pub fn without_index() -> Self {
        Self { index: None }
    }

    /// 组装带索引能力的状态。
    #[must_use]
    pub fn with_index(index: Arc<dyn IndexRead>) -> Self {
        Self { index: Some(index) }
    }
}
