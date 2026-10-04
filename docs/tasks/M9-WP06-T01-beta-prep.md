# Task: M9-WP06-T01 WP06 SPEC 起草 + v0.1.0-beta 发布准备件（离线可交付）

> **范围外（红线）**：不创建 GitHub Release、不 push tag、不 publish crate
> /Release（发布执行 = 用户确认 beta 口径后 T03）；不操控 GUI；release.yml
> 实改归 T02；M9-WP00 §1-WP06/§2 拍板项④状态回填随 WP06 关账（T03）。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP06-T01 |
| **类型** | 起草 + 准备件（R0 纯文档 PR；单 PR ≤400 行） |
| **优先级** | P0（M9-WP00 §1-WP06 γ 压轴 WP 的开工件；M8-report §5 债表 drop-caches 行「发布前 release 复测窗口」承接点） |
| **范围** | SPEC M9-WP06 全文 + changelog 草稿（docs/release/CHANGELOG.md v0.1.0-beta 节）+ 发布 checklist（docs/release/RELEASE-CHECKLIST-beta.md）+ 本卡；含本地可跑准备件核验记录（构建/SBOM 步骤 dry-run 实跑留痕） |
| **创建日期** | 2026-10-05 |
| **来源** | M9-WP00 §1-WP06/§2 拍板项④（⏸ 待落锤）+ M8-WP02 判例（SPEC + release-report §4 偏差 1/2）+ M8-report §5 债表/§8-4 + M8-WP07-bench §2 债 |

## 契约落点

1. **SPEC 发布清单**（沿 M8-WP02 §2 四件套复跑 + 偏差清偿）：签名管线
   （M8-WP02 已备，ADR-0027 双钥，复跑零改动核验）/ 双构建 hash 对照
   （release-report §4 偏差 1 清偿，reproducibility job）/ SBOM（偏差 2
   清偿，auditable 嵌入 + cyclonedx 独立导出）/ release workflow 修订
   （产物面扩展 + beta 产物名后缀）/ changelog beta 口径 / drop-caches
   冷缓存复测窗口（M8-WP07-bench §2 债，Linux 容器执行项挂 T03）。
2. **beta 口径三轴推荐落锤，显式标注「待用户确认后执行发布」**（M9-WP00
   §2 拍板项④）：版本号 tag `v0.1.0-beta`（Cargo version 维持 0.1.0——
   path-dep `version = "0.1.0"` 解析约束，pre-release Cargo 版本需全图改
   版本约束，改动面与收益不成比）；产物面三产物框架 + tar 包内扩
   partisync-mcp / partisync-fuse bin，hub 仍 = hub-demo-web（§4-4 债
   延续，如实标注 demo 面）；**含 FUSE 写回**（M8-report §8-4 原文
   「写回面 + by-hash 后的产品化节点」，边界作已知限制写入 notes）。
3. **准备件**：changelog 草稿自 v0.1.0-alpha（tag→84c342d）起按 WP 归组
   （M8 WP05/06/07 + M9 WP01–WP05，`git log v0.1.0-alpha..HEAD` 64
   commits 实测）；checklist 全步骤可脚本化（命令逐条给出，tag/Release
   步标注「待用户确认后执行」）。
4. **本地核验（只报实跑，2026-10-05）**：`cargo metadata --locked`
   → LOCK_OK（可复现构建 lock 前提）；`cargo build --locked --release
   -p partisync-core` → Finished 5.45s（release profile 叶子冒烟）；
   `cargo deny check` → bans/licenses/sources ok + advisories ok
   （依赖门禁）；`cargo install --locked cargo-auditable`（0.7.7，
   20s）+ `cargo auditable build --locked --release
   -p partisync-core` → Finished 1.67s（SBOM 嵌入路径干跑）；
   `cargo build --locked --release -p partisync-cli` → Finished
   6m00s（<10 分钟线，产物 38M 实测）。cargo-cyclonedx 本机未装，
   独立 SBOM 导出步骤为 T02 CI 步，本地不假跑。

## 边界与既有决定

- SPEC 状态「草稿（合入 = 批准）」沿 M9-WP00 判例（M8-WP00 PR #59）。
- 版本事实起草期核实（2026-10-05）：workspace `version = "0.1.0"`
  （Cargo.toml:23）、tauri.conf.json `version: "0.1.0"`（:4）、唯一 tag
  `v0.1.0-alpha`→`84c342d`；产物文件名判例
  `PartiSync.Desktop_0.1.0_aarch64.dmg`（release-report §1）。
- 发布动作零执行：本任务产物全部为仓内文档；tag/Release/publish 均在
  T03 且以用户确认为前置（红线，本卡与 SPEC §5 双重登记）。
- 任务卡先落卡后动工（本卡随 SPEC 同 PR 入仓，沿 M9-WP04-T01 判例）。
