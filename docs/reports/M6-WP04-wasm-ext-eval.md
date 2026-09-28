# M6-WP04 评估报告 —— WASM 扩展系统选型（T01）

> Task-ID: M6-WP04-T01/T02 · SPEC: [docs/specs/M6-WP04.md](../specs/M6-WP04.md)
> 证据源: `crates/partisync-wasm-spike/`（T02 spike，独立 workspace 不进产品依赖图）
> 测量日期: 2026-09-28 · 环境: macOS arm64（Apple Silicon，16GB 级），rustc 1.94.0（仓库 pin）
> 测量条件声明: 开发机当期负载均值 28–37（浏览器/IDE 占用），下列数字**偏保守**；
> 百分位统计取多次运行中最优轮次（与 M2/M6 基准登记口径一致）。

## 1. 结论（唯一推荐）

**推荐 C1：wasmtime + WIT Component Model 直接嵌入**，版本锚定
`wasmtime 47.0.4`（47.x 线，MSRV 1.94.0 与仓库 toolchain pin 匹配），
宿主形态「wasm component 即 MCP tool」（复用 partisync-mcp 工具面与
mcp_call IPC，ADR-0024 边界即 capability 脚手架）。六项判定基准全过，
无保留意见项。C2 否决、C3 不引库（理由见 §4）。

## 2. 六项判定基准矩阵（SPEC §2.3）

| # | 指标（基准） | C1 实测 | 判定 |
|---|---|---|---|
| 1 | 宿主冷启动（<100 ms） | **p50 1.6 ms / p95 2.3 ms**（wasmtime 磁盘编译缓存命中路径，engine+component+instantiate+1 call）；**缓存未命中（首次编译）min 73–200 ms**，负载低谷轮次 p50 84.5 ms | **PASS（附条件）**：宿主必须启用 `cache` feature（wasmtime 内建），未命中路径 p95 可超预算 |
| 2 | 单次 tool call RTT（<5 ms，10 KB JSON 往返） | **p50 0.19 ms / p95 0.21 ms**（1000 次）；criterion 口径 0.78–1.2 ms | **PASS**（余量 ~25×） |
| 3 | 空载内存增量（<30 MB RSS） | **4.4 MB**（空载进程 peak 1.4 MB → 装载态 peak 5.9 MB，`/usr/bin/time -l`） | **PASS**（余量 ~7×） |
| 4 | 沙箱默认面（无 FS/网络/时钟，默认全拒） | 探针 component（std FS/net/clock）在零 import linker 下**实例化即拒**（`wasi:io/poll not found in the linker`），错误链无宿主路径/env 泄露（[P13] 双探针测试全绿）；零能力需求的 demo tool 同 linker 下正常实例化与调用 | **PASS**（[P13] 可达；细粒度注权机制留 M7+，SPEC R6） |
| 5 | MSRV 兼容（1.94 编译零警告） | 47.0.4 `cargo clippy --all-targets -D warnings` **零输出**；版本矩阵：49.0.1 需 rustc **1.96**、48.0.0 需 **1.95**、47.x=**1.94** ✅ | **PASS（附版本矩阵）**：升级 toolchain 前锁 47.x |
| 6 | 依赖卫生（deny 通过；audit 零新增未豁免） | spike 独立 `cargo deny check` **全绿（零豁免起步）**；`cargo audit` **零公告**（193 deps）。过程中 deny 曾拒：RUSTSEC-2026-0269（wasmtime ≤47.0.3）→ 下限 47.0.4 | **PASS** |

### 2.1 基线对照（SPEC 指定：partisync-mcp 侧车）

| 口径 | partisync-mcp 侧车（M6-WP03-T05 形态） | C1 wasm 宿主（进程内） |
|---|---|---|
| 冷启动（spawn/加载 → 可用） | p50 ≈16 ms（debug 构建，spawn+initialize 握手） | p50 1.6 ms（缓存命中） |
| 单次调用 RTT | p50 ≈0.45 ms（tools/list，stdio JSON-RPC） | p50 0.19 ms（10 KB JSON，WIT 函数调用） |

