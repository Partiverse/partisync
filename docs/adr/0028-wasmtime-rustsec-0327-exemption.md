# ADR-0028: wasmtime RUSTSEC-2026-0325/0326/0327 门禁豁免（漏洞路径均不可达）

版本: 0.2 · 状态: **接受**（2026-10-03 用户指令「批准ADR-0028，批准pr
合入」回填，随本 PR 合入生效；批准人 @lead——deny.toml 白名单红线项
经人工签字解锁）· 关联: RUSTSEC-2026-0325/0326/0327（bytecodealliance
同一修复波次）、ADR-0025（wasmtime 线位 >=47.0.4,<48；修订 6 判例）、
ADR-0003（toolchain 升级）、M6-WP04-T02（版本矩阵实测）、SPEC
M8-WP07-T02 拆卡 PR #101（被本 advisory 挡门，本 ADR 合入后解锁）
负责人: @lead · 批准人: @lead · 起草日期: 2026-10-03

## 背景

2026-10-03 起 RustSec DB 新增 wasmtime 47.0.4 三公告（bytecodealliance
同一修复波次，Solution 均为 `>=48.0.4,<49` 或 `>=49.0.2`，47.x 线内
无补丁）：

| ID | 漏洞 | 触达前提 | 本仓状态 |
|---|---|---|---|
| RUSTSEC-2026-0325 | WebAssembly tag imports 类型错标 → GC 堆损坏 | 宿主启用 exception-handling/tags | **未启用**（features 集无） |
| RUSTSEC-2026-0326 | `try_call` 跨界 GC rooting 缺失 → GC 堆损坏 | 宿主启用 GC / `try_call` | **未启用**（无 gc feature；WIT 面无 resource/record——ADR-0025 修订 6 实证） |
| RUSTSEC-2026-0327 | async-lifted callback 结果计数未校验 → 宿主栈溢出 | 宿主 async-lift exports | **零 async-lift 用法**（grep 实证 + 未启 `async` feature） |

cargo-deny 每次联网取最新 advisory-db → **所有在途 PR 的 deny 门禁
开始红灯**（main 基线 CI 37053396805 尚绿——红灯纯由 DB 更新时点
引入，非代码回归）。

约束矩阵：

| 事实 | 证据 |
|---|---|
| 本仓 wasmtime 线位 `>=47.0.4,<48`（47.0.4 无补丁版） | Cargo.toml:74，ADR-0025 决策节 |
| **47.x 系无修复版**（advisory Solution 只给 48/49） | RUSTSEC-2026-0327 |
| wasmtime 48/49 与 rust-toolchain 1.94 pin 不兼容 | M6-WP04-T02 实测（R2 版本矩阵），升级 = 连锁 toolchain bump（ADR-0003 全量回归） |
| 漏洞路径 = **async-lifted** exports 的宿主回调；本仓**零 async-lift 用法** | `grep -rn "async_lift\|func_wrap_async\|call_async" crates/partisync-ext-host/` = 空；wasmtime features 未启用 `async`（Cargo.toml:74-77：cranelift/component-model/runtime/wat/demangle/cache） |
| deny.toml ignore 属红线（需 ADR） | AGENTS.md 禁止事项 |

## 决策（草案）

**C1：deny.toml advisories.ignore 增加 `RUSTSEC-2026-0325/0326/0327`**，
附撤销条件（双触发任一即撤销并升级）：

1. ext-host 引入任何 async-lift 用法（`func_wrap_async` /
   `call_async` / wasmtime `async`/`gc`/`exception-handling` feature）
   ——**先升级后接线**，本条作为 PR 检查项写入 ext-host crate 注释；
2. toolchain 升级（ADR-0003 路径）使 wasmtime ≥48.0.4 可达时——
   随 WP 顺路升级并撤销本条。

## 备选否决

- **C2（升级 wasmtime ≥48.0.4）**：干净但被 toolchain pin 卡死——
  需 ADR-0003 全量回归 + M6-WP04 版本矩阵重测，WP07 主体（写回
  overlay）再次延后数会话；与本 advisory 的暴露面（我们不用 async
  lift）不成比例；
- **C3（不处置）**：门禁红灯不可接受（铁律 5），且每 PR 都红。

## 后果

- 正向：门禁恢复；处置与证据（零 async-lift 实证）可审计；撤销
  条件绑定真实触发器而非日期；
- 负向/风险：宿主侧 wasmtime 维持 47.0.4——但受攻击面需「宿主以
  async-lift 方式消费扩展结果」，本仓架构上不存在；ext-host 仅加载
  本机 `~/.partisync/extensions` 自管扩展（无远程分发，M7-WP03 供应链
  空白已登记），攻击者需先有本机文件写入权——届时沙箱假设本身已破；
- 中性：M8-WP07-bench/后续 KPI 报告引用本 ADR 作为 deny 豁免依据。

## 修订登记

| 版本 | 日期 | 修订 |
|---|---|---|
| 0.1 | 2026-10-03 | 初稿（提案，待签字） |
| 0.2 | 2026-10-03 | 签字回填：用户「批准ADR-0028」→ 接受；RUSTSEC-2026-0325/0326 纳入（同一波次三公告） |
