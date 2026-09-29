# Task: host function 注入 + load_with_manifest 整合路径（M7-WP01-T02b）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M7-WP01-T02（PR-B 子任务，任务卡独立） |
| **类型** | 实施（R1：host function 注入 + linker 整合路径，依赖 PR-A） |
| **优先级** | P0（SPEC M7-WP01 §3 T02 第二子任务） |
| **范围** | `crates/partisync-ext-host/src/inject.rs` + `src/lib.rs` + `tests/integration_p13_p14.rs` + 本任务卡 |
| **创建日期** | 2026-09-29 |
| **来源** | SPEC M7-WP01 §2.2 capability 白名单冻结表 + §3 T02 验收 + PR-A（a106d28） |

## 交付物

1. **`inject` 模块**（`crates/partisync-ext-host/src/inject.rs`）：
   - `linker_for(engine, manifest)` —— 按 manifest 声明面构造 linker
   - `inject_clock(instance)` —— 实装 `partisync.ext/clock.now_millis`：
     签名 `( ) -> (u64,)`，命名空间 `partisync.ext/clock`，
     `func_wrap::<_, (), (u64,)>` turbofish 显式指定泛型
   - `inject_index_stub(_)` —— 显式占位（SPEC §2.2 注入点保留）：
     不调 func_wrap，T04 引入 `IndexRead` trait 时仅加 match 分支
2. **`load_with_manifest(wasm_path, manifest_path)`**（lib.rs）：
   三阶段整合路径 `Manifest::load` → `load_component` → `linker_for`。
   任何阶段失败 = 加载拒（[P14] 核心语义）
3. **集成探针 ×4**（`tests/integration_p13_p14.rs`）：
   - 缺 manifest → `ManifestError::Io`（端到端）
   - 合法 manifest + 零能力 demo tool → 加载通过 + 实例化 +
     call(input) → output JSON（[P14] 注入面 = 声明面 + spike 同通路）
   - 撞名 manifest → 加载期拒绝（[P14] 拒绝先于 host function 暴露）
   - probe_deny.wasm 在带 clock.read linker 下仍拒（[P13] 默认拒权不变）

## API 查证（铁律红线）

wasmtime 47 func_wrap 真实签名
（`/registry/wasmtime-47.0.4/src/runtime/component/linker.rs:480`）：
- 三参数 `&mut self, name: &str, func: F`（**不是** 4 参 `(module, name, func)`）
- `name` 走点号命名空间 `module.func`
- `F: Fn(StoreContextMut<T>, Params) -> Result<Return>`
- `Params / Return` 必须 `ComponentNamedList + Lower`（unit type 写作 `()`，
  单值返回必须 tuple `(u64,)`）
- `Linker` 顶层**无** func_wrap，须经 `linker.root()` 取 `LinkerInstance`

## 验收

- [x] `cargo test -p partisync-ext-host` 24/24 绿（inject unit 4 + integration
      4 + manifest unit 7 + [P14] 探针 6 + smoke 3 + doc 0）
- [x] `clippy --all-targets -D warnings` 零警告；`fmt --check` 绿
- [x] `deny check` 零新增豁免
- [ ] CI 全绿 + 时长对照（与 PR-A 串行跑测；wasmtime 冷编译代价已在
      T01#27 闭环，本 PR 走增量预期大幅下降）

## 钉子清单 disposition（PR-B 触及表面）

n/a — PR-B 不触及任何钉子清单表面：inject + 整合路径为新 crate 内
新模块；无 unsafe、无 deny.toml 改动、无 migrations、无跨 crate
pub API、无 crypto/auth。`IndexRead` trait 注入按 SPEC §2.1 预案推迟
到 T04，本 PR 仅占位注入点。

## 与 PR-A 的协作

PR-A（a106d28）落 manifest 校验 + [P14] 单测；PR-B 在 PR-A 基础上
接力 host function 注入 + 整合路径 + [P13]/[P14] 集成探针。两 PR
按 ≤400 行拆分守铁律 3（原子交付）。