# ADR-0024: Tauri 2 桌面壳运行时纳入 —— macOS/Linux 优先，iOS/Android 留 M7+

版本: 0.2 · 状态: **已接受（2026-09-27 用户拍板）** · 关联: SPEC M6-WP03
（产品化演示桌面壳骨架）、执行方案 §6.7（M6 产品化滚动阶段四项
主题之一）、ADR-0022（Task-ID 正则扩展·D 档命名约定）
负责人: @lead · 起草日期: 2026-09-27

## 背景

M6 §6.7 列出滚动产品化的四项主题： Tauri 2 桌面壳、WASM 扩展系统
评估、SMB 桥接评估、企业特性。其中「Tauri 2 桌面壳」是产品化
**最高可见度**的可交付面（投资演示/招聘 demo/用户上手 GUI），也是
唯一一项需要 **新增顶层依赖** 的工作（其它三项可以走纯评估路径，
零新增依赖）。

M6-WP01（M6:C 路径「产品化演示包」）已落地：

- `scripts/demo.sh` 一键演示（拉 LCSTS → 启 hub → 索引 → 5 demo
  查询， 实测 < 5min， 见 commit `c0d00b3`）
- `crates/partisync-cli/ui.html` 单页演示面（5 个 JSON 端点 + JS 前端，
  `partisync ui` 子命令启动 axum 服务于 127.0.0.1）
- 三段 demo 录制脚本（demo-narratives.md）

但 `partisync ui` 走 axum 公开 HTTP（绑 127.0.0.1 是当前妥协）， GUI
形态在浏览器 tab， 投资演示/招聘场景下需要**原生窗口体验**：
系统托盘 + Dock 图标 + 启动闪屏 + 菜单栏快捷键 + 全局快捷键 +
键盘焦点切换 + 窗口记忆（位置/尺寸/最大化态）。 这些都是浏览器 tab
给不出的。

Tauri 2（稳定版 2.12.0， Apache-2.0 OR MIT 双协议）满足：

- 系统 webview： macOS WKWebView / Linux webkit2gtk-4.1 /
  Windows WebView2（无 Chromium 打包， 二进制增量 < 5MB）
- Rust 工具链下限 1.77（仓库 1.94 远超）
- 内置 IPC（`tauri::command`）替代 HTTP-JSON 折中
- 侧车 sidecar（`tauri::api::process`）天然适合包装 `partisync-cli` 与
  `partisync-mcp` 子进程
- WASM 扩展系统的天然载体（M6-WP03 留口， M7+ 接 Spacedrive 经验）

## 决策

将 `tauri = "=2.12.0"` 与配套构建时依赖 `tauri-build = "=2.7.0"` 纳入
workspace，顶层 `[workspace.dependencies]` 新增精确锁版本条目。
`tauri-build` 版本经 crates.io 实测修正： tauri 2.12.0 在
`Cargo.toml` 内要求 `tauri-build ^2.7`， 故 `=2.7.0`（2026-09-26 release）
为匹配 patch； 初稿 `=2.0` 为误判， 已在 M6-WP03-T01 实施期纠正。
新增 workspace member `crates/partisync-desktop`， 作为 Tauri 2 桌面壳
二进制入口（crate 名 `partisync-desktop`、二进制名 `partisd-desktop`）。
不在现有 `partisync-cli` 上叠加 Tauri——CLI 已有 axum HTTP 演示路径
（`partisync ui`）， Tauri 与 axum 是两条独立 UI 入口， 共用同一份
`ui.html` 资产（通过 build.rs `include_str!` 嵌入）。

精确 diff：

```diff
# Cargo.toml
 [workspace.dependencies]
 ...
+tauri = { version = "=2.12.0", features = ["protocol-asset"] } # ADR-0024（M6-WP03 桌面壳）
+tauri-build = "=2.7.0" # ADR-0024（build.rs 资源嵌入； 与 tauri 2.12.0 配对）
+serde_json = { workspace = true } # 已在 workspace.dependencies

# workspace.members 新增
 "crates/partisync-desktop",

# crates/partisync-desktop/Cargo.toml（新建）
+[package]
+name = "partisync-desktop"
+version.workspace = true
+edition.workspace = true
+license.workspace = true
+description = "PartiSync 桌面壳（Tauri 2， SPEC M6-WP03）"
+
+[build-dependencies]
+tauri-build = { workspace = true }
+
+[dependencies]
+tauri = { workspace = true }
+serde = { workspace = true }
+serde_json = { workspace = true }
+partisync-gateway = { path = "../partisync-gateway", version = "0.1.0" }
+partisync-graph = { path = "../partisync-graph", version = "0.1.0" }
+partisync-cas = { path = "../partisync-cas", version = "0.1.0" }
+
+[[bin]]
+name = "partisd-desktop"
+path = "src/main.rs"
```

