# Task: WASM 宿主 crate 骨架 + wasmtime 入根（M7-WP01-T01）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M7-WP01-T01 |
| **类型** | 实施（R1：新 crate + 新顶层依赖接线，SPEC 批准后首任务） |
| **优先级** | P0（SPEC M7-WP01 任务链 T01–T05 之首） |
| **范围** | 根 `Cargo.toml` / `Cargo.lock` / `crates/partisync-ext-host/**` / `docs/tests/properties.md` + 本任务卡 |
| **创建日期** | 2026-09-29 |
| **来源** | SPEC M7-WP01 §3 T01（2026-09-29 批准，PR #26）；ADR-0025 决策节线位 diff |

## 交付物

1. **P13 扩展 + P14 新增登记**（先于实现代码，partisync-property-registry
   口径）：P13 行扩展 per-call 拒绝语义（M7 实施全量 = 每次调用宿主函数
   必败，非仅实例化期）；P14 新增注权 manifest 不变量（校验失败加载即
   拒、注入面 = 声明面 ∩ 宿主白名单）。
2. **wasmtime 入根 `[workspace.dependencies]`**：按 ADR-0025 决策节 diff
   原样落位（`>=47.0.4, <48`、`default-features = false`、features =
   cranelift/component-model/runtime/wat/demangle/**cache**；cache 必选 =
   冷启动预算前提）。deny.toml **零改动**（license 链实测通过）。
3. **`crates/partisync-ext-host` 骨架**（SPEC §2.1：能力层，T01 无内部
   依赖，依赖面 ⊆ {core} 承诺保持）：`Engine` 进程级单例（OnceLock；
   cache 盘不可写时降级无缓存，正确性不受影响）+ `deny_linker()`（零
   import 默认拒权，[P13]）+ `load_component()`。
4. **冒烟测试 ×3**：零能力 demo tool 加载+实例化+调用通路；缺权探针
   实例化即拒 + 错误文本不泄露宿主路径/env（[P13] 冒烟级）；Engine
   单例同一性。fixture 复用 spike component 产物（M6-WP04 §2.3）。

## 明确不做（T01 边界）

- 注权 manifest / host function 注入（T02）；per-call 全量探针（T03）；
  MCP 工具面接线（T04）；基准复测（T05）。

## 验收

- [x] `cargo test -p partisync-ext-host` 3/3 绿
- [x] `cargo clippy -p partisync-ext-host --all-targets -- -D warnings`
      零警告；`cargo fmt --all --check` 绿
- [x] `cargo deny check` 全绿（零新增豁免；RUSTSEC-2026-0269 由线位
      下限 47.0.4 化解，与 ADR-0025 §后果登记一致）
- [x] P13/P14 登记先于实现代码成稿（同一 commit 内 properties.md 登记
      行独立可审；partisync-property-registry SKILL 允许登记行随实现
      同 PR 落地）
- [ ] CI 全绿 + wasmtime 编译代价 CI 时长对照（SPEC §3 强制项，T05
      汇总；本 PR 描述登记首次增量观察）
