# ADR-0025: WASM 扩展运行时选型（wasm component 即 MCP tool）

状态: 草稿 · 日期: 2026-09-28 · 决策人: 待定（@lead + 架构双签）
关联: SPEC M6-WP04（评估 WP）、ADR-0024（Tauri 桌面壳，IPC 边界脚手架）、执行方案 §6.7

## 背景

M6 §6.7 要求对「WASM 扩展系统（学 Spacedrive 沙箱）」做评估。
PartiSync 已有两块可复用地基：(a) MCP 工具面（M4-WP03 L1/L2 闭环 +
`partisync-mcp` stdio 侧车 + 桌面壳 `mcp_call` IPC，M6-WP03 T05）；
(b) Tauri IPC capability 边界（ADR-0024 留口）。扩展形态据此锁定为
「沙箱化 MCP tool」。选型约束：铁律 8「无聊依赖」（新顶层依赖须
ADR + cargo deny）、Rust 1.94 toolchain pin、deny.toml 白名单纪律。
技术面基线（2026-09 核查）：WASI 0.3 已于 2026-06 落地，Component
Model 进入生产可信区；wasmtime 为 Rust 宿主事实标准。

## 决策

**待定**——本 ADR 以草稿状态登记选型框架与候选否决理由模板，
最终决策由 SPEC M6-WP04 的 spike 实测矩阵（T01/T03）填定。
当前倾向：**C1 wasmtime + WIT Component Model 直接嵌入**，
spike 实测不推翻即定稿。

## 备选方案

### C1 wasmtime + WIT 直接嵌入（当前倾向）

- 优点：Bytecode Alliance 事实标准、单一顶层依赖、Component Model
  原生、WASI 0.3 支持最完整；「无聊依赖」契合度最高。
- 代价：host 侧加载/注权/JSON 桥接 boilerplate 自担；wasmtime 为
  重量级 crate（编译时间与体积代价由 R1 缓解项量化）。
- 否决理由（如最终否决）：spike 实测 R1/R2 超标时启用。

### C2 Extism（wasmtime 之上的封装层）

- 优点：跨语言 PDK、宿主契约与 manifest 开箱即用，boilerplate 最少。
- 否决理由（倾向）：为省 boilerplate 引入第二层依赖耦合
  （extism 版本 ↔ wasmtime 版本联动升级），与「无聊依赖」和
  deny 白名单维护成本相抵；桥接层薄（JSON in/out）时自担成本可控。

### C3 Wassette 模式（参考设计，不引库）

- 优点：「wasm component 即 MCP tool」的语义与既有 MCP 工具面
  完全对齐，设计可直接借鉴（host function 白名单注权）。
- 处置：作为 M7+ 实施期的设计参照收录，不作为运行时依赖候选。

## 后果

（T03 定稿时填写：选定方案的正负面、放弃的东西、wasmtime/extism
的 cargo deny 试算输出、编译时间与二进制体积实测数字。）

## 重新评估条件

- WASI 0.3 / Component Model 工具链（wit-bindgen 等）出现破坏性
  版本抖动或安全 advisory；
- Spacedrive 扩展系统落地并给出可复制的生产范式；
- Rust toolchain pin 变更（1.94 → 更高）导致依赖面重估；
- spike 实测指标（SPEC M6-WP04 §2.3）任一超标且 C2 可达标。
