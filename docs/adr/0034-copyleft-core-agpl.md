# ADR-0034: 基座许可再调整——AGPL-3.0-only(核心下游强制开源)

状态: 已接受 · 日期: 2026-10-11 · 决策人: @lead（AI 代理起草，@lead 2026-10-11 会话内核签确认）
关联: ADR-0033（取代其决策 1/3 之许可条款，其余条款继续有效）· ADR-0000（决策 5 二次复议）· deny.toml（零改动，依据见后果）· docs/reviews/M11-roadmap-proposal.md §1.5（D19/Partiverse 影响面）

## 背景

1. @lead 2026-10-11 指示：「调整开源 license，核心部分要求下游强制开源」——许可策略的目标函数自「最大化嵌入采纳」（ADR-0033 的 rclone 式 permissive 逻辑）转为「下游强制开源」（copyleft 传染），闭源下游与云化白嫖的法律面要闭合。
2. ADR-0033 刚于 2026-10-10 将 Apache-2.0 转正并显式否决 AGPL；按「已接受 ADR 不可改、开新件取代」纪律，本件仅取代其**许可条款**（决策 1 及备选方案中的 AGPL 否决段），其余条款（商业扩展区边界、DCO 贡献策略、三阶段商业路线、「临时许可」义务终结）**继续有效**。
3. 「核心部分」的边界判定：本仓即引擎核心（D19：PartiSync = CLI + 库 + MCP 永久底座）；依赖方向 core ← graph/cas ← … ← 壳全链静态链接，下游链接 core 即构成 AGPL 意义上的合并作品——「按 crate 切分强弱许可」在分发语义上不可行且给下游虚假预期。故核心部分 = **全仓基座**，无例外 crate。

## 决策

**全部 workspace crate 自本件签核后的版本起以 AGPL-3.0-only 发布：任何下游分发或网络服务使用，必须依 AGPL-3.0 开源其修改与对应完整源码（含 §13 网络服务条款）；唯一例外是 ADR-0033 预留的商业扩展区（hub 治理平面，独立仓，不进本仓）。**

条款：

1. **字段与文本**：workspace `license = "AGPL-3.0-only"`；显式覆写的三个 crate（partisync-fuse、partisync-fuse-spike、partisync-wasm-spike）同步；根 LICENSE 换为 GNU AGPL-3.0 官方文本（gnu.org 原文，2026-10-11 下载校验 661 行）。
2. **选 AGPL-3.0 而非 GPL-3.0**：partisd（loopback MCP）、hub、mcp-http 均为网络服务面，仅 AGPL §13 能闭合「SaaS 使用不开源」漏洞。
3. **选 -only 而非 -or-later**：单一文本、下游合规可预期（MinIO/Grafana 判例）；避免 FSF 后续版本的不确定性进入授权判断。
4. **历史不可撤回**：v0.2.0-rc.1 及更早发布物/tag 仍为 Apache-2.0（构建自 tag 时点源码）；CHANGELOG 的 rc.1 节不追改（如实），许可变更随下一版本（≥rc.2/GA）changelog 登记。
5. **deny.toml 零改动**：其许可白名单约束的是**依赖准入**，与本仓自身 license 无关；AGPL 仓正常使用 Apache/MIT 系依赖，cargo deny/audit 门禁不受影响。
6. **README/ADR 指针同步**：README 许可证节改写；ADR-0033、ADR-0000 加取代指针。

## 备选方案

- **维持 ADR-0033（Apache-2.0 终局）**：否决——与 @lead 2026-10-11 方向指示直接冲突，本件即为复议。
- **GPL-3.0**：否决——网络服务面（daemon/hub/MCP）不受 GPL 传染覆盖，SaaS 下游可合规规避，达不到「强制开源」目标。
- **FSL/BSL 延迟开源**：否决——「延迟开源」不是「强制开源」，与目标不符；声誉成本 ADR-0033 已析。
- **按 crate 切分（核心 AGPL / 边缘 Apache）**：否决——静态链接下边缘 crate 分发即被传染，切分徒增复杂度并给下游虚假预期；且依赖方向上「边缘」不存在（全部最终依赖 core）。
- **AGPL-3.0-or-later**：否决——下游合规需追踪 FSF 未来文本演进，-only 更可预期。

## 后果

正面：

- 闭源下游（含云服务）必须开源：rc.1 后的引擎被嵌入专有产品或以网络服务形式使用即触发 §13/§4-§6 义务，基座专有化/白嫖的法律面闭合。
- 与商业分层更自洽：基座 copyleft 使 ADR-0033 商业扩展区（独立仓专有）的护城河不再依赖信任叙事；「开源核心+商业扩展」从信任约定升级为法律结构。
- 生态许可一致性：Partiverse V2 crate 级集成（graph/cas）后，Partiverse 成为 AGPL 合并作品——两仓同为 @lead 所有，对齐成本可控且方向一致。

负面（明示放弃的东西）：

- **嵌入采纳面收窄**：闭源产品捆绑、商业发行版集成从此法律上不可行，「rclone 式无摩擦嵌入」路线正式放弃；引擎渗透速度预期下修，增长改依赖开源生态与 Partiverse 前端。
- ADR-0033 刚建立的「Apache 终局」对外口径翻转，对外沟通成本一次性发生（以本件 + README 为准）。
- crates.io 发布面（条件债）执行时须核对 license 字段与包文档口径。
- **Partiverse 仓 license 需对齐检查**：若其现行为 permissive 且拒绝转为 AGPL 兼容，V2 crate 级集成方案须重议（进程隔离/链接面切割）——风险登记，不阻塞本件。
- 未来若走双许可（AGPL + 商业），需从零建 CLA；本件签核前外部贡献为零，CLA 窗口仍在。

门禁影响：docs-only + Cargo.toml metadata diff；deny/audit/toolchain/CI 门禁零改动。

## 重新评估条件

- Partiverse 侧明确不能接受 AGPL 传染 → 集成边界复议（进程面隔离 vs 再许可）；
- 企业客户对基座提出非 AGPL 授权诉求 → 双许可评估（先建 CLA）；
- FSF 发布 AGPL v4 → -only 锁定复议。

## 修订登记

- 2026-10-11 初稿（草稿）。
- 2026-10-11 @lead 核签接受（草稿 → 已接受），随 PR #231 合入生效。

## 待批准项（已清偿）

- ~~@lead 人工签核~~ → 已清偿：2026-10-11 会话内核签确认（Reviewed-By: @lead）。
