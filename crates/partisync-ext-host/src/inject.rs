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
use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};
use wasmtime::component::Linker;
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

/// clock.read 的 WIT interface 全名（`package:interface@version`）。
///
/// **T03 修正**：T02 误按扁平名 `partisync.ext/clock.now_millis` 在根
/// instance 注册——真实 component 的 import 是 interface **实例**
/// （见 `tests/fixtures/clock_probe.wat`：`import
/// "partisync:ext/clock@0.1.0" (instance ...)`）。wasmtime TypeChecker
/// 对 `TypeDef::ComponentInstance` 要求 linker map 里存在
/// `Definition::Instance`（matching.rs `TypeChecker::definition`），
/// 根 instance 上的 `Definition::Func` 永远匹配不上——注权时钟
/// component 在 T02 实现下无法实例化。正确形态 = `linker.instance(全名)`
/// 后在该 instance 内 `func_wrap(函数名)`。
const CLOCK_INTERFACE: &str = "partisync:ext/clock@0.1.0";

/// clock interface 内 host function 名（WIT 标识符 kebab-case）。
const CLOCK_FN_NOW_MILLIS: &str = "now-millis";

/// 按 manifest 声明面构造 linker。**注入面 = 声明面**（白名单
/// 与 Capability 枚举第一版一致，故「∩ 白名单」= 声明面；演化到
/// 白名单表与 Capability 分离时此处加二次过滤）。
///
/// 调用方（`load_with_manifest`）必须保证 manifest 已 `validate()`
/// 通过。本函数不做二次校验——重复校验等于「在校验后再校验」，语义
/// 冗余且易漂移；调用面契约写明。
pub fn linker_for(engine: &wasmtime::Engine, manifest: &Manifest) -> Result<Linker<()>> {
    let mut linker = Linker::new(engine);
    // T03 修正：manifest 容忍同一 capability 重复声明（`validate` 去重
    // 视为同一项，见 manifest.rs），但 `Linker` 以 `allow_shadowing: false`
    // 构造，重复 `linker.instance(全名)` 会因 `NameMap` 重名硬失败。此处
    // 按去重后的集合注入，与 manifest 侧的宽容语义对齐。
    let mut injected: BTreeSet<Capability> = BTreeSet::new();
    for cap in &manifest.capabilities {
        if !injected.insert(*cap) {
            continue;
        }
        inject(&mut linker, *cap)?;
    }
    Ok(linker)
}

/// 单一 capability 注入。每个 capability 对应一个 WIT interface，
/// 注入 = 在该 interface 实例下注册其 host function。
fn inject(linker: &mut Linker<()>, cap: Capability) -> Result<()> {
    match cap {
        Capability::ClockRead => inject_clock(linker),
        // `index.read` 由 T04 + `IndexRead` trait 注入时接线（PARTISYNC
        // 索引只读接口的 wasm host function）；T03 暂不引 trait 依赖
        // ——保留注入点签名，后续 PR 仅加 match 分支即可。
        Capability::IndexRead => inject_index_stub(linker),
    }
}

/// `clock.read` 实装：在 `partisync:ext/clock@0.1.0` interface 实例下
/// 注册 `now-millis: func() -> u64`。
fn inject_clock(linker: &mut Linker<()>) -> Result<()> {
    let mut clock = linker.instance(CLOCK_INTERFACE)?;
    clock.func_wrap::<_, (), (u64,)>(CLOCK_FN_NOW_MILLIS, |_store, ()| Ok((now_millis(),)))
}

/// `index.read` 第一版占位（SPEC §2.2 注入点保留 + §2.1 预案）：
/// 不暴露任何 host function。T04 接线由 gateway 注入 `IndexRead`
/// trait 实现；本 PR 显式登记「本 capability 当前未注入 host
/// function」——声明 `index.read` 的 component 因缺 interface 实例而
/// 实例化必拒（[P13] 默认拒权），T04 接入后可达。
fn inject_index_stub(_linker: &mut Linker<()>) -> Result<()> {
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
        // 零注权：任何 component 的 import 都无法解析（真实断言见
        // tests/probes_p13_p14.rs 的零注权探针——本 unit 测试只验
        // 构造路径不报错，真实拒绝语义由 fixture 探针覆盖）。
        let _ = linker.root();
    }

    /// 重复 capability 声明去重（T03 修正 F-3）：manifest 侧容忍重复
    /// （`validate` 视为同一项），linker 侧以 `allow_shadowing: false`
    /// 构造会因 interface 重名硬失败——两者语义曾矛盾。
    #[test]
    fn linker_for_deduplicates_repeated_capability() {
        let engine = crate::engine();
        let m = Manifest {
            tool_name: "dup_clock".into(),
            capabilities: vec![Capability::ClockRead, Capability::ClockRead],
        };
        assert_eq!(m.validate(), Ok(()), "manifest 侧容忍重复声明");
        let linker = linker_for(engine, &m).expect("重复 capability 必须去重后注入，不得硬失败");
        let _ = linker;
    }

    #[test]
    fn linker_for_clock_read_injects_clock_interface() {
        let engine = crate::engine();
        let m = Manifest {
            tool_name: "clock_user".into(),
            capabilities: vec![Capability::ClockRead],
        };
        let mut linker = linker_for(engine, &m).expect("clock.read manifest 合法");
        // 构造路径通畅；**真实可达性**（component 能否解析并调用该
        // interface 下的 `now-millis`）由 tests/probes_p13_p14.rs 的
        // `p14_clock_granted_clock_probe_calls_host_function` 用真实
        // component fixture 覆盖——T02 缺陷正是「构造通过但永不可达」。
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
        // 声明 index.read 但不注入 interface 实例 → 需该接口的
        // component 必拒（真实断言见 tests/probes_p13_p14.rs）。
        let _ = linker.root();
    }
}
