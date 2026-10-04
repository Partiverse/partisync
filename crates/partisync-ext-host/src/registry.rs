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

/// 单次工具调用 fuel 预算（M8-WP06 SPEC §2.2：编译期常量，不进 manifest
/// schema——P14 注权面零改动）。按 demo-tool 实测量级 10× 余量定。
pub const FUEL_BUDGET: u64 = 1_000_000_000;

/// 单次工具调用 epoch 预算（毫秒；/EPOCH_TICK_MS = deadline ticks）。
/// 与 gateway `EXT_CALL_TIMEOUT`（10s）对齐——内层 epoch 先到先终止，
/// 外层 timeout 保留为二层防御。
pub const DEADLINE_BUDGET_MS: u64 = 10_000;

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
    /// component 未导出 `call(input) -> output`——不是 MCP 工具语义
    /// （携带 get_typed_func 的原始错误，区分「无导出」与「签名不匹配」；
    /// PR #35 审查 P2-4）。
    MissingToolExport(String),
    /// linker 构造失败。
    Linker(String),
    /// 实例化失败（import 无法解析等）。
    Instantiate(String),
    /// 扩展目录扫描失败（IO / 命名）。
    Scan(String),
    /// 同批扩展工具名撞名（PR #35 审查 P2-3：语义独立于 Instantiate，
    /// gateway 按变体匹配时不会误报实例化失败）。
    Duplicate(String),
    /// [P21] 同名 `.minisig` 缺失（SPEC M9-WP04 §2.2；无豁免通道）。
    /// 携带扩展路径定位。
    Unsigned(String),
    /// [P21] `.minisig` 格式非法或对全部锚定公钥验证失败（篡改字节 /
    /// 未知钥签名 / 非 legacy-allowed 算法）。携带扩展路径定位。
    BadSignature(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manifest(e) => write!(f, "manifest: {e}"),
            Self::Preflight(e) => write!(f, "preflight: {e}"),
            Self::Component(e) => write!(f, "component: {e}"),
            Self::MissingToolExport(e) => write!(
                f,
                "component 未导出 call(input: string) -> string（MCP 工具语义约定）：{e}"
            ),
            Self::Linker(e) => write!(f, "linker: {e}"),
            Self::Instantiate(e) => write!(f, "instantiate: {e}"),
            Self::Scan(m) => write!(f, "scan: {m}"),
            Self::Duplicate(m) => write!(f, "duplicate: {m}"),
            Self::Unsigned(m) => write!(
                f,
                "extension not signed (no `<name>.minisig`; loading is verify-mandatory): {m}"
            ),
            Self::BadSignature(m) => write!(f, "extension signature invalid: {m}"),
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
    /// 内部互斥锁中毒（持锁线程 panic——IndexRead 实现违约 panic 或
    /// wasmtime 内部 panic；可达路径，防御性保留。PR #34 审查更正：
    /// 同步路径 guest trap 不产生 panic，与 Store「毒化」无关）。
    Poisoned,
    /// guest 调用失败（trap；存 Display 消息——`wasmtime::Error` 为
    /// anyhow 别名不实现 StdError，且避免 wasmtime 类型泄漏进公共 API，
    /// PR #35 审查 P2-5）。
    Trap(String),
    /// epoch deadline 到——guest 被 wasmtime 强制中断（M8-WP06：真终止，
    /// 非 timeout 假终止；线程随 trap 释放）。
    Deadline,
    /// fuel 预算耗尽——单调用计算量超限（M8-WP06 SPEC §2.2 固定常量）。
    Fuel,
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Poisoned => f.write_str("ext tool lock poisoned"),
            Self::Trap(e) => write!(f, "guest call failed: {e}"),
            Self::Deadline => f.write_str("extension deadline exceeded (epoch interruption)"),
            Self::Fuel => f.write_str("extension fuel budget exhausted"),
        }
    }
}

impl std::error::Error for CallError {}

