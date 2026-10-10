# PartiSync

Local-first 数据同步引擎：常驻核心守护进程（partisd）+ CLI + MCP 服务面，可选自托管 hub（partisync-hub）。
引擎优先、界面靠后：消费级 GUI 由姊妹项目 Partiverse 承接（D19 分工裁决，见 [M11 路线提案 §1.5](docs/reviews/M11-roadmap-proposal.md)）。

## 快速开始

```bash
cargo run -p partisync-cli -- --help   # index / ui / watch / ls / find / dedupe 等子命令
```

工具链版本见 [rust-toolchain.toml](rust-toolchain.toml)；crate 依赖方向与架构见 [AGENTS.md](AGENTS.md) crate 地图。

## 许可证

Apache-2.0（[LICENSE](LICENSE)）。此为终局决策、非临时标签——复议过程与商业扩展区边界见 [ADR-0033](docs/adr/0033-license-finality-and-commercial-tiers.md)。

## 状态与发布

- 版线：v0.2.0-rc 窗口进行中（发布节奏见 [docs/reviews/M10-WP06-release-cadence.md](docs/reviews/M10-WP06-release-cadence.md)）。
- 发布工件：GitHub Releases，附 minisig 签名 + CycloneDX SBOM，可复现构建（ADR-0027/0030）。

## 治理与贡献

单人（@lead）+ AI 代理作业模式：治理总纲见 [docs/partisync-AI开发执行方案.md](docs/partisync-AI开发执行方案.md)，架构决策档案见 [docs/adr/](docs/adr/)。
重大决策以 git 提交 trailer（`Reviewed-By`）为签核凭证；外部贡献以 DCO（`Signed-off-by`）接收，范围限于开源基座（见 ADR-0033 条款 3）。