deny.toml 无需改动（Tauri 2 = Apache-2.0 OR MIT 双协议， 已在
`[licenses].allow` 白名单）；版本锁精确到 patch（`=2.12.0` /
`=2.0`）符合 ADR-0019/0020 既有 house style。

**显式不在本 ADR 范围内**（避免范围蔓延， 各自有独立决策空间）：

- 移动端（iOS/Android）： webkit-mobile 路径与 wry 桌面路径构建差异
  显著（Xcode 项目生成、 Gradle 集成、 移动安全模型）， 需独立 ADR
  + macOS dev toolchain。 留 M7+ 评估
- 自定义协议 URI（`partisync://`）： Tauri deep link 涉及 OS 注册，
  与本期 MVP 解耦
- 代码签名与公证： M6 §6.7「企业特性」议题， 单独路径

## 备选方案

| 备选 | 评估 | 否决理由 |
|------|------|----------|
| **A. 采纳 Tauri 2.12.0 + 新增 `partisync-desktop` 子 crate**（采纳） | Apache-2.0 OR MIT 合规； 系统 webview 不打包 Chromium → 二进制增量 < 5MB； Rust 工具链兼容（≥ 1.77 vs 仓库 1.94）； sidecar 模式天然适合包装现有 CLI/MCP | — |
| B. 用 Electron 包装现有 `partisync ui` HTTP 端点 | 零 Rust 改动 | Chromium 打包 +150MB 二进制； Node 工具链（AGENTS.md「无聊依赖」·新工具链 ADR）； 与现有 Rust 单一技术栈冲突 |
| C. 用 egui / iced 原生 Rust GUI | 零外部运行时 | 当前无现成 HTML/CSS 资产可用（`ui.html` 重写成本高）； 投资演示视觉表现力弱于 web； 偏离「产品化」语义 |
| D. 把 Tauri 直接加到 `partisync-cli` crate | 少一个 crate | CLI 已有 axum HTTP 演示路径； Tauri 是独立 UI 入口； 混在一个 crate 后续 Tauri-build 资源嵌入与 axum Router 共存易踩坑（`tauri::generate_context!` 与 axum Router 不能在同一二进制实例化两次） |
| E. 推迟桌面壳到 M7，先做 WASM 扩展评估 | 零新依赖 | §6.7 四项主题并列， 桌面壳是产品化「门面」， 与 M6-WP01 演示包紧耦合； 推迟意味着 M6 阶段 GUI 缺口持续； 用户已选 A |
| F. 用 Slint / Druid | 工具链轻 | 生态成熟度远低于 Tauri（system webview + 跨平台 + sidecar 三件套）； 视觉自定义成本高 |

## 后果

正面：

- macOS + Linux 桌面壳可与现有 `partisync-cli ui` 同源（同一份
  `ui.html`）， 投资演示/招聘 demo 双入口（CLI 演示 + GUI 演示）
- 移动端（iOS/Android）路径独立留 M7+， 不阻塞桌面壳收官
- Tauri 2 IPC (`tauri::command`) 可作为后续 WASM 扩展系统的天然
  沙箱边界（SPEC M6-WP03 §非目标登记， 但脚手架就位）
- 不打包 Chromium → 二进制增量 < 5MB； Linux 仅依赖
  `webkit2gtk-4.1` 系统库（Debian `libwebkit2gtk-4.1-dev` /
  Fedora `webkit2gtk4.1-devel`）， macOS 自带 WKWebView
- 无新 license 风险（Apache-2.0 OR MIT 已在白名单）

负面 / 放弃：

- 新增顶层依赖 `tauri` 与 `tauri-build`（违反「无聊依赖」直觉但
  必需； ADR 记录理由）
- 新增 crate `partisync-desktop`（+1 workspace member）
- CI 增加 Tauri 资源嵌入 build 步骤（`tauri-build`）， Linux runner
  需预装 webkit2gtk-4.1。**修订 3（2026-09-27 T02 同步， 见后）**：
  ubuntu-latest runner **不**默认带 webkit2gtk-4.1（2026-Q3 起 GitHub
  Actions runner image 已剥离系统级 webview 依赖）， T01 期间 PR #3
  CI run `36302844159` 双 FAIL 即此根因； commit `c04e5b0` 在
  `ci.yml` clippy + test (ubuntu-latest) job 与 `nightly.yml` chaos
  job 各加一个 `install Tauri 2 Linux system deps` step， 装 Tauri
  官方推荐 6 包：`libwebkit2gtk-4.1-dev build-essential libxdo-dev
  libssl-dev libayatana-appindicator3-dev librsvg2-dev`。 macOS
  runner 自带 WKWebView 无影响
