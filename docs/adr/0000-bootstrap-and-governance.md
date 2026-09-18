# ADR-0000: 仓库引导——结构、工具链、安全基线与许可

状态: 已接受 · 日期: 2026-09-18 · 决策人: @lead（本引导阶段由 AI 代理起草，人工签核待补）
关联: 执行方案 §6.0（M-1-WP01）、调研方案附录 A

## 背景

PartiSync 从零开始建立代码库。首批决策将约束后续所有开发，需要一次定案并留档。

## 决策

1. **仓库形态**：单一 cargo workspace，12 个产品 crate（partisync-core/graph/cas/provider/transfer/sync/index/ai/gateway/hub + partisd + partisync-cli）+ 1 个开发工具 crate（xtask）。依赖方向：core ← graph/cas ← provider/transfer/sync ← index/ai/gateway ← 壳（见根 AGENTS.md crate 地图）。
2. **工具链**：rust-toolchain.toml 钉住 `1.93.1`（含 rustfmt/clippy）。升级工具链需新 ADR + 全量回归基准。
3. **Rust edition = 2021**。理由：在本仓库「无聊地基」原则下选择生态兼容面最大的 edition；2024 edition 的收益（RPIT lifetime 捕获等）对当前骨架无必要。重新评估条件：首个真实需要的 2024 特性出现时。
4. **安全基线**：workspace 级 `unsafe_code = "forbid"`（调研方案 §5.2 零 unsafe 政策）。确需 unsafe 的模块（io_uring/FUSE/ort 预计在 M1–M4）以 ADR 前提在该 crate 内显式放宽为 `deny` 并说明。
5. **许可证 = Apache-2.0（临时决策）**：保证依赖兼容面（deny.toml 白名单以 Apache/MIT 系为主）与未来商业化切换空间。**标记为临时**：治理结构（基金会/公司/社区）确定后复议——Spacedrive（AGPL→FSL）与 AList（治理危机）的教训写入调研方案 §2.5/§2.6。
6. **文档语言 = 中文，代码标识符与 crate 名 = 英文**：团队工作语言为中文；代码遵循生态惯例。
7. **AI 作业规范**：以仓库根 AGENTS.md 为第一上下文，执行方案为治理总纲；提交 trailer 强制 Task-ID（本地钩子 + CI 双重校验）。

## 备选方案（摘要）

- 多仓库（各 crate 独立库）：早期协调成本过高，放弃；联邦化时再评估 hub 拆仓。
- edition 2024：无即时收益，见决策 3。
- GPL/AGPL：与未来「开源核心 + 商业扩展」路径冲突风险大，见决策 5。

## 后果

- 全仓 `cargo fmt/clippy/test` 单命令可跑；CI 门禁可在一台 runner 上完成（M-1 阶段）。
- forbid(unsafe) 意味着 M-1 阶段不能引入任何 FFI；与 M-1 范围一致。
- 许可证复议前，所有对外发布的产物须附带「临时许可」说明。

## 重新评估条件

- 工具链 1.93.1 出现安全通告 → 立即升级 + 全量回归；
- 治理结构确定 → 许可证复议（决策 5）；
- 任一能力层 crate 出现对另一能力层的依赖需求 → 依赖方向复议（AGENTS.md crate 地图）。