/// [P21] 装载期验签（SPEC M9-WP04 §2.2）：读扩展字节 + 同名 `.minisig`
/// （`wasm_path.with_extension("minisig")`——对 `.wasm`/`.wat` 同规则，
/// 签名对象 = 传入编译器的同一文件字节），逐锚钥验证，任一通过即放行。
///
/// wasm 字节读取失败归 [`LoadError::Component`]（编译前置 IO，非签名面）；
/// 缺签 → [`LoadError::Unsigned`]；坏签/未知钥 → [`LoadError::BadSignature`]
/// （Display 携带路径定位）。
fn verify_signature(
    wasm_path: &Path,
    anchors: &[minisign_verify::PublicKey],
) -> Result<(), LoadError> {
    let content = std::fs::read(wasm_path)
        .map_err(|e| LoadError::Component(format!("read {}: {e}", wasm_path.display())))?;
    let sig_path = wasm_path.with_extension("minisig");
    let sig = match minisign_verify::Signature::from_file(&sig_path) {
        Ok(sig) => sig,
        // 缺签（NotFound）与坏签分野——[P21] 双变体错误面
        Err(minisign_verify::Error::IoError(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(LoadError::Unsigned(wasm_path.display().to_string()));
        }
        Err(e) => {
            return Err(LoadError::BadSignature(format!(
                "{}: {e}",
                sig_path.display()
            )));
        }
    };
    crate::signature::verify(&content, &sig, anchors)
        .map_err(|e| LoadError::BadSignature(format!("{}: {e}", wasm_path.display())))
}

impl ExtTool {
    /// 装载并实例化一个扩展工具（产品路径：锚集 = 内嵌发布双钥
    /// [`crate::signature::ANCHOR_PUBKEYS`]）。
    ///
    /// 拒绝路径（按序）：manifest 三阶段校验 → **[P21] 验签**（缺签/
    /// 坏签在 component 字节进入 wasmtime 编译器之前拒绝）→
    /// [`HostState::preflight`]（F-4，fail-closed）→ component 编译 →
    /// linker 注入 → 实例化 → `call` 导出检查。**拒绝先于任何 host
    /// function 暴露**（[P14]）。无 unsigned 豁免开关。
    pub fn load(
        wasm_path: impl AsRef<Path>,
        manifest_path: impl AsRef<Path>,
        state: HostState,
    ) -> Result<Self, LoadError> {
        Self::load_with_anchors(
            wasm_path,
            manifest_path,
            state,
            &crate::signature::anchored_pubkeys(),
        )
    }

    /// 锚集参数化的装载入口（[`Self::load`] 的核心体）。
    ///
    /// 产品路径恒走 [`Self::load`]（内嵌发布双钥，无任何开关/env/豁免，
    /// [P21](c) 字面）；参数化面是 SPEC M9-WP04 §2.3「测试面自足」/§4
    /// 「测试钥签名路径」的实现前提——验签恒强制，仅锚集来源可换（与
    /// ADR-0030 修订触发「专钥分域」方向兼容），非 unsigned 豁免通道。
    pub fn load_with_anchors(
        wasm_path: impl AsRef<Path>,
        manifest_path: impl AsRef<Path>,
        state: HostState,
        anchors: &[minisign_verify::PublicKey],
    ) -> Result<Self, LoadError> {
        let wasm_path = wasm_path.as_ref();
        let manifest = Manifest::load(&manifest_path).map_err(LoadError::Manifest)?;
        verify_signature(wasm_path, anchors)?;
        state.preflight(&manifest).map_err(LoadError::Preflight)?;
        let component =
            load_component(wasm_path).map_err(|e| LoadError::Component(e.to_string()))?;
        let linker: Linker<HostState> =
            linker_for(crate::engine(), &manifest).map_err(|e| LoadError::Linker(e.to_string()))?;
        let mut store = Store::new(crate::engine(), state);
        // M8-WP06：epoch_interruption(true) 下 Store 默认 deadline = 当前
        // epoch（tick 一推进即 interrupt trap）——装载期先设大 delta
        // （u64::MAX/2；deadline = current + delta，直接传 u64::MAX 会
        // 加法溢出），调用期由 call 前重置为预算 ticks；fuel 同理在
        // consume_fuel(true) 下初始 = 0（canonical ABI 起始 shim 计费），
        // 实例化前预注入。
        store.set_epoch_deadline(u64::MAX / 2);
        store
            .set_fuel(FUEL_BUDGET)
            .map_err(|e| LoadError::Instantiate(format!("fuel init: {e}")))?;
        let instance = linker
            .instantiate(&mut store, &component)
            .map_err(|e| LoadError::Instantiate(e.to_string()))?;
        let func = instance
            .get_typed_func::<(String,), (String,)>(&mut store, TOOL_EXPORT)
            .map_err(|e| LoadError::MissingToolExport(e.to_string()))?;
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
        // M8-WP06（SPEC §2.1/§2.2）：每次调用前重置终止预算——epoch
        // deadline（100ms tick × 预算 ms）+ fuel 固定常量，先到者终止。
        store.set_epoch_deadline(DEADLINE_BUDGET_MS / crate::EPOCH_TICK_MS);
        store
            .set_fuel(FUEL_BUDGET)
            .map_err(|e| CallError::Trap(format!("fuel init: {e}")))?;
        let result = self
            .func
            .call(&mut *store, (input_json.to_owned(),))
            .map_err(|e| {
                // trap 文案在 anyhow 根因层（顶层只有 wasm backtrace）：
                // epoch 中断 = "wasm trap: interrupt"；fuel 耗尽 =
                // "all fuel consumed by WebAssembly"
                let msg = e.to_string();
                let cause = e.root_cause().to_string();
                if cause.contains("fuel") {
                    CallError::Fuel
                } else if cause.contains("interrupt")
                    || cause.contains("epoch")
                    || cause.contains("deadline")
                {
                    CallError::Deadline
                } else {
                    CallError::Trap(msg)
                }
            });
        let (out,) = result?;
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

    /// 装载单个扩展并注册（产品锚 = 内嵌发布双钥）。
    ///
    /// # Errors
    /// manifest / 验签 / preflight / 装载任一失败；或工具名与已注册扩展
    /// 撞名（同批扩展之间的撞名由装载序保证拒绝——内建工具撞名已在
    /// `Manifest::validate` 拒绝）。
    pub fn register(
        &mut self,
        wasm_path: impl AsRef<Path>,
        manifest_path: impl AsRef<Path>,
        state: &HostState,
    ) -> Result<(), LoadError> {
        Self::register_with_anchors(
            self,
            wasm_path,
            manifest_path,
            state,
            &crate::signature::anchored_pubkeys(),
        )
    }

    /// 锚集参数化的注册入口（[`Self::register`] 核心体；参数化语义同
    /// [`ExtTool::load_with_anchors`]——非豁免通道）。
    ///
    /// # Errors
    /// 同 [`Self::register`]。
    pub fn register_with_anchors(
        &mut self,
        wasm_path: impl AsRef<Path>,
        manifest_path: impl AsRef<Path>,
        state: &HostState,
        anchors: &[minisign_verify::PublicKey],
    ) -> Result<(), LoadError> {
        let tool = ExtTool::load_with_anchors(wasm_path, manifest_path, state.clone(), anchors)?;
        let name = tool.manifest().tool_name.clone();
        if self.tools.contains_key(&name) {
            // BTreeMap::insert 会静默覆盖——撞名必须显式拒
            return Err(LoadError::Duplicate(format!(
                "extension tool name `{name}` already registered"
            )));
        }
        self.tools.insert(name, tool);
        Ok(())
    }

    /// 扫描扩展目录（SPEC §2.2 发现约定：`<dir>/*.json` manifest 驱动，
    /// 同名 `.wasm` 为 component；[P21] 起另要求同名 `.minisig` 签名
    /// 文件，缺签/坏签/孤儿签名均装载期拒）。单个扩展装载失败 → 整体
    /// 失败（fail-closed；部分加载会让工具面呈现不可预期的半态）。
    pub fn scan(dir: impl AsRef<Path>, state: &HostState) -> Result<Self, LoadError> {
        Self::scan_with_anchors(dir, state, &crate::signature::anchored_pubkeys())
    }

    /// 锚集参数化的扫描入口（[`Self::scan`] 核心体；参数化语义同
    /// [`ExtTool::load_with_anchors`]——非豁免通道）。
    ///
    /// # Errors
    /// 同 [`Self::scan`]。
    pub fn scan_with_anchors(
        dir: impl AsRef<Path>,
        state: &HostState,
        anchors: &[minisign_verify::PublicKey],
    ) -> Result<Self, LoadError> {
        let dir = dir.as_ref();
        let mut registry = Self::empty();
        // P2-2：read_dir 迭代中的 IO 错误不吞（与 fail-closed 文档一致）
        let entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| LoadError::Scan(format!("read {}: {e}", dir.display())))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| LoadError::Scan(format!("iterate {}: {e}", dir.display())))?;
        // P2-1：孤儿 `.wasm`（无同名 `.json`）显式拒——与「manifest 缺失
        // 的 component 加载即拒」（SPEC §2.3）对齐，坏扩展静默消失会让
        // 工具面呈现不可预期的半态。
        for entry in &entries {
            let p = entry.path();
            if p.extension().is_some_and(|x| x == "wasm") {
                let has_manifest = p.with_extension("json").exists();
                if !has_manifest {
                    return Err(LoadError::Scan(format!(
                        "orphan component (no manifest): {}",
                        p.display()
                    )));
                }
            }
            // [P21]（SPEC M9-WP04 §2.2）：孤儿 `.minisig`（有签名无同名
            // `.wasm`）显式拒——坏文件静默消失会让工具面呈半态（沿孤儿
            // `.wasm` P2-1 判例）。
            if p.extension().is_some_and(|x| x == "minisig") {
                let has_wasm = p.with_extension("wasm").exists();
                if !has_wasm {
                    return Err(LoadError::Scan(format!(
                        "orphan signature (no component): {}",
                        p.display()
                    )));
                }
            }
        }
        let mut manifests: Vec<_> = entries
            .into_iter()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        manifests.sort();
        for manifest_path in manifests {
            let stem = manifest_path.file_stem().ok_or_else(|| {
                LoadError::Scan(format!("bad manifest name: {}", manifest_path.display()))
            })?;
            let wasm_path = dir.join(format!("{}.wasm", stem.to_string_lossy()));
            registry.register_with_anchors(&wasm_path, &manifest_path, state, anchors)?;
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