- `partisync-desktop` 与 `partisync-cli` 双二进制并存， 用户认知成本
  小幅上升（README 标注「GUI 用户用 `partisd-desktop`， CLI 用户用
  `partisync`」）

deny / audit 影响（**实测输出待 T01 实现期补全**——本 ADR 起草时
Tauri 尚未纳入 workspace.dependencies， 无法跑 `cargo deny check`
全量 + `cargo audit`； T01 任务第一动作即是落实 Tauri 入 workspace
并跑 deny/audit， 输出粘贴至本节并 PR review 必查）：

- **license 基线（已实测， 本会话 2026-09-27 07:04 UTC）**：
  `cargo deny check licenses` 在本仓库（不含 Tauri） 当前状态输出
  `licenses ok`， 证明现有 deny.toml 白名单自身是闭合的
- **license 期望（基于 crates.io 元数据）**： Tauri 2.12.0 =
  Apache-2.0 OR MIT（双协议字段， `cargo-deny` 取首项报「Apache-2.0」，
  已白名单）； wry / tao / ico / png / objc2 / cocoa / webkit2gtk-sys
  / windows-sys 传递链均为 MIT 或 Apache-2.0， 已白名单
- **advisories**： 本仓库当前 advisories 数据库 fetch 在本会话
  失败（GitHub HTTP2 偶发不稳定， 与本 ADR 决策无关）， 既有
  ignore 列表覆盖 ADR-0014/0016/0020/0021 历史豁免； T01 实现期
  必须重跑 `cargo audit`， 若 Tauri 传递链出现新 advisory， 按
  ADR-0014/0020 既有「可达性论证」模式处理（豁免或修复）
- **重复版本（`multiple-versions = "warn"`）**： wry 0.4x 可能与
  现有 webview 相关 crate 冲突——本仓库当前无 webview 相关依赖，
  预期为零冲突； T01 实测确认
- **CI 影响**： 原以为 Linux ubuntu-latest runner 已默认带
  `libwebkit2gtk-4.1-dev`、 0 额外 CI runner 配置——此假设**错误**。
  **修订 3（2026-09-27 T02 同步， 已在 ci.yml / nightly.yml 落地）**：
  ubuntu-latest runner 实际不默认带 webkit2gtk-4.1， PR #3 run
  `36302844159` 实证（glib-sys build script 找不到 glib-2.0）。
  修法见上面「负面 / 放弃」段同位置。 本 ADR 起草期未实测 CI 假设
  是治理遗漏——后续 ADR 涉及 CI runner 系统库的， 必须先在 runner
  image 上跑一次 `apt list --installed | grep <lib>` 验证假设再写。
  macOS runner 自带 WKWebView 不受影响
- **deny.toml 不修改**： 现有 `[licenses].allow` 列表已涵盖
  Tauri 全传递链许可证， 无需新增条目
- **deny.toml 修改（T01 实测修订， 2026-09-27 用户拍板 A 路径）**：
  `cargo deny check licenses` 实测失败 ——
  `target-lexicon v0.12.16`（GTK 传递链 `cfg-expr` → `system-deps`
  的纯 Rust 间接依赖） license = `Apache-2.0 WITH LLVM-exception`，
  非裸 Apache-2.0。 该 license 是 Apache-2.0 + LLVM 例外条款，
  OSI/FSF 认可， 与现有白名单 `Apache-2.0` 实质兼容但字面不同。
  deny.toml 精确 diff：

  ```diff
  [licenses]
  allow = [
      "Apache-2.0",
  +   "Apache-2.0 WITH LLVM-exception", # ADR-0024 amend（Tauri 传递链 target-lexicon， GTK 链纯 Rust 间接依赖）
      "MIT",
      ...
  ]
  ```

  本修订属「同决策空间」（Tauri 纳入）， 且 ADR-0024 尚未合并入
  main（仍处预部署阶段）， 故不另开 ADR-0025。 此注释即修订溯源。

