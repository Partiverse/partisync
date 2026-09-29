# ADR-0025: WASM 扩展运行时选型 —— wasmtime + WIT Component Model（wasm component 即 MCP tool）

版本: 1.0 · 状态: **已接受（2026-09-28 定稿；2026-09-29 用户签收，签字表见文末）**
关联: SPEC M6-WP04（评估 WP，批准 2026-09-28）、ADR-0024（Tauri 桌面壳，
IPC capability 边界脚手架）、ADR-0003（toolchain 1.94 pin）、执行方案 §6.7
负责人: @lead · 起草日期: 2026-09-28（commit `4260208`，PR #20）·
定稿日期: 2026-09-28（M6-WP04-T03）

## 背景

M6 §6.7 要求对「WASM 扩展系统（学 Spacedrive 沙箱）」做评估。
PartiSync 已有两块可复用地基：(a) MCP 工具面（M4-WP03 L1/L2 闭环 +
`partisync-mcp` stdio 侧车 + 桌面壳 `mcp_call` IPC，M6-WP03 T05）；
(b) Tauri IPC capability 边界（ADR-0024 留口）。扩展形态据此锁定为
「沙箱化 MCP tool」——wasm component 实现既有 MCP 工具语义（JSON
入参 → JSON 出参），经宿主加载后可被 `partisync-mcp` 工具面与
`mcp_call` IPC 无差别调用。

选型约束：铁律 8「无聊依赖」（新顶层依赖须 ADR + cargo deny）、Rust
1.94 toolchain pin（ADR-0003）、deny.toml 白名单纪律。技术面基线
（2026-09 核查）：WASI 0.3 已于 2026-06 落地，Component Model 进入
生产可信区；wasmtime 为 Rust 宿主事实标准。

评估方式：SPEC M6-WP04 预注册三候选（C1 wasmtime+WIT / C2 Extism /
C3 Wassette 模式）与六项可测量判定基准（SPEC §2.3），spike 实测矩阵
（T01/T02，报告 [docs/reports/M6-WP04-wasm-ext-eval.md](../reports/M6-WP04-wasm-ext-eval.md)）
不推翻草稿倾向即定稿。**spike 六项基准全 PASS（2026-09-28 实测），
初稿预注册的决策规则触发，本 ADR 定稿。**

## 决策

**采纳 C1：wasmtime + WIT Component Model 直接嵌入，版本锚定
`>=47.0.4, <48`（47.x 线），宿主形态「wasm component 即 MCP tool」，
复用 partisync-mcp 工具面与 mcp_call IPC，ADR-0024 IPC 边界即
capability 脚手架。**

- 本 ADR 记录**选型决策**；`wasmtime` 正式入根
  `[workspace.dependencies]` 留 M7+ 实施 WP（独立 SPEC，骨架见
  `docs/specs/M7-WASM-impl-draft.md`），届时按本节线位落 diff，
  不重复选型。
- 版本线位（M7+ 实施 PR 须遵守）：

  ```diff
  # Cargo.toml（M7+ 实施 PR 落地，本 ADR 仅登记线位）
  [workspace.dependencies]
  +wasmtime = { version = ">=47.0.4, <48", default-features = false, features = [
  +    "cranelift", "component-model", "runtime", "wat", "demangle", "cache",
  +] } # ADR-0025（M7+ WASM 扩展宿主；线位与 spike 实测一致）
  ```

  线位理由：47.x 线 MSRV = 1.94.0 与 `rust-toolchain.toml` pin 匹配
  （49.x 需 rustc 1.96、48.x 需 1.95）；47.0.0–47.0.3 有
  RUSTSEC-2026-0269 → 下限 47.0.4；上限 `<48` 防 caret 漂移进不兼容
  minor。`cache` feature 为**必选**（冷启动预算前提，见后果节）。
  参考落点：`crates/partisync-wasm-spike/Cargo.toml`（spike 独立
  workspace 同线位，2026-09-28 实测）。

## 备选方案

| 备选 | 评估 | 裁定理由 |
|------|------|---------|
| **C1 wasmtime + WIT 直接嵌入（采纳）** | Bytecode Alliance 事实标准、单一顶层依赖、Component Model 原生、WASI 0.3 支持最完整；「无聊依赖」契合度最高 | 六项基准全 PASS（冷启动 p50 1.6 ms / RTT p50 0.19 ms / RSS +4.4 MB / 默认拒权可达 / clippy 1.94 零警告 / deny+audit 全绿）；宿主面 <100 行，boilerplate 自担成本实测可控 |
| C2 Extism（wasmtime 之上封装层） | 跨语言 PDK、宿主契约与 manifest 开箱即用，boilerplate 最少 | **否决**：为省 boilerplate 引入第二层依赖耦合（extism ↔ wasmtime 版本联动升级），违背铁律 8；其卖点（简化宿主 API）被 spike 证伪——C1 宿主面本身 <100 行；同底座下性能不可能优于 C1；MSRV/deny 面多一环而无对应收益 |
| C3 Wassette 模式（参考设计） | 「wasm component 即 MCP tool」语义与既有 MCP 工具面完全对齐；manifest 声明 capability → 宿主注权白名单 | **不作为运行时依赖候选**：其运行时本体同为 wasmtime，引入无增量价值；其**注权 manifest 设计**收录为 M7+ 实施参照（SPEC M6-WP04 R6 开放问题——宿主注权机制——的处置方向，登记进 M7 骨架草案） |

