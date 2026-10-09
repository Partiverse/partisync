# Kubuntu 迁移交接（macOS → Kubuntu，2026-10-09）

> 目的：开发机从 macOS 迁往 Kubuntu，实现无痛接续。本文件随 git 走，
> 是唯一需要在新机读取的交接锚点；zcode 会话级上下文另见项目记忆
> `migration-handoff-macos-to-linux`（2026-10-09 版）。
> 上一次同型迁移（2026-09-21，M3 期）判例已吸收，过时项已剔除。

## 1. 迁移时点快照

- **main = `247566d`**（origin/main 同步，工作区 clean）。GPG 签名提交，
  签名钥 `20F09F2BF9402C59`（Partiverse <partiverse@tuta.io>）。
- **M10 全部关账**（G3 = PR #204，三签回填 + Reviewed-By 补认 46 提交）：
  六 WP 全合入，里程碑报告 #203，44+ PR 对账在案。
- **唯一开放 PR：#167**（M10-WP02-T03 相对时间四档 + tag chips，head
  `11afa9f`，CI 8/8 全绿）——只差桌面 GUI 实操补验（见 §4 待办①）。
- **M11 提案 v0.2 已入仓待拍板**：`docs/reviews/M11-roadmap-proposal.md`
  （候选五线 + 推荐排序 + §4 拍板清单）。
- 本仓库 toolchain 钉 `1.94.0`（rust-toolchain.toml，rustup 自动装）。

## 2. 新机一次性初始化（按序）

1. **系统依赖**（桌面构建必需，沿 CI ubuntu job 判例）：
   ```bash
   sudo apt install build-essential pkg-config libssl-dev \
     libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
     librsvg2-dev libxdo-dev file
   ```
2. **rustup**（装 1.94.0 由 rust-toolchain.toml 自动驱动）。
3. **git hooks**：`.git/hooks/` 不随 clone 走——clone 后立即
   `./scripts/install-hooks.sh`（commit-msg Task-ID 检查）。
4. **GitHub 凭证**：`gh auth login`（HTTPS push 走 gh 凭证助手）。
5. **GPG 签名钥**（repo-local git config 已带 `commit.gpgsign=true` +
   `user.signingkey`，clone 后即生效，但**钥体在 macOS keyboxd**）。
   在旧机导出、新机导入：
   ```bash
   # 旧 mac：
   gpg --export-secret-keys 20F09F2BF9402C59 > partisync-signing.key
   # 新 Kubuntu（安全信道带走文件后）：
   gpg --import partisync-signing.key && gpg --delete-file partisync-signing.key 2>/dev/null
   echo "20F09F2BF9402C59:6:" | gpg --import-ownertrust   # ultimate
   rm partisync-signing.key
   ```
   未迁移前新机 commit 会签名失败——届时临时 `-c commit.gpgsign=false`
   仅限本地未推送提交，推送前必须补签或等钥到位。
6. **网络代理**（中国大陆网络时）：旧机 clash-verge 监听 127.0.0.1:7897；
   Kubuntu 侧按需重装并配置 git 代理
   `git config --global http.https://github.com/.proxy http://127.0.0.1:<port>`。
   仓内无代理配置（已核实），不会带坏新机。
7. **cargo 源**（可选）：`~/.cargo/config.toml` 配 rsproxy（机器级，
   勿入仓——M6 双源 patch 错位置判例）。
8. **zcode 侧**（与仓库无关，需手工带走/重挂）：
   - `~/.zcode/AGENTS.md`（用户级十条规则）；
   - `~/.zcode/cli/config.json` 的 `mcp.servers`（partisync-mcp）；
   - `~/.zcode/skills/` 与 `~/.agents/skills/`（partisync-* 治理技能 +
     通用技能）；`~/.zcode/commands/`（如有）；
   - zcode hook（Mimosa commit/push 前置扫描）在新机重装；
   - **记忆重挂**：project key 按工作区路径哈希，Kubuntu 路径不同 →
     新 key。首次打开新工作区生成目录后，把 mac 的
     `~/.zcode/cli/memories/projects/partisync-dev-513f89be40964bf2/memory/`
     整体拷入新 key 目录（`MEMORY.md` 索引 + 全部 `*.md`）。

## 3. 平台差异与判例（macOS 判例哪些还成立）

