# Task: [P13] 全量拒绝探针 + 注权端到端（T03，修正 T02 注权命名空间缺陷）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M7-WP01-T03 |
| **类型** | 实施（R1：[P13]/[P14] property 探针全量化 + **T02 遗留缺陷修正**） |
| **优先级** | P0（SPEC M7-WP01 §3 验收项 1 主交付） |
| **范围** | `crates/partisync-ext-host/src/inject.rs` + `tests/fixtures/clock_probe.wat`（新建）+ `tests/probes_p13_p14.rs`（新建）+ 本任务卡 |
| **创建日期** | 2026-09-29 |
| **来源** | SPEC M7-WP01 §3 验收项 1 + §3 任务 T03；对抗审查 F-4（未验真实调用）/ F-5（命名空间选型未登记）/ F-6（IndexRead 注入面待补探针） |

## 交付物

1. **`tests/fixtures/clock_probe.wat`**（77 行，源码形态可审）：真实
   component ——import `partisync:ext/clock@0.1.0` interface 的
   `now-millis: func() -> u64`，export `now() -> u64`。生成路径可复现
   （wit-component 0.259.0 / wasm-tools 1.259.0，ADR-0025 锁定同线）：
   `wat/clock.wit` → canonical-ABI core module → `component embed` →
   `component new` → `wasm-tools print`
2. **`tests/probes_p13_p14.rs`（5 探针）**：
   - `p14_clock_granted_clock_probe_calls_host_function`：**注权后真实
     调用** host function，返回值 > 2020-01-01 ms（[P14] 可达半边；
     闭环对抗审查 F-4）
   - `p13_index_read_declared_but_not_wired_rejects_clock_import`：只注权
     `index.read`（占位未接线）时 `clock` import 不可解析（[P13] 授予 X
     不误授 Y；闭环 F-6）
   - `p13_zero_grant_deny_probe_rejected_without_host_leak`：零注权下
     component 必拒 + 错误不泄露宿主路径/env（[P13] 零注权面）
   - `p13_wasi_gated_component_denied_under_zero_grant`：需宿主能力的
     wasip2 component（源码触碰 `std::fs` / `std::net::TcpStream` /
     `std::time::SystemTime` 三类）在零注权下必拒 + 不泄露（SPEC §3
     验收项 1 三类 API 面）
   - `p14_collision_rejected_before_component_compile`：撞名 manifest
     在 component 编译前被拒（[P14] 加载侧「拒绝先于 host function 暴露」）
3. **修正 T02 遗留缺陷（实质正确性 bug）** + **重复 capability 去重**
   （对抗审查 F-3，`linker_for` 按去重集合注入，与 manifest 侧宽容语义
   对齐）

## 修正的 T02 缺陷（本 PR 的主要发现）

T02 在**根 instance** 上以扁平名 `partisync.ext/clock.now_millis` 注册
host function。真实 component 的 import 是 **interface 实例**（fixture
实证：`import "partisync:ext/clock@0.1.0" (instance ...)`），wasmtime
`TypeChecker` 对 `TypeDef::ComponentInstance` 要求 linker map 中存在
`Definition::Instance`（`matching.rs::definition`）——根 instance 上的
`Definition::Func` 永远匹配不上。**后果：注权时钟 component 在 T02 实现
下无法实例化，该能力面实际不可达。**

T02 无真实 fixture（复用 spike 的零 import `demo_tool.wasm`）故未暴露；
对抗审查 F-4 指出「未验真实调用」但未定位根因。本 PR 以真实 fixture
实证并修正：

```rust
// 修正前（永不可达）
instance.func_wrap::<_, (), (u64,)>("partisync.ext/clock.now_millis", ..)
// 修正后
let mut clock = linker.instance("partisync:ext/clock@0.1.0")?;
clock.func_wrap::<_, (), (u64,)>("now-millis", ..)
```

## 验收

- [x] `cargo test -p partisync-ext-host` 30/30 绿（unit 12 + integration 4
      + P14 探针 6 + T03 探针 5 + smoke 3）
- [x] `clippy --all-targets -D warnings` 零警告；`fmt --check` 绿
- [x] `deny check` 零新增豁免；`audit` 零新增
- [x] **修正验证**：`p14_clock_granted_clock_probe_calls_host_function`
      在修正后通过且返回合理 ms（对抗审查独立复核：回退到 T02 版本
      重跑该探针确实 FAILED——`component imports instance
      'partisync:ext/clock@0.1.0', but a matching implementation was not
      found in the linker`，根因判断与修正均获独立实证）
- [x] **F-1/F-2/F-6 覆盖度诚实登记**：三类的**逐类隔离**未达成
      （TypeChecker 在首个无法解析处 bail，实测首个拒绝项是
      `wasi:io/poll` 而非 fs/net/clock）——已在探针 doc 注释 +
      properties.md P13 行 + 本卡如实登记；逐类隔离留后续任务
- [x] **F-2 措辞调和**：实施面达成的是**严格强于** per-call 的
      实例化期拒绝（import 静态解析，guest 代码零执行），properties.md
      P13 行按「以更强形式满足」登记
- [ ] CI 全绿 + 时长对照

## 钉子清单 disposition

n/a — T03 不触及 schema migration / unsafe / deny.toml / 跨 crate
pub API / crypto/auth；改动限于新 crate 内 `inject.rs`（注权注册路径）
+ 测试 fixture + 探针。

## 与 T02 的协作

T02（PR #29，main=`23f3b0e`）落地注权机制时以扁平名假设注册，本 PR
以真实 component fixture 实证并修正注册路径。修正后 T02 的
`linker_for_index_read_stub_injects_nothing_but_succeeds` 等 unit 测试
仍全绿（占位语义未变：声明 `index.read` 但未注入 interface 实例）。