# SPEC: M7-WPnn —— WASM 扩展运行时实施（骨架草案）

版本: 0.1-draft · 状态: **草稿（未批准）** · 关联: ADR-0025（已接受
2026-09-28）、SPEC M6-WP04（评估前置 WP）、ADR-0024（IPC capability
脚手架）
负责人: 待定 · 批准人: @lead · 批准日期: 未批准

> 本文件是 SPEC M6-WP04 §3 T04 的交付物：M7+ 实施 WP 的 SPEC 骨架
> 草案。**仅文件入仓，未获批准**——WP 编号（M7-WPnn）、任务拆分、
> 验收细化由 M7 开局会话补全并走 partisync-spec-draft 批准流程。
> 批准前不得据此编写任何实现代码（铁律 1 规格先行）。

## 1. 动机

（M7 开局补全。已锁定锚点：）

- ADR-0025 已接受：wasmtime `>=47.0.4, <48` + WIT Component Model，
  宿主形态「wasm component 即 MCP tool」——扩展实现既有 MCP 工具
  语义（JSON 入参 → JSON 出参），经宿主加载后被 `partisync-mcp`
  工具面与桌面壳 `mcp_call` IPC 无差别调用
- 评估证据：[M6-WP04 评估报告](../reports/M6-WP04-wasm-ext-eval.md)
  （六项基准全 PASS 实测矩阵 + partisync-mcp 侧车基线对照 +
  spike 宿主面参考实现）
- 不解决的代价：扩展能力缺位，每次新工具需求走「新 MCP tool 进
  partisync-gateway + 全门禁」重路径，核心持续膨胀（M6-WP04 §1）

## 2. 契约

（M7 开局细化；已锁定的边界：）

### 2.1 宿主面

- wasmtime 依赖入根 `[workspace.dependencies]`，版本线位与 feature
  集按 ADR-0025 决策节 diff 落地；`cache` feature **必选**（冷启动
  预算前提，未命中路径 p95 超预算是已知行为，首载预热消化）
- 宿主 crate 位置与依赖方向待定（候选：gateway 下新模块或独立
  crate，遵守 crate 地图「只允许向下依赖」；跨层反向依赖需 ADR）

### 2.2 沙箱与注权（R6 处置方向，本 WP 核心增量）

- **P13 全量化**：per-call 拒绝 + 错误归因细粒度化（M6 spike 只证得
  实例化期默认拒 + 错误不泄露宿主路径/env）
- **宿主注权机制：Wassette 式 manifest**——component 侧声明所需
  capability，宿主白名单校验通过后注入对应 host function。处置
  SPEC M6-WP04 R6 开放问题（「默认全拒」与「tool 需要部分宿主能力」
  的冲突）
- 注权面最小集（候选，批准时收敛）：索引只读（复用既有 MCP 工具
  语义）；无 FS 写；无网络；时钟按需。每类 capability 独立探针

### 2.3 调用面

- 扩展工具与内建工具同语义可达：partisync-mcp 工具面列举 +
  桌面壳 `mcp_call` 调用（M6-WP03 §2.3 IPC 面不扩口）
- 扩展工具结果须标注扩展来源，不隐式获得内建工具信任级别

## 3. 验收标准（可执行）

（骨架条目；批准前逐条细化并挂 property ID：）

- [ ] P13 全量：未注权 component per-call 调用 FS/网络/时钟 API
      必败，错误文本不泄露宿主路径/env（扩展现有 P13 登记行的
      语义覆盖面，先于实现代码登记——partisync-property-registry 路由）
- [ ] 注权 manifest 校验失败 → 加载即拒，拒绝先于任何宿主函数暴露
- [ ] 六项基准复测全 PASS（SPEC M6-WP04 §2.3 同口径，含 cache 命中
      路径），数字入 M7 里程碑报告
- [ ] `cargo clippy -D warnings`（1.94 pin）/ `cargo deny check` /
      `cargo audit` 门禁零新增豁免（deny.toml 若需改动，走
      ADR-0025 修订登记，不开新 ADR）
- [ ] CI 时长对照入实施 PR（wasmtime 编译代价兑现量化，ADR-0025
      后果节登记的负面项闭环）

## 4. 非目标

| 项 | 推迟至 | 理由 |
|----|--------|------|
| **扩展自定义数据模型随空间同步** | 独立研究 WP（不设里程碑） | graph schema 演进大坑（Spacedrive 未交卷的领域），M6-WP04 §4 原样继承，严禁混入实施 WP |
| UI/前端扩展点（桌面壳插件面板） | M7+ 后续 WP | 先工具面后界面（M6-WP04 §4 继承） |
| 扩展分发/签名/registry | 企业特性议题 | 供应链面，需独立威胁模型（M6-WP04 §4 继承） |
| WASI 0.3 全依赖面（HTTP/sockets 等 hosted 能力） | 注权机制成熟后 | 第一版 capability 白名单最小集（M6-WP04 §4 继承） |

## 5. 涉及文件清单

（骨架；M7 批准时定稿为唯一编辑面。预期增量：）

```
crates/<host-crate>/**           # 宿主 crate（位置待定，§2.1）
docs/specs/M7-WPnn.md            # 本 SPEC 正式化
docs/tests/properties.md         # P13 行扩展（per-call 拒绝语义）
Cargo.toml / deny.toml           # wasmtime 入根（按 ADR-0025 线位）
docs/reports/M7-WPnn-*.md        # 基准复测报告
```

## 6. 风险与开放问题

| # | 风险 | 概率 | 影响 | 处置方向 |
|---|------|------|------|---------|
| R1 | 注权 manifest 设计过宽 → 沙箱面失守 | 中 | 高 | 白名单最小集起步；每类 capability 独立 property 探针；注权语义先登记后实现 |
| R2 | wasmtime 编译时长拖慢 CI | 中 | 中 | 实施 PR 附 CI 时长对照；必要时裁剪 feature（wat/demangle 可关） |
| R3 | wasmtime 47.x 线安全公告无 patch 内修复 | 低 | 高 | ADR-0025 重新评估条件已登记（升 toolchain 或新 ADR） |
| R4 | 扩展工具与内建工具权限混淆 | 中 | 中 | mcp_call 面标注扩展来源（§2.3）；扩展结果不隐式获得宿主信任 |
| R5 | WASI 0.3 / wit-bindgen 版本抖动 | 中 | 中 | ADR-0025 锁定组合（wit-bindgen 0.62 / wasmparser 0.259）；升级走重新评估条件 |
