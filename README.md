# PartiSync

AI 时代的数字资产传输与多设备终端资产融合管理基建底座（Rust）。
定位、架构与选型见 [docs/partisync-调研与构建方案.md](docs/partisync-调研与构建方案.md)；
开发流程、审计体系与阶段计划见 [docs/partisync-AI开发执行方案.md](docs/partisync-AI开发执行方案.md)。

## 状态

M-1（工程基建与审计演练）进行中。里程碑报告见 `docs/reports/`。

## 快速开始（开发者）

```bash
cargo build --workspace
cargo test --workspace
cargo xtask trace <TASK-ID>   # 追溯任意任务的全链工件
```

提交前安装钩子：`./scripts/install-hooks.sh`（强制 Task-ID trailer）。

## 许可证

Apache-2.0（ADR-0000「临时决策」，随仓库公开转正；治理结构确定后复议）。

## CI

五道门禁：fmt / clippy / test(ubuntu+macos) / cargo-deny / task-ids。首跑记录见 docs/reports/。
