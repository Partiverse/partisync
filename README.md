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

- 版线：v0.2.0-rc 窗口进行中（发布节奏见 [docs/reviews/M10-WP06-release-cadence.md](docs/reviews/M10-WP06-release-cadence.md)）；变更历史见 [docs/release/CHANGELOG.md](docs/release/CHANGELOG.md)。
- 发布工件：GitHub Releases——tar/dmg 产物 + `SHA256SUMS` + 各产物 `.minisig` 签名 + 每 crate 一份 CycloneDX `.cdx.json` + 双构建对账 `reproducibility.txt`（签名与验证工具链见 ADR-0027/0030）。

## 验证发布工件

产物清单（以 GitHub Releases 实际 assets 为准）：`partisync-cli-<ver>-linux-x86_64.tar.gz`（五 bin：partisync-cli / partisync-mcp / partisync-mcp-http / partisync-fuse / hub-demo-web）、`partisync-cli-<ver>-macos-aarch64.tar.gz`（三 bin，无 partisync-fuse）、`PartiSync.Desktop_<ver>_aarch64.dmg`。

干净环境验签（完整步骤与公钥见 [docs/release/RELEASE-PUB-KEY.md](docs/release/RELEASE-PUB-KEY.md)，双公钥任一通过即有效）：

```bash
minisign -Vm SHA256SUMS -p partisync.pub
sha256sum -c SHA256SUMS
```

如实状态（不假绿）：CI 签名 secret 未配置时 Release 显式 unsigned；复现构建当前登记 **REPRODUCIBLE=no**（cargo 构建路径相关性），`reproducibility.txt` 随 Release 分发；macOS dmg 未公证；hub 产物为演示面（hub-demo-web）。扩展签名见 [docs/release/EXT-SIGNING.md](docs/release/EXT-SIGNING.md)。

## 治理与贡献

单人（@lead）+ AI 代理作业模式：治理总纲见 [docs/partisync-AI开发执行方案.md](docs/partisync-AI开发执行方案.md)，架构决策档案见 [docs/adr/](docs/adr/)。
重大决策以 git 提交 trailer（`Reviewed-By`）为签核凭证；外部贡献以 DCO（`Signed-off-by`）接收，范围限于开源基座（见 ADR-0033 条款 3）。
