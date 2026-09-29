//! 扩展装载与调用的高层封装（SPEC M7-WP01 §2.3 调用面）。
//!
//! 设计动机：gateway 不应直接依赖 wasmtime（重量级 crate，编译代价
//! 已由 ADR-0025 后果节登记）——扩展的 Store / 实例 / TypedFunc 全部
//! 封装在 [`ExtTool`] 内，gateway 只消费「按名调用、JSON 进 JSON 出」
//! 的干净 API。
//!
//! **工具调用约定**（冻结于 T04-B）：扩展 component 必须导出
//! `call: func(input: string) -> string`（沿 spike demo-tool world
//! `partisync:demo/demo-tool`，M6-WP04 §2.3「wasm component 即 MCP
//! tool」的 JSON 语义）。无 `call` 导出的 component 在装载期拒绝。
//!
//! **并发模型**：wasmtime 调用需 `&mut Store`，每个扩展实例以
//! `std::sync::Mutex` 串行化。MCP 工具调用是短平快操作（M6-WP04 评估
//! 实测 RTT p50 0.19 ms），阻塞 tokio worker 可接受；若 T05 基准复测
//! 显示长尾劣化，再迁 `spawn_blocking`（登记为开放问题，不预占）。

use std::collections::BTreeMap;
use std::path::Path;

use wasmtime::component::{Linker, TypedFunc};
use wasmtime::Store;

use crate::host_state::{HostState, IndexError};
use crate::manifest::Manifest;
use crate::{linker_for, load_component};

/// MCP 工具调用的扩展导出函数名（world `partisync:demo/demo-tool`）。
const TOOL_EXPORT: &str = "call";

/// 装载失败（加载期；含 [P14] 全部拒绝路径 + F-4 preflight）。
///
/// wasmtime 相关变体存 `String`（`wasmtime::Error` 是 anyhow 别名，
/// 不实现 `std::error::Error`，Display 已含完整 context 链）。
#[derive(Debug)]
pub enum LoadError {
    /// manifest 缺失 / 非法 / 校验失败。
    Manifest(crate::manifest::ManifestError),
    /// F-4：声明了 `index.read` 而组装期未接线——fail-closed。
    Preflight(IndexError),
    /// component 加载 / 编译失败。
    Component(String),
    /// component 未导出 `call(input) -> output`——不是 MCP 工具语义。
    MissingToolExport,
    /// linker 构造失败。
    Linker(String),
    /// 实例化失败（import 无法解析等）。
    Instantiate(String),
    /// 扩展目录扫描失败（IO / 命名）。
    Scan(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manifest(e) => write!(f, "manifest: {e}"),
            Self::Preflight(e) => write!(f, "preflight: {e}"),
            Self::Component(e) => write!(f, "component: {e}"),
            Self::MissingToolExport => {
                f.write_str("component 未导出 call(input: string) -> string（MCP 工具语义约定）")
            }
            Self::Linker(e) => write!(f, "linker: {e}"),
            Self::Instantiate(e) => write!(f, "instantiate: {e}"),
            Self::Scan(m) => write!(f, "scan: {m}"),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Manifest(e) => Some(e),
            Self::Preflight(e) => Some(e),
            _ => None,
        }
    }
}

/// 一个已装载、可调用的扩展工具。
pub struct ExtTool {
    manifest: Manifest,
    store: std::sync::Mutex<Store<HostState>>,
    func: TypedFunc<(String,), (String,)>,
}

impl std::fmt::Debug for ExtTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Store / TypedFunc 无 Debug；manifest 已足够标识
        f.debug_struct("ExtTool")
            .field("manifest", &self.manifest)
            .finish_non_exhaustive()
    }
}

/// 工具调用失败（运行期；与装载期 [`LoadError`] 分离）。
#[derive(Debug)]
pub enum CallError {
    /// 内部互斥锁中毒（持锁线程 panic；Store 毒化即此路径）。
    Poisoned,
    /// guest 调用失败（trap）。
    Trap(wasmtime::Error),
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Poisoned => f.write_str("ext tool lock poisoned"),
            Self::Trap(e) => write!(f, "guest trap: {e}"),
        }
    }
}

impl std::error::Error for CallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // `wasmtime::Error`（anyhow 别名）不实现 StdError，Trap 不挂 source
        None
    }
}

