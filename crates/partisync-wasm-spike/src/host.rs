//! 宿主面最小实现：engine + 两种 linker + demo tool 的裸 Component API 调用。
//!
//! - [`deny_linker`]：零 import —— SPEC §2.3「沙箱默认面」的 spike 形态
//!   （capability 默认全拒；[P13] 探针用它证明拒权可达）。
//! - demo tool 经同一 deny_linker 实例化：component 无宿主能力依赖时
//!   默认拒权不影响工具调用面（R6 的「默认拒 vs MCP 语义」spike 级回答）。
//!
//! 不用 bindgen! 宏：单函数 world 的 spike 用 `get_typed_func` 足够，
//! 生成的脚手架反而掩盖宿主面的真实大小。

use wasmtime::component::{Component, Linker, TypedFunc};
use wasmtime::error::Context;
use wasmtime::{Config, Engine, Result, Store};

/// 已实例化的 demo tool：`call(input) -> output`（world `partisync:demo/demo-tool`）。
pub struct DemoTool {
    store: Store<()>,
    func: TypedFunc<(String,), (String,)>,
}

impl DemoTool {
    pub fn instantiate(
        engine: &Engine,
        component: &Component,
        linker: &Linker<()>,
    ) -> Result<Self> {
        let mut store = Store::new(engine, ());
        let instance = linker.instantiate(&mut store, component)?;
        let func = instance
            .get_typed_func::<(String,), (String,)>(&mut store, "call")
            .context("export `call` 未找到")?;
        Ok(Self { store, func })
    }

    pub fn call(&mut self, input: &str) -> Result<String> {
        // wasmtime 47：TypedFunc::post_return 已废弃（无效果，无需调用）
        let (out,) = self.func.call(&mut self.store, (input.to_owned(),))?;
        Ok(out)
    }
}

/// Engine：Cranelift 默认 opt + wasmtime 磁盘编译缓存（同 component 重复
/// 加载跳过编译）。缓存关闭/未命中路径的数字见报告矩阵两行。
pub fn engine() -> Result<Engine> {
    let mut cfg = Config::default();
    let cache = wasmtime::Cache::new(wasmtime::CacheConfig::default())?;
    cfg.cache(Some(cache));
    Engine::new(&cfg).context("engine init")
}

/// 默认拒权 linker：未注册任何宿主函数/接口。
pub fn deny_linker(engine: &Engine) -> Linker<()> {
    Linker::new(engine)
}

/// 加载 assets/ 下的 component（相对本 crate 根，随 cwd 无关）。
pub fn component(engine: &Engine, file: &str) -> Result<Component> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(file);
    Component::from_file(engine, &path).with_context(|| format!("load {file}"))
}

/// 完整冷启动一步：engine + component + instantiate + 一次 call。
pub fn cold_start_once(payload: &str) -> Result<String> {
    let engine = engine()?;
    let component = component(&engine, "demo_tool.wasm")?;
    let linker = deny_linker(&engine);
    let mut tool = DemoTool::instantiate(&engine, &component, &linker)?;
    tool.call(payload)
}

/// SPEC §2.3 的 10 KB JSON 载荷（确定性生成）。
pub fn ten_kb_json() -> String {
    let item = serde_json::json!({
        "path": "photos/2026/09/IMG_0001.heic",
        "asset_id": "01JBCDEFGHJKMNPQRSTVWXYZ0001",
        "tags": ["family", "trip", "raw"],
        "size": 4_821_331_u64,
        "mtime": "2026-09-28T12:00:00Z",
    });
    let mut out = String::with_capacity(10 * 1024);
    out.push('[');
    while out.len() < 10 * 1024 {
        if out.len() > 1 {
            out.push(',');
        }
        out.push_str(&item.to_string());
    }
    out.push(']');
    out
}
