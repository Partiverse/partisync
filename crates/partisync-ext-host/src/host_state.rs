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

use std::fmt;
use std::sync::Arc;

/// 查询串长度上限（F-12 登记项）。`search` 是宿主代执行的昂贵操作，
/// guest 可自由构造超长查询；无上限时单次调用即可放大为宿主侧的大串
/// 解析 + 全量扫描。
///
/// 取 8 KiB：远高于任何真实查询串（自然语言查询通常数百字节），又能
/// 装进探针 fixture 的单页 wasm 内存（64 KiB），使限额本身可被端到端
/// 探针验证而非仅单测。
pub const MAX_QUERY_BYTES: usize = 8 * 1024;

/// 索引查询失败（映射到 wire 层的 `{"error": "..."}` JSON）。
///
/// **F-2 的存在理由**：`IndexRead` 早期签名为 `-> String`，真实实现遇
/// 索引不可用只能 panic。而 wasmtime **同步**路径下，宿主函数内的
/// panic 不会被转成 trap——它以 **Rust panic 形态越过 wasm 边界直接
/// 传播进 gateway 调用方线程**（wasmtime `traphandlers.rs` 对宿主
/// panic 执行 `resume_unwind`）；`panic = "abort"` 构建下更是直接中止
/// 整个宿主进程。两种形态都把实现方的内部失败变成了不可控的宿主侧
/// 事故。`Result` 化让失败成为可归因的普通值，在 wasm 边界处消化。
/// （注：真正的「Store 毒化」仅存在于 wasmtime 的 concurrent component
/// API——本 crate 未使用；若未来迁移该 API，此处契约需重审。）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexError {
    /// 查询串超过 [`MAX_QUERY_BYTES`]。
    QueryTooLong { len: usize, max: usize },
    /// 组装期未注入索引实现（fail-open 态，见 [`HostState::index`]）。
    NotWired,
    /// 底层索引实现报告的失败（消息由实现方给出）。
    Backend(String),
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueryTooLong { len, max } => {
                write!(f, "query too long: {len} bytes (max {max})")
            }
            Self::NotWired => f.write_str("index.read unavailable"),
            Self::Backend(m) => write!(f, "index backend error: {m}"),
        }
    }
}

impl std::error::Error for IndexError {}

/// 索引只读查询能力（宿主定义，注入方向：组装期 → 宿主）。
///
/// JSON 语义：入参与出参均为 JSON 字符串，与 MCP 工具面同口径
/// （「wasm component 即 MCP tool」的核心是 JSON 进 JSON 出）。失败以
/// [`IndexError`] 返回，宿主层映射为 `{"error": "..."}` JSON——**不得
/// panic**：同步路径下宿主函数内的 panic 会以 Rust panic 形态越过
/// wasm 边界直接传播进 gateway 调用方线程（`panic = "abort"` 构建下
/// 直接中止宿主进程），详见 [`IndexError`] 文档。
///
/// # 实现方消息契约（T04-B 冻结）
///
/// [`IndexError::Backend`] 的消息会**原样进入 guest 可见的错误 JSON**——
/// 不得包含内部路径 / 接线细节等不可信 guest 不应获得的实现信息
/// （T04-A 审查 F-8 教训的延伸）；此类诊断信息应由实现方自行记日志。
pub trait IndexRead: Send + Sync + 'static {
    /// 按查询串检索索引，返回 JSON 结果串。
    ///
    /// 实现方契约（T04-B 冻结）：**必须返回 `Err` 而非 panic**。
    fn search(&self, query: &str) -> Result<String, IndexError>;
}

/// 宿主侧 [`Store`](wasmtime::Store) 的状态载荷。
///
/// `index` 为 `None` 表示组装期未注入索引。**T04-B 处置 F-4**：组装期
/// 应在加载前调用 [`preflight`](Self::preflight) 自检——声明了
/// `index.read` 却未接线时**加载期拒**，而非让扩展静默可实例化、每次
/// 调用拿错误 JSON（fail-open 无检测路径）。
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

    /// 组装期自检（F-4）：manifest 声明了 `index.read` 而宿主未注入实现
    /// 时返回 `Err`——调用方应在**加载期**据此拒绝扩展，使 fail-open
    /// 态变成 fail-closed。
    ///
    /// 与 [P14]「注入面 = 声明面」不冲突：声明了 `index.read` 的 manifest
    /// 校验通过、宿主**应当**注入；未接线是组装期遗漏，应在装载时就
    /// 暴露，而非留给运行期静默降级。
    pub fn preflight(&self, manifest: &crate::manifest::Manifest) -> Result<(), IndexError> {
        let wants_index = manifest
            .capabilities
            .contains(&crate::manifest::Capability::IndexRead);
        if wants_index && self.index.is_none() {
            return Err(IndexError::NotWired);
        }
        Ok(())
    }
}