impl ExtTool {
    /// 装载并实例化一个扩展工具。
    ///
    /// 拒绝路径（按序）：manifest 三阶段校验 → [`HostState::preflight`]
    /// （F-4，fail-closed）→ component 编译 → linker 注入 → 实例化 →
    /// `call` 导出检查。**拒绝先于任何 host function 暴露**（[P14]）。
    pub fn load(
        wasm_path: impl AsRef<Path>,
        manifest_path: impl AsRef<Path>,
        state: HostState,
    ) -> Result<Self, LoadError> {
        let manifest = Manifest::load(&manifest_path).map_err(LoadError::Manifest)?;
        state.preflight(&manifest).map_err(LoadError::Preflight)?;
        let component =
            load_component(&wasm_path).map_err(|e| LoadError::Component(e.to_string()))?;
        let linker: Linker<HostState> =
            linker_for(crate::engine(), &manifest).map_err(|e| LoadError::Linker(e.to_string()))?;
        let mut store = Store::new(crate::engine(), state);
        let instance = linker
            .instantiate(&mut store, &component)
            .map_err(|e| LoadError::Instantiate(e.to_string()))?;
        let func = instance
            .get_typed_func::<(String,), (String,)>(&mut store, TOOL_EXPORT)
            .map_err(|_| LoadError::MissingToolExport)?;
        Ok(Self {
            manifest,
            store: std::sync::Mutex::new(store),
            func,
        })
    }

    /// manifest（工具名 + 声明面）。
    #[must_use]
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// 调用扩展工具（JSON 进 JSON 出）。
    ///
    /// 查询长度在 host function 层已被 `MAX_QUERY_BYTES` 兜底（F-12）；
    /// 这里不再重复检查——单点强制。
    pub fn call(&self, input_json: &str) -> Result<String, CallError> {
        let mut store = self.store.lock().map_err(|_| CallError::Poisoned)?;
        let (out,) = self
            .func
            .call(&mut *store, (input_json.to_owned(),))
            .map_err(CallError::Trap)?;
        Ok(out)
    }
}

/// 扩展注册表：按 manifest `tool_name` 索引的已装载扩展集合。
#[derive(Default)]
pub struct ExtRegistry {
    tools: BTreeMap<String, ExtTool>,
}

impl std::fmt::Debug for ExtRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtRegistry")
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ExtRegistry {
    /// 空注册表。
    #[must_use]
    pub fn empty() -> Self {
        Self {
            tools: BTreeMap::new(),
        }
    }

    /// 装载单个扩展并注册。
    ///
    /// # Errors
    /// manifest / preflight / 装载任一失败；或工具名与已注册扩展撞名
    /// （同批扩展之间的撞名由装载序保证拒绝——内建工具撞名已在
    /// `Manifest::validate` 拒绝）。
    pub fn register(
        &mut self,
        wasm_path: impl AsRef<Path>,
        manifest_path: impl AsRef<Path>,
        state: &HostState,
    ) -> Result<(), LoadError> {
        let tool = ExtTool::load(wasm_path, manifest_path, state.clone())?;
        let name = tool.manifest().tool_name.clone();
        if self.tools.contains_key(&name) {
            // BTreeMap::insert 会静默覆盖——撞名必须显式拒
            return Err(LoadError::Instantiate(format!(
                "duplicate extension tool name: {name}"
            )));
        }
        self.tools.insert(name, tool);
        Ok(())
    }

    /// 扫描扩展目录（SPEC §2.2 发现约定：`<dir>/*.json` manifest 驱动，
    /// 同名 `.wasm` 为 component）。单个扩展装载失败 → 整体失败
    /// （fail-closed；部分加载会让工具面呈现不可预期的半态）。
    pub fn scan(dir: impl AsRef<Path>, state: &HostState) -> Result<Self, LoadError> {
        let dir = dir.as_ref();
        let mut registry = Self::empty();
        let mut manifests: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| LoadError::Scan(format!("read {}: {e}", dir.display())))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        manifests.sort();
        for manifest_path in manifests {
            let stem = manifest_path.file_stem().ok_or_else(|| {
                LoadError::Scan(format!("bad manifest name: {}", manifest_path.display()))
            })?;
            let wasm_path = dir.join(format!("{}.wasm", stem.to_string_lossy()));
            registry.register(&wasm_path, &manifest_path, state)?;
        }
        Ok(registry)
    }

    /// 已注册工具数。
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// 是否为空。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// 已注册工具的 manifest 列表（`tool_name` 序）。
    #[must_use]
    pub fn manifests(&self) -> Vec<&Manifest> {
        self.tools.values().map(|t| t.manifest()).collect()
    }

    /// 按工具名调用。未注册 → `None`。
    pub fn call(&self, tool_name: &str, input_json: &str) -> Option<Result<String, CallError>> {
        self.tools.get(tool_name).map(|t| t.call(input_json))
    }
}