## 后果

正面：

- 扩展以沙箱组件接入资产图谱，不进 workspace 依赖树、不触碰 R2
  钉子清单；新工具不再走「新 MCP tool 进 partisync-gateway + 全门禁」
  重路径
- ADR-0024 的 7 commands + mcp_call capability 脚手架获得消费者，
  价值兑现
- P13「扩展沙箱」不变量经 spike 双探针证实可达：未授予能力的
  component 实例化即拒、错误文本不泄露宿主路径/env
  （`docs/tests/properties.md` 已登记，commit `2d94b5b`）
- 性能优于既有 MCP 侧车基线：进程内 wasm 调用 RTT 约为 stdio
  JSON-RPC 侧车的 1/2，且无子进程生命周期管理面

负面 / 放弃 / 附条件：

- **wasmtime 为重量级 crate**：编译时间与二进制体积代价在 M7+ 实施
  PR 兑现（spike 期独立 workspace 已隔离，根 Cargo.toml/deny/CI 零
  改动）；实施 PR 须附 CI 时长对照
- **冷启动预算附条件**：宿主必须启用 wasmtime `cache` feature——缓存
  命中 p50 1.6 ms / p95 2.3 ms（< 100 ms 预算内）；未命中路径（首次
  编译）min 73–200 ms、负载低谷 p50 84.5 ms，**p95 可超预算是已知
  行为**，运维以「首载预热」消化，不作指标放宽
- **MSRV 锁死 47.x**：升级 wasmtime 48/49 必须先升 toolchain
  （ADR-0003 路径），升级须重测六项基准（见重新评估条件）
- **纯工具 guest 编译约束**：`wasm32-unknown-unknown` +
  `wasm-tools component new`（wasip2 目标的 std 残留 `wasi:io/poll`
  import，过不了默认拒权 linker）；需要宿主能力的扩展才用 wasip2
  目标（provably-granted capability 面）
- **放弃 C2 的开箱 PDK/manifest**：注权机制宿主自担，M7+ 以
  Wassette 式 manifest 设计补齐

deny / audit 实测（spike 独立 workspace，193 deps；T02 首测 +
T03 定稿日重跑复核，两次全绿）：

```text
$ cd crates/partisync-wasm-spike && cargo deny check
warning[duplicate]: found 2 duplicate entries for crate 'winnow'
    ┌─ crates/partisync-wasm-spike/Cargo.lock:173:1
    │ winnow 0.7.15 / winnow 1.0.4（toml 0.9 传递链，信息级告警）

advisories ok, bans ok, licenses ok, sources ok
```

- `cargo audit`：零公告（193 deps，2026-09-28）。历史命中
  RUSTSEC-2026-0269（wasmtime ≤ 47.0.3）已由下限 47.0.4 化解，
  非豁免路径
- 根 `deny.toml` **零改动**：wasmtime 传递链 license（Apache-2.0 /
  MIT 系）全部落在既有白名单；spike 独立 deny 全绿且零豁免起步
- winnow 双版本为 `multiple-versions = "warn"` 信息级告警（toml 0.9
  传递链），不构成豁免项；M7+ 实施 PR 复核

## 重新评估条件

- **toolchain 升级**：rust-toolchain 升 1.95 → wasmtime 48.x 可用；
  升 1.96 → 49.x 可用。升级须重测 SPEC M6-WP04 §2.3 六项基准
- **版本抖动**：WASI 0.3 / Component Model 工具链（wit-bindgen
  0.62 / wasmparser 0.259 为 spike 锁定组合）出现破坏性版本或安全
  advisory
- **wasmtime 47.x 线安全公告**：出现命中 ≥ 47.0.4 的 RUSTSEC 且
  patch 线内无修复 → 评估提前升 toolchain 或临时处置（新 ADR）
- **范式漂移**：Spacedrive 扩展系统落地并给出可复制的生产范式
- **指标超标回溯**：M7+ 实施期六项基准任一复测超标且 C2 可达标

### 重新评估条件触发记录：RUSTSEC-2026-0315 / 0316（2026-09-29）

**触发**：上列第 3 条（wasmtime 47.x 线安全公告，patch 线内无修复）。
wasmtime 47.0.4 命中两条新公告：0315 修复版本为 `>=48.0.3, <49.0.0` 或
`>=49.0.1`；0316 另含 `>=36.0.16, <37.0.0`。两者在 **47.x patch 线内均
无修复**，且 48.x 需先升 rust-toolchain（`rust-toolchain.toml` 钉 1.94，
治理表面需独立决策）。

