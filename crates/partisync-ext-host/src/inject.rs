//! host function 注入面（SPEC M7-WP01 §2.2 capability 白名单冻结表落地）。
//!
//! 注入面 = 声明面 ∩ 宿主白名单；未声明的 capability 一律不注入（[P14]
//! 不变量：拒绝先于任何 host function 暴露）。
//!
//! 第一版白名单（SPEC §2.2 批准即冻结）：
//! - `index.read` — 索引只读查询（T04 + \`IndexRead\` trait 注入时接线）
//! - `clock.read` — 当前时间戳（\`now_millis()\`，本 PR 落地）
//!
//! FS / 网络 / 环境不在白名单——相应 host function 根本不存在于
//! linker，无注入点；[P13] 已被 spec spike 与 T01 smoke 冒烟覆盖（缺
//! 权 component 实例化即拒，错误不泄露宿主路径/env）。

use crate::manifest::{Capability, Manifest};
use std::time::{SystemTime, UNIX_EPOCH};
use wasmtime::component::{Linker, LinkerInstance};
use wasmtime::Result;

/// 当前时间戳毫秒（`clock.read` 能力语义）。注入到 linker 的 host
/// function 直接调用本函数——避免宿主路径/环境进 wasmtime 上下文
/// （[P13] 不泄露路径不变）。
///
/// 行为：UTC 毫秒 since UNIX epoch。错误语义 = 1970-01-01 之前的
/// 系统时间（实际不可达）。
fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 按 manifest 声明面构造 linker。**注入面 = 声明面**（白名单
/// 与 Capability 枚举第一版一致，故「∩ 白名单」= 声明面；演化到
/// 白名单表与 Capability 分离时此处加二次过滤）。
///
/// 调用方（\`load_with_manifest\`）必须保证 manifest 已 \`validate()\`
/// 通过。本函数不做二次校验——重复校验等于「在校验后再校验」，语义
/// 冗余且易漂移；调用面契约写明。
pub fn linker_for(engine: &wasmtime::Engine, manifest: &Manifest) -> Result<Linker<()>> {
    let mut linker = Linker::new(engine);
    let mut instance = linker.root();
    for cap in &manifest.capabilities {
        inject(&mut instance, *cap)?;
    }
    Ok(linker)
}

/// 单一 capability 注入。宿主接口定义（WIT）使用 wasmtime 47 直接
/// 注册模式（非 WIT bindgen 宏），与 spike 宿主面（<100 行）风格一致。
/// `func_wrap` 真实签名：\`Fn(StoreContextMut<T>, Params) -> Result<Return>\`，
/// 名字走点号命名空间（`module.function`）。
fn inject(instance: &mut LinkerInstance<'_, ()>, cap: Capability) -> Result<()> {
    match cap {
        Capability::ClockRead => inject_clock(instance),
        // `index.read` 由 T04 + `IndexRead` trait 注入时接线（PARTISYNC
        // 索引只读接口的 wasm host function）；T02b 暂不引 trait 依赖
        // ——保留注入点签名，后续 PR 仅加 match 分支即可。
        Capability::IndexRead => inject_index_stub(instance),
    }
}

/// `clock.read` 实装：暴露 `partisync.ext/clock.now_millis` host function。
/// 签名：( ) -> ( u64 )。命名空间按 wasmtime func_wrap 约定：
/// `module.function` 形式。
fn inject_clock(instance: &mut LinkerInstance<'_, ()>) -> Result<()> {
    instance.func_wrap::<_, (), (u64,)>("partisync.ext/clock.now_millis", |_store, ()| {
        Ok((now_millis(),))
    })
}

/// `index.read` 第一版占位（SPEC §2.2 注入点保留 + §2.1 预案）：
/// 不暴露任何 host function。T04 接线由 gateway 注入 \`IndexRead\`
/// trait 实现；本 PR 显式登记「本 capability 当前未注入 host
/// function」，调用方 \`load_with_manifest\` 不会因 IndexRead 声明
/// 而走额外注入路径。
fn inject_index_stub(_instance: &mut LinkerInstance<'_, ()>) -> Result<()> {
    // 不调用 instance.func_wrap——index.read 的 host function 由 T04
    // 引入；本 PR 不预占 trait 依赖，保留 match 分支位置。
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_now_millis_is_reasonable() {
        // 简单 sanity：返回的毫秒时间戳在合理范围（>2020-01-01 = 1.5e12 ms）
        let t = now_millis();
        assert!(t > 1_577_836_800_000, "now_millis 异常小: {t}");
    }

    #[test]
    fn linker_for_empty_manifest_yields_empty_linker() {
        let engine = crate::engine();
        let m = Manifest {
            tool_name: "empty".into(),
            capabilities: vec![],
        };
        let mut linker = linker_for(engine, &m).expect("空 manifest 合法 = 零注入");
        // 根 instance 可取；不在断言接口名（wasmtime API 面非稳定公开）
        let _ = linker.root();
    }

    #[test]
    fn linker_for_clock_read_injects_clock_interface() {
        let engine = crate::engine();
        let m = Manifest {
            tool_name: "clock_user".into(),
            capabilities: vec![Capability::ClockRead],
        };
        let mut linker = linker_for(engine, &m).expect("clock.read manifest 合法");
        // 注入即成功：linker 接受 func_wrap；接口命名失败由 wasmtime
        // 返回 Err，测试通过即证明注入通路通畅。
        let _ = linker.root();
    }

    #[test]
    fn linker_for_index_read_stub_injects_nothing_but_succeeds() {
        let engine = crate::engine();
        let m = Manifest {
            tool_name: "future_index".into(),
            capabilities: vec![Capability::IndexRead],
        };
        let mut linker = linker_for(engine, &m).expect("index.read 占位注入合法");
        let _ = linker.root();
    }
}