- **advisory ignore 修订（T01 audit 同步修订， 2026-09-27 用户
  拍板 A 路径）**： `cargo audit` 实测命中 5 条漏洞 —— 4 条
  已在现有 deny.toml ignore 列表（ADR-0020 rsa、 ADR-0021 h2 /
  webpki 三条）； 1 条 **新增**：

  - **RUSTSEC-2026-0253 lru `LruCache::pop()` UAF** —— 两条传递链：
    - `lru 0.7.8` → `reed-solomon-erasure 6.0.0` → `partisync-cas`
      （ADR-0014 已 pin v6， lru 升级需等上游 RS-erasure 跟进）
    - `lru 0.16.4` → `tantivy 0.26.2` → `partisync-index`（workspace
      pin 0.26， 升级需等 tantivy 0.27+）
  - **可达性论证**（按 ADR-0020/0021 既有模式）：
    - 触发条件 = `LruCache::pop()` 期间 panic， 同时持有一个已弹
      出元素的引用； 这是 **复合失败** 路径， 需用户输入 + 内部
      bug + 引用生命周期同时对齐
    - `partisync-cas` 修复路径： 信任本地分块（M0-WP03 P4 不变），
      损坏走 error 返回非 panic， 无用户控制数据流入 LRU cache key
    - `partisync-index` 查询缓存： 只读查询路径（M4-WP02）， tantivy
      内部 panic 是上游 bug， 不在我们可控面
    - 风险等级： 低（不可达 / 复合失败）， 但非零； 上游 lru 0.18.2+
      已修复， 待 tantivy 与 reed-solomon-erasure 跟进后撤销豁免
  - deny.toml 精确 diff：

    ```diff
    [advisories]
    ignore = [
        "RUSTSEC-2024-0384", # ADR-0014
        ...
    +   "RUSTSEC-2026-0253", # ADR-0024 amend（lru pop() UAF； 复合失败路径， tantivy/reed-solomon-erasure 跟进后撤销）
        "RUSTSEC-2023-0071", # ADR-0020
        ...
    ]
    ```

  本豁免按 ADR-0014/0020/0021 既有「可达性论证 + 撤销条件」 模式
  登记， 撤销条件 = tantivy ≥ 0.27 或 reed-solomon-erasure ≥ v7（携
  带 lru 0.18.2+） 任一发布。 本修订与 deny.toml license 白名单同
  属 T01 实施期发现， 故合并入 ADR-0024 而不开 ADR-0025。

**重新评估条件（详细）**：

- Tauri 2.x 发布 2.13 / 2.14 patch / minor： 仅 patch 直接 bump，
  minor 需评估 API 变更后新 ADR
- Tauri 3.0 stable 发布： 触发主版本迁移 ADR（API 不兼容已知）
- `deny.toml` license 白名单修改： 重审本 ADR「正面」节
- macOS WKWebView / Linux webkit2gtk 出现安全公告且无补丁： 评估
  替代 webview 抽象层
- Linux 发行版生态变化（webkit2gtk-4.1 → 4.2 迁移）： 评估 wry 升
  级路径
- 投资演示场景出现「必须用 Chromium 内核」的需求（如 webm codec）→
  重新评估 Tauri vs Electron

**修订登记**（同决策空间， 不另开 ADR）：

| 修订 | 触发任务 | 修订内容 | 关联 commit / SPEC 章节 |
|------|---------|---------|------------------------|
| 修订 1 | M6-WP03-T01 | deny.toml license 白名单新增 `Apache-2.0 WITH LLVM-exception`（target-lexicon 传递链） | commit `f0fe865` / SPEC §2 |
| 修订 2 | M6-WP03-T01 | deny.toml advisories ignore 新增 `RUSTSEC-2026-0253`（lru pop() UAF， 复合失败路径可达性论证） | commit `f0fe865` / SPEC §2 |
| 修订 3 | M6-WP03-T02 | ci.yml clippy + test + nightly.yml chaos 三处装 Tauri 2 Linux 6 系统包（ubuntu-latest runner 不默认带 webkit2gtk-4.1） | commit `c04e5b0` / SPEC §6 R2 |
| 修订 4 | M6-WP03-T03 | `crates/partisync-desktop/Cargo.toml` 新增 3 个 path-only workspace 内部依赖（`partisync-graph` / `partisync-cas` / `partisync-index`）以承载 6 个 IPC command 处理器。 **非顶层 crates.io 新依赖** ， 仅 workspace 内部 crate 接线； T01 起草期 ADR 后果节 diff 段已预留位置， 本修订落实之 | 即将 commit / SPEC §2.3 + §5 |