| 项 | macOS 判例 | Kubuntu 处置 |
|---|---|---|
| numkong 本地 patch | Apple Clang 16 SIGABRT，build.rs 注释 FP8 + stub（**不入仓**） | **不需要**，Linux 直接原版构建 |
| 桌面 GUI 验收驱动 | osascript System Events **AXPress**（小热区最可靠）/ cliclick / `screencapture -x` | 换血：X11 用 `xdotool`（click/key/windowactivate）+ `import`/`scrot`；KDE Wayland 用 `ydotool`/KWin 脚本 + `spectacle -b -n -o <file>`。**验收硬性规则不变**：真实实操 + 截图归档 docs/screenshots/ + Read 自验 |
| 智能引号坑 | osascript 传中文经 pbcopy 粘贴 | xdotool type 直敲中文仍建议走剪贴板（`xclip -selection clipboard`）+ Ctrl+V，规避 IME/布局差异 |
| FUSE 探针 | 必须 Docker 容器 + sudo /dev/fuse（M7 判例，alpine apk） | 同判例成立；Linux 也可试直挂（macOS 才强制容器） |
| **drop-caches 复测窗口** | 挂「待 Linux 台架」（M10-WP06-T03 登记） | **本机即可执行，债直接解锁**：`sync && echo 3 | sudo tee /proc/sys/vm/drop_caches` 后沿 M8-WP01-T03 冷缓存基准口径复测 |
| 基准数字口径 | 冷启动/P99 等基线多为 Apple Silicon 实测 | 与 Linux 数字**不可直接对比**，验收重开口径（2026-09-21 判例延续） |
| 磁盘纪律 | 460G 卷三次被多代理全量测试灌满（APFS 容器他卷占 ~400G） | 协议磁盘纪律仍适用：跑全量测试前 `df` ≥15G、复用 target、每任务全量最多两轮 |
| CI 对齐 | 本机 macOS 与 macos-latest runner 同族 | 本机 Linux 与 ubuntu-latest 同族——CI 首轮红可先本地复跑定界 |

## 4. 新机首日待办卡（按序）

1. **#167 GUI 补验关账**（10–15 分钟）：按 PR #167 评论区手册——
   原 demo 语料（`../partisync-gui-demo`）不迁移，需按手册重建 11 条
   种子（sidecar 真实链路 + 相对时间四档边界 + 回拨自洽）；验证点：
   四档相对时间显示 / >30 天回落绝对 / title 悬浮完整时间 / tag chips
   过滤。PASS 后摘 label → squash 合入 → WP02 全绿、M10 零 OPEN PR。
2. **drop-caches 复测**（窗口已解锁，见 §3；结果回填 M10-WP06-T03
   登记处）。
3. **M11 拍板**：读 `docs/reviews/M11-roadmap-proposal.md` §4 拍板清单，
   拍板后新会话走 M11-WP00 开局（`partisync-task-bootstrap`）。
4. **债表消化顺序**（M10 报告 §债台账）：D5 sidecar 写锁（GUI 主线直接
   受益）→ Mimosa 完整审计窗口（上次 deep 重扫结论 **inconclusive**，
   封印 `scan-…-1345bae55067`，xtask git() finding 已判 accepted-by-design）
   → 示例扩展生产钥签名（密钥持有人动作）→ flaky 三颗（sync
   m5_wp03.rs:713 / fuse probe_mount.rs:246 / hub m5_wp06.rs:77）。

## 5. 本地不迁物与重建法

- `target/`：Linux 全量重编（首次 `cargo test --workspace` 约 20–40 分钟）。
- `../partisync-gui-verify` worktree 与 `../partisync-gui-demo` 语料：
  不迁；#167 种子按手册重建（§4-①）。
- `~/.mimosa/security-scans/`：不迁；封印 id 已记录于记忆与本文档。
- 本地残留分支（m10/wp01-spec 等已 squash 合入的本地分身）：无内容
  价值，新机 clone 后自然不存在，无需处理。

## 6. 会话谱系与续接

- 项目记忆（重挂后）入口：`MEMORY.md` → `m10-full-execution-workflow`
  （最新终态）→ `migration-handoff-macos-to-linux`（本文档的会话侧副本）。
- 跨机续接可用 `ReadSessionContext`：接手会话 `sess_7bfb93ba`（M10 全量
  执行 + 本迁移交接）。
- 协作规则：repo `AGENTS.md`（十条铁律/提交格式/桌面验收硬性规则）
  随 git 走；用户级偏好见 `~/.zcode/AGENTS.md`（§2-8 随身带走）。

——交接完。新机 `gh auth login` + hooks + GPG 三件就位后，从 §4-① 开工。