| ID | 摘要 | 本仓库可达性 |
|----|------|-------------|
| RUSTSEC-2026-0315 | `call_ref` 与 exception `catch` 可丢 fuel 记账，导致指数级 fuel amplification | **不可达**。①宿主 `Config::default()` + 磁盘 cache，**未启用** `consume_fuel`——fuel 是该公告危害向量的唯一载体（危害是「相对 fuel 预算的指数级放大」，本仓库无 fuel 预算则该向量不成立）；②exception handling 未启用——`WasmFeatures::EXCEPTIONS` 由 `cfg!(feature = "gc")` 决定（wasmtime `config.rs`：`features.set(WasmFeatures::EXCEPTIONS, cfg!(feature = "gc"))`），本仓库 wasmtime feature 集为 `[cranelift, component-model, runtime, wat, demangle, cache]`，**不含 `gc`**；`Config::new` 亦对「无 `gc` 却开启 EXCEPTIONS」直接 `bail!`，故 `Engine::new` 成功本身即反证其关闭。 |
| RUSTSEC-2026-0316 | 动态 record lifting 可超出 hostcall fuel limit 分配 | **不可达**。①同样未启用 fuel（无 fuel limit 即无「超出 fuel limit 分配」的度量基准）；②已 link 的宿主 interface（`partisync:ext/clock@0.1.0` 的 `now-millis`、`partisync:ext/index@0.1.0` 的 `search`）**签名中不含任何 resource/record 类型**；未满足的 import 在实例化期即失败（[P13] 探针已覆盖），guest 无法凭空取得 record-returning hostcall。 |

**处置**：`deny.toml` `[advisories] ignore` 增两条（治理表面，随本
修订登记），取 ADR-0024 amend 判例的登记格式（可达性论证 + 撤销
条件）。

**为何不按本 ADR 第 3 条字面「新 ADR」处置**：`docs/specs/M7-WP01.md`
§3 验收项 4 明确授权「deny.toml 若需改动，走 ADR-0025 修订登记，不开新
ADR」——SPEC（后于 ADR 定稿，2026-09-29 批准）对同一触发条件给出了
更具体的处置路径，本修订登记即按该授权执行。决策节线位**不变**（升
48.x 须先升 rust-toolchain，属独立治理决策，不在本次范围）。

**撤销条件**（任一即撤销豁免并重开处置）：
1. `rust-toolchain.toml` 升 1.95+ → wasmtime 切 `>=48.0.3` 线位
2. **根 `[workspace.dependencies] wasmtime.features` 增开 `gc`** ——
   `EXCEPTIONS` 随之自动开启，0315 前提②失效
3. 宿主 `Config` 启用 `consume_fuel`（或任何 fuel 计量）——两条公告的
   共同前提失效
4. 宿主 `Config` 启用 async / `component-model-async` feature
5. 宿主 WIT 面引入 resource / record 类型，或 link 新的宿主 interface
   含 record 签名
6. wasmtime 47.x 线发布含修复的 patch 版本
  → 重开本 ADR

## 签字登记（铁律 7：新增顶层依赖属地基级决策，架构双签）

| 角色 | 状态 | 日期 |
|------|------|------|
| 决策规则预授权（初稿「spike 实测不推翻即定稿」随 SPEC M6-WP04 批准） | ✅ 用户拍板 | 2026-09-28（PR #20） |
| spike 实测触发（六项基准全 PASS，T01/T02） | ✅ 完成 | 2026-09-28（PR #21） |
| AI 定稿起草（本文件 v1.0） | ✅ 完成 | 2026-09-28（M6-WP04-T03） |
| 架构负责人双签 | ✅ 已签（用户拍板「同意签收 ADR-0025」；单人维护模式 @lead 兼任架构决策，沿 ADR-0024「用户拍板即已接受」判例） | 2026-09-29 |
| 人工终审（用户） | ✅ 签收（会话指令「同意签收 ADR-0025」） | 2026-09-29 |

## 修订登记

| 修订 | 触发任务 | 修订内容 | 关联 commit / PR |
|------|---------|---------|-----------------|
| 初稿 0.1 | M6-WP04-T01 | 选型框架 + 候选否决理由模板 + 决策规则预注册 | commit `4260208` / PR #20 |
| 定稿 1.0 | M6-WP04-T03 | 状态草稿→已接受；决策节 C1 落定 + 版本线位；后果节填实测数字 + deny/audit 复跑输出；签字登记 + 修订登记表建立 | commit `8196fb0` / PR #22 |
| 签收回填 | M6-WP04-T03 | 签字表两待签位签收（用户拍板 2026-09-29）；状态行同步；内容零变更 | 本 commit |
| 修订 6 | M7-WP01-T04 | **触发「重新评估条件」第 3 条**（wasmtime 47.x 线安全公告，patch 线内无修复）：RUSTSEC-2026-0315 / 0316 两条新公告的可达性论证（宿主未启用 fuel / exception handling，WIT 面无 resource）+ `deny.toml` 豁免 + 六条撤销条件。决策节线位**不变**（仍 `>=47.0.4, <48`）——升 48.x 须先升 rust-toolchain，属独立治理决策。处置路径按 SPEC M7-WP01 §3 验收项 4 授权（不开新 ADR） | 本 commit / PR #32（hash 随 amend 变动，以 PR 号为稳定锚点） |
