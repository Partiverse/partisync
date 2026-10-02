# M8-WP06 终止保障实装报告（R8 核销）

任务: M8-WP06-T01 · 日期: 2026-10-02 · 环境: darwin arm64 debug ·
规格: [specs/M8-WP06.md](../../specs/M8-WP06.md)（2026-10-02 批准 PR #90）·
wasmtime 47.0.4（lock 实际版本；API 查证于 47.0.0 同线源码）

## 1. 实装摘要

| 机制 | 配置 | 生效点 |
|---|---|---|
| **epoch**（真终止主机制） | `epoch_interruption(true)` + 100ms tick 线程（daemon）+ 每次 call 前 `set_epoch_deadline(100 ticks)`（= 10s 预算，与 gateway `EXT_CALL_TIMEOUT` 对齐） | 到点 trap → `CallError::Deadline` |
| **fuel**（确定性补充） | `consume_fuel(true)` + `FUEL_BUDGET = 1e9`（编译期常量，10× 余量，不进 manifest） | 耗尽 trap → `CallError::Fuel` |
| **外层 timeout** | gateway tokio timeout 保留 | 二层防御（tick 线程异常兜底），错误文本三路径可区分 |

装载/调用期初始化要点（实测踩坑修正）：
- `consume_fuel(true)` 下 Store 初始 fuel = **0**——canonical ABI 起始
  shim 即计费 → 装载期（实例化前）预注入 + 调用期重置；
- `epoch_interruption(true)` 下 Store 默认 deadline = 当前 epoch——tick
  一推进即 interrupt → 装载期设大 delta（`u64::MAX/2`；直接
  `u64::MAX` 会 `current + delta` 加法溢出，store.rs:2233 实测）；
- trap 文案在 anyhow **根因层**（顶层 to_string 只有 wasm backtrace）
  → `root_cause()` 判别 `interrupt`/`fuel`。

## 2. 探针结果（`tests/wp06_termination.rs`，3/3 绿）

| 探针 | 实测 |
|---|---|
| **死循环终止**（spin_loop.wat，手写 canonical ABI） | `loop (br)` guest **~1s 内被 fuel trap 强制终止**（fuel 先于 10s epoch 预算命中——br 计费低但 1e9 条耗尽即 ~1s）；SPEC §2.2「先到者终止」语义，Deadline/Fuel 皆证真终止 |
| **线程释放** | trap 返回后新工具调用成功（spawn_blocking 线程可复用——对照现状 timeout 假终止下线程永占） |
| **正常工具不受影响** | demo-tool 在预算内通过（余量断言） |
| **回归** | ext-host 全量 51 测试绿（47/47 存量 + 3 新探针 + 1，Store 初始化适配 14 处经 `init_termination_budget` helper）；gateway 47 测试绿 |

## 3. R8 核销（M7-WP01 §6-R8）

> 原登记：「扩展无 resource 限制：epoch/fuel 终止缺失，依赖外层
> timeout 假终止（wasm 超时后仍在后台空转）」

**已核销**：epoch+fuel 实装后 guest 在预算内被 wasmtime 强制中断
（探针实测），线程随 trap 释放；外层 timeout 降级为二层防御。R8 关账。

## 4. 偏差与遗留

| 项 | 说明 | 去向 |
|---|---|---|
| 终止归属实测为 fuel 先到 | 死循环形态 fuel 先于 epoch；纯 IO/长 sleep 型 guest 由 epoch 兜底——两机制互补，SPEC 契约内 | 无行动 |
| spin_loop fixture 为手写 WAT | canonical ABI（core call `(param i32 i32)->(result i32)` 返回 ret-area 指针）——validate 实测修正两轮 | 源码形态入仓可审 |
| fuel 预算常量未经真实重工具校准 | 1e9 按 demo-tool 10× 余量；重工具场景触发时按 §6-R2 处置 | 触发后调参 |

## 5. 复核日志

- 执行: GLM-5.3-Flash（ZCode，M8-WP06-T01）· 门禁: fmt/clippy 零 +
  ext-host/gateway 全量绿（CI 复跑）
- API 查证: 本地 wasmtime 47.0.0/47.0.4 源码（lock 实际 47.0.4）
