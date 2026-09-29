# Task: IndexRead trait + index.read 注权实装（T04-A）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M7-WP01-T04（T04 第 1 步：注权面收口） |
| **类型** | 实施（R1：[P14] 白名单第二项实装 + 宿主 Store 状态类型变更） |
| **优先级** | P0（SPEC M7-WP01 §3 T04；T02b 遗留的 `inject_index_stub` 占位即待此接线） |
| **范围** | `crates/partisync-ext-host/src/host_state.rs`（新建）/ `src/inject.rs` / `src/lib.rs` / `tests/fixtures/ext.wit`（新建，取代 `clock.wit`）/ `tests/fixtures/index_probe.wat`（新建）/ `tests/probe_index.rs`（新建）/ 三个既有测试的 Store 类型迁移 + 本任务卡 |
| **创建日期** | 2026-09-29 |
| **来源** | SPEC M7-WP01 §2.1（`IndexRead` trait 注入契约）+ §2.2 白名单表 `index.read` 行；T02b 占位 / T03 F-6 收口 |

## 交付物

1. **`host_state.rs`（新建）**：`IndexRead` trait（宿主定义，注入方向
   组装期 → 宿主）+ `HostState`（Store 载荷，`index: Option<Arc<dyn
   IndexRead>>`）。SPEC §2.1 契约落地：**扩展宿主 crate 不直连
   index / graph crate**。
2. **`inject.rs`：`index.read` 实装**（替换 T02b 的
   `inject_index_stub` 占位）——在 `partisync:ext/index@0.1.0` interface
   实例下注册 `search: func(query: string) -> string`，转发到
   `HostState::index`。**Store 类型 `()` → `HostState`**（破坏性变更，
   仅限新 crate 内）。
3. **`tests/fixtures/ext.wit`**：取代 T03 的 `clock.wit`，扩为宿主
   完整 WIT 面（`clock` + `index` 两个 interface，两个 world）。
4. **`tests/fixtures/index_probe.wat`**：真实 component（import
   `partisync:ext/index@0.1.0`，export `search`）。生成路径可复现，
   记录在文件头——含两个 canonical ABI 必需项：core module 的
   import 名须带 `@version`、**必须 export `cabi_realloc`**（缺则
   `component new` 报 `module does not export a function named
   cabi_realloc`）。
5. **`tests/probe_index.rs`（3 探针）**：
   - 注权 + 组装期注入实现 → 扩展真实调用 trait 实现，查询串原样
     抵达、扩展回传串原样透传
   - 未注权 → import 不可解析、实例化必拒（[P13] 拒绝半边）
   - 注权但组装期未注入 → 实例化成功、调用返回 `{"error": ...}`
     而非 trap（SPEC §2.1「注权面（声明）与宿主接线（实现）分离」）

## 签名口径决策

`index.read` 用 `func(query: string) -> string`（JSON 进 JSON 出），
**不用** canonical `result<T,E>`：后者要求 `Return = (Result<..>,)`
且每元素满足 `ComponentType`，与 MCP 工具面的错误口径
（`is_error: true` + 文本）不对齐；JSON 形态更贴合「wasm component
即 MCP tool」的既有语义。理由记入 `ext.wit` 头注。

## SPEC §2.1 预案状态

§2.1 预案「若 trait 注入不足以表达某 capability，回退
`ext-host → graph` 直依赖」——**未启用**。T04 落地证 trait 表达力
足够（索引只读查询是单一无状态方法），且保持 crate 地图「只允许向下
依赖」。

## 验收

- [x] `cargo test -p partisync-ext-host` **33/33 绿**（unit 12 +
      integration 4 + P14 探针 6 + **index 探针 3** + T03 探针 5 +
      smoke 3）
- [x] `clippy --all-targets -D warnings` 零警告；`fmt --check` 绿
- [x] `deny check` 零新增豁免（治理前置见 PR #32）
- [x] Store 类型迁移无回归：T01/T02/T03 全部既有测试仍绿
- [ ] CI 全绿 + 对抗审查

## 钉子清单 disposition

**已登记：公共 API（`IndexRead` trait + WIT wire 格式）** —— 对抗审查 F-3
判定本项应为「登记」而非 n/a：`IndexRead` 是 T04-B gateway 要实现、
T05 之后每个扩展作者要面对的公共契约，其 wire 口径（JSON 进 JSON 出 +
`{"error": ...}` 失败形态）已由 `ext.wit` + `probe_index.rs` 冻结。
- 双人评审：AI 对抗审查（GLM-5.3-Flash R2）+ 人工终审 @lead（PR #33）
- 冻结内容：`search(query: string) -> string`；错误以
  `{"error": "index.read unavailable"}` 中性文案表达（对抗审查 F-8：
  不向不可信 guest 暴露内部接线状态 oracle）
- 其余钉子清单表面 n/a：无 schema migration / unsafe / deny.toml /
  crypto/auth 改动；`IndexRead` 与 `HostState` 目前**无既有跨 crate
  调用方**（ext-host 尚未被 gateway 依赖，审查 grep 证实 0 命中）

## T04-B 前置项（对抗审查登记，本 PR 不处置）

| # | 项 | 处置方向 |
|---|---|---|
| F-2 | `IndexRead::search` 签名无失败通道，真实实现遇索引不可用只能 panic——panic 会被 wasmtime 在 wasm 边界转成 trap 并**毒化 Store**，该扩展实例此后所有调用失败于 `cannot access a poisoned store`（既非 JSON 也非可用实例），与 trait 文档承诺的「不 panic、不 trap」矛盾 | T04-B 改 `Result<String, IndexError>`，由 `inject_index` 映射成同一 wire JSON（线格式不变）；会再次变更 Store 相关 API，混入本 PR 会超 400 行 |
| F-4 | 「注权面与宿主接线分离」是**静默降级**：gateway 忘接线时扩展实例化仍成功，每次调用拿错误 JSON，全链路无日志无自检 | T04-B 加 `preflight(&Manifest, &HostState) -> Result<()>`（声明了但未接线 → 加载期拒，与 [P14] 语义一致）或宿主侧 `log::warn` |
| F-12 | `search` 无 fuel / epoch / 长度上限，guest 可用大串查询放大 | T04-B 上线前立项（SPEC §2.4 有 RSS 预算，wire 层无对应约束） |
