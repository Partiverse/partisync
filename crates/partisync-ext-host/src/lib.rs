//! PartiSync WASM 扩展宿主（SPEC M7-WP01 / ADR-0025）。
//!
//! 宿主形态「wasm component 即 MCP tool」：扩展实现既有 MCP 工具语义
//! （JSON 入参 → JSON 出参），经宿主加载后被 `partisync-mcp` 工具面与
//! 桌面壳 `mcp_call` 无差别调用（T04 接线）。
//!
//! `load_component()` + 拒绝路径见 [`manifest::Manifest::load`]。

use std::path::Path;
use std::sync::OnceLock;

pub mod inject;
pub mod manifest;
pub use inject::linker_for;
pub use manifest::{Capability, Manifest, ManifestError, ValidateError};

use wasmtime::component::{Component, Linker};
use wasmtime::{Cache, CacheConfig, Config, Engine};

static ENGINE: OnceLock<Engine> = OnceLock::new();

/// 进程级 Engine 单例：Cranelift 默认 opt + wasmtime 磁盘编译缓存
/// （同 component 重复加载跳过编译；缓存未命中路径 p95 超冷启动预算
/// 是已知行为，ADR-0025 后果节）。
///
/// 缓存盘不可写时退化为无缓存运行——冷启动预算前提失效但正确性不受
/// 影响；Engine 本体初始化失败视为不可恢复环境错误。
pub fn engine() -> &'static Engine {
    ENGINE.get_or_init(|| {
        let mut cfg = Config::default();
        if let Ok(cache) = Cache::new(CacheConfig::default()) {
            cfg.cache(Some(cache));
        }
        Engine::new(&cfg).expect("wasmtime Engine 初始化失败（不可恢复）")
    })
}

/// 默认拒权 linker：未注册任何宿主函数/接口——未注权 component 缺
/// import 时实例化即拒（[P13]，错误文本不含宿主路径/env）。per-call
/// 细粒度拒绝与白名单注入随 T02 manifest 机制落地。
pub fn deny_linker() -> Linker<()> {
    Linker::new(engine())
}

/// 从文件加载 component。扩展发现约定 `<data_root>/extensions/*.wasm`
/// （SPEC M7-WP01 §2.2；manifest 校验在 T02 于实例化前前置）。
pub fn load_component(path: impl AsRef<Path>) -> wasmtime::Result<Component> {
    Component::from_file(engine(), path)
}

/// 整合加载路径（PR-B 新增）：manifest 校验 → component 加载 →
/// linker 注入。三阶段任一失败 = 加载拒，调用方不得实例化。
///
/// `<wasm_path>` 与 `<manifest_path>` 由调用方解析（SPEC §2.2 发现约定：
/// 同目录同名 `<name>.wasm` + `<name>.json`；命名约定由 T04 PR-A 在
/// `partisync-ext-host::discovery` 模块落地，本函数不耦合命名）。
pub fn load_with_manifest(
    wasm_path: impl AsRef<Path>,
    manifest_path: impl AsRef<Path>,
) -> Result<(Component, Linker<()>), ManifestError> {
    let manifest = Manifest::load(manifest_path)?;
    // 阶段 2/3 失败归 `ManifestError::Parse` 桶位：wasmtime 错误为
    // `anyhow::Error`，无法塞进 `Io(std::io::Error)` 类型槽；借用
    // 「parse 阶段失败」桶位承载，语义上属于「加载期失败」（PR-A
    // ManifestError 三变体口径不变）。T04 若需细分（component 编译
    // 错 vs linker 注入错），新增 `ManifestError::Component(String)`
    // 与 `Linker(String)` 变体；当前 PR 借桶登记偏差。
    let component = load_component(&wasm_path)
        .map_err(|e| ManifestError::Parse(format!("component load failed: {e}")))?;
    let linker = linker_for(engine(), &manifest)
        .map_err(|e| ManifestError::Parse(format!("linker inject failed: {e}")))?;
    Ok((component, linker))
}