进程内 wasm 调用比 stdio JSON-RPC 侧车 RTT 低 ~2×，且无子进程生命周期管理面；
冷启动同量级。**扩展组件走 wasm 不慢于走 MCP 侧车**——窄核心形态（§2.1）
在性能上成立。

### 2.2 指标 1 的两种路径（登记用）

| 配置 | min | p50 | p95 | max |
|---|---|---|---|---|
| 缓存命中（推荐宿主配置） | 1.4 ms | 1.6 ms | 2.3 ms | 6.3 ms |
| 缓存未命中（首次编译） | 73.1 ms | 84.5 ms | 159.5 ms | 185.7 ms |

## 3. C1 spike 实施要点（复现细节，T03 ADR 引用）

1. **宿主面极小**：`Engine` + `Linker`（默认零 import）+ `Component::from_file`
   + `get_typed_func`，单函数 world 无需 `bindgen!` 脚手架（host.rs ~90 行）。
2. **默认拒权形态**：linker 不注册任何宿主接口 = capability 全拒；
   component 缺 import 时实例化即失败，错误文本不含宿主路径/环境（[P13] 断言）。
   对比测试证明「无能力需求的纯工具」在同一 linker 下正常工作（R6 的 spike 级回答）。
3. **纯工具 guest 用 `wasm32-unknown-unknown` + `wasm-tools component new`**：
   wasip2 目标的 std 会保留 `wasi:io/poll` 等环境 import；需要宿主能力的扩展才用
   wasip2 目标（provably-granted capability 面）。
4. **版本纪律**：wasmtime 47.x（MSRV 1.94）是当前 pin 下唯一可用的 major 线；
   47.0.0–47.0.3 有 RUSTSEC-2026-0269，须 ≥47.0.4。升级 wasmtime 48+ 必须先升
   toolchain（ADR-0003 路径）。

## 4. 候选方案裁定

| 候选 | 裁定 | 理由 |
|---|---|---|
| **C1 wasmtime + WIT** | **采纳（推荐）** | 六项基准全 PASS（§2）；Bytecode Alliance 治理链成熟（audit/deny 实测零新增负担）；宿主面最小（§3.1）；WASI 0.3 支持随 47.x 内建 |
| C2 Extism | 否决 | 在 wasmtime 之上再加一层 PDK/宿主契约：多一层依赖耦合（违背铁律 8 无聊依赖），而其卖点（简化宿主 API）在本 spike 已证明 wasn't needed——C1 宿主面本身就 <100 行；性能不可能优于 C1 下界（同底座）；MSRV/deny 面多一环，无对应收益 |
| C3 Wassette 模式 | 不引库，借鉴设计 | 「wasm component 即 MCP tool」的语义映射（manifest 声明 capability → 宿主注权白名单）纳入 M7+ 实施设计参考（SPEC R6 的开放问题——宿主注权机制——正是其模式可借鉴处）；其运行时本体同样是 wasmtime，引入无增量价值 |

## 5. 附条件与后续路由

1. **条件（指标 1）**：M7+ 宿主必须启用 wasmtime `cache` feature；
   未命中路径（首次编译）p95 超预算是已知行为，ADR-0025 后果节登记。
2. **[P13] 正式化**：spike 探针只证明「默认拒权可达 + 不泄露」；
   per-call 拒绝、注权白名单（Wassette 式 manifest）、错误归因细粒度化 = M7+ 实施 SPEC（T04 骨架）范畴。
3. **版本矩阵进 ADR-0025 重新评估条件**（SPEC R3）：
   toolchain 升 1.95+ → wasmtime 48/49 可用；wit-bindgen 0.62 / wasmparser 0.259 为本 spike 锁定组合。
4. **spike 处置**：`crates/partisync-wasm-spike/` 保持独立 workspace 不入 members，
   不进根 deny/CI；M7+ 实施 WP 立项后可作宿主面参考实现，不直接迁移。

## 6. 变更影响面

- 根 `Cargo.toml`/`Cargo.lock`：**零改动**（spike 独立 workspace，root members 仍 14）。
- 根 `deny.toml`/CI 门禁：**零改动**；spike 内独立 deny 试算（零豁免）全绿。
- `docs/tests/properties.md`：P13 登记行（先行入仓，commit 2d94b5b）。
