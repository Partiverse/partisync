# M10-WP06-T02 rc/0.2.0 发布口径拍板文档

> Task-ID: M10-WP06-T02 · 日期: 2026-10-08 · SPEC: docs/specs/M10-WP06.md §2.1
> 执行: GLM-5.3-Flash (ZCode) · 判例母本: docs/specs/M9-WP06.md §2.1（轴 + 推荐案 + 依据）· 改进项来源: docs/reviews/M9-WP06-T03-release-report.md
>
> **⚠ 本档全部为推荐案，待用户确认**——确认（或修订）前不 tag、不建
> Release、不 publish、不实施 workspace bump（M9-WP06 §2.1 红线延续；
> M10-WP06 §4 非目标行）。用户修订口径时本档按修订版本登记（SPEC §6-R1）。

## 0. 用户确认位

- [x] 版本号轴（§1）：**确认**（2026-10-10，用户拍板「M10-WP06-T02 确认收官」——tag `v0.2.0-rc.1` + workspace 0.2.0 整数版，沿 §1 推荐案）
- [x] 产物面轴（§2，含改进项 a)–d) 处置）：**确认**（沿推荐案：a) strip 实施；b) reproducibility 联动评估随窗口实测；c) mcp-http 入包；d) macOS 维持）
- [x] 发布触发节奏轴（§3）：**确认**（沿推荐案；M11-WP01 窗口即日开启，§3.2 四步对单执行）

## 1. 版本号轴

### 1.1 推荐案

| 项 | 推荐 | 理由 / 依据 |
|---|---|---|
| tag | `v0.2.0-rc.1` | semver pre-release 标准位：`-rc.N` 可排序可迭代（rc.1 → rc.2 → … → GA `v0.2.0`）；沿 beta「tag 携 pre-release」判例（M9-WP06 §2.1：tag `v0.1.0-beta`） |
| Cargo workspace version | 0.1.0 → **0.2.0**（整数版本；pre-release 只在 tag 与产物名，不进 Cargo 版本） | ① roadmap §5 M10-WP05 行原案二选一「v0.1.0-rc / 0.2.0 增面版」——**选增面版**：M10 增量面（远程 MCP 透传 / 记忆二期 tombstone+GC / GUI 检索透出 / memory schema v16→v18）以 minor 位表达，0.x 阶段 minor 位即 breaking 位，semver 合规；② Cargo pre-release 版本（`0.2.0-rc.1`）同样不满足 path-dep `version = "0.1.0"` 约束（beta 判例同构，M9-WP06 §2.1），且 rc.1→rc.2 每轮迭代都得重写全图约束——**整数 0.2.0 使 rc 迭代轮次零 Cargo 面 diff**，49 处约束一次性原子切换；③ beta 维持 0.1.0 的理由（「改动面与收益不成比」）在 rc 失效：rc 语义 = 锁定 0.2.0 API 面接受验收，bump 正是 rc 的实质内容 |
| 与 beta 的实质差异 | **workspace 版本真 bump**（beta 判例 = Cargo 维持 0.1.0 + 产物名加后缀；rc = Cargo 真升 0.2.0 + tag/产物名 `-rc.1`） | beta 是 0.1.0 线的首个可分发标记；rc 是 0.2.0 线的验收候选——版本口径必须把两线分开，否则 GA `v0.2.0` 与 rc 产物同版本号无法区分 |

### 1.2 bump 改动面实测清单（**实测非估计**，2026-10-08 本会话实证）

实测命令与结果（在本仓库 main=`655a3a0` 上执行）：

```
$ grep -rn 'version = "0.1.0"' crates/*/Cargo.toml Cargo.toml | wc -l
      49
$ grep -rln 'version = "0.1.0"' crates/*/Cargo.toml Cargo.toml | wc -l
      14
```

**49 处 = 14 个文件（13 个成员 Cargo.toml + 根），细分如下：**

| 类别 | 处数 | 逐项 |
|---|---|---|
| path-dep 版本约束（`path = …, version = "0.1.0"`） | **46** | ai 3（:16/:17/:34）· cas 1（:9）· cli 7（:9–:14/:16）· desktop 6（:22–:25/:29/:34）· fuse 3（:21/:37/:38）· gateway 8（:12/:13/:14/:17/:20/:23/:27/:28）· graph 3（:9/:10/:11）· hub 5（:12/:13/:14/:15/:30）· index 3（:15/:16/:32）· provider 1（:9）· sync 3（:9/:10/:11）· transfer 3（:9/:10/:20） |
| 自身版本字段 | **3** | 根 `Cargo.toml:23`（`[workspace.package] version`——15/16 成员经 `version.workspace = true` 继承，partisd/core/ext-host/xtask 等零 path-dep 命中成员全靠此行）；`crates/partisync-fuse/Cargo.toml:3`（唯一自带 version 行的 workspace 成员）；`crates/partisync-fuse-spike/Cargo.toml:7`（**独立评估 workspace**，自带 Cargo.lock/deny.toml，不入产品依赖图，M7-WP03 判例——bump 是否随行留实施窗口裁定，默认随行保持字面一致） |

**bump 实施时的附加面（逐项）：**

| # | 面 | 实测位置 | 处置 |
|---|---|---|---|
| 1 | 桌面版本 | `crates/partisync-desktop/tauri.conf.json:4` `"version": "0.1.0",` | → `"0.2.0"`；desktop 无 package.json 版本面（实测 `ls` 无此文件），tauri.conf 单点即可 |
| 2 | 产物名模板 | `.github/workflows/release.yml` 6 处：`:40`/`:45`（linux tar 打包+upload）、`:66`/`:71`（macOS tar 打包+upload）、`:91`/`:96`（dmg cp+upload），命名 `…-0.1.0-beta-…` / `…_0.1.0-beta_…` | → `0.2.0-rc.1` 后缀（如 `partisync-cli-0.2.0-rc.1-linux-x86_64.tar.gz`、`PartiSync.Desktop_0.2.0-rc.1_aarch64.dmg`）；GA 时去 pre-release 段 |
| 3 | workflow 口径注记 | `release.yml:5–6` 头注释（beta 口径说明：tag/Cargo/产物名三要素） | 改写为 rc 口径注记（引用本档为依据） |
| 4 | Cargo.lock | 已入库（`git ls-files` 实测） | workspace 成员版本变更后随构建/`cargo update -w` 刷新，属 bump 附随 diff，随 bump PR 一并提交 |
| 5 | hub path-dep 语义核查（SPEC §6-R2） | hub 5 处约束（`:12/:13/:14/:15/:30`），`version = "0.1.0"` 语义 = `^0.1.0`（≥0.1.0, <0.2.0） | workspace 升 0.2.0 后任何一处不同步 = 依赖解析**硬错误**（build fail），不存在静默漂移——46 处必须与根版本行**同一 PR 原子切换**，全图 `cargo check` 通过为实施验收判据 |

> bump 实施本身**不在本 WP**（SPEC §4 非目标：随 rc 发布窗口独立 PR 执行）；
> 本节仅为该窗口提供逐项施工图与验收判据。

## 2. 产物面轴

### 2.1 推荐案

| 项 | 推荐 | 理由 / 依据 |
|---|---|---|
| 框架 | 沿 beta 41 assets 框架不变：linux tar（多 bin）/ macOS tar / desktop dmg（aarch64）/ SHA256SUMS + 各 `.minisig` / 22 个 CycloneDX `.cdx.json` / reproducibility.txt（M9-WP06-T03 报告 §1 发布事实） | 发布工程判例复跑（M8-WP02 → M9-WP06 两轮验证）；签名管线 ADR-0027 双钥 + ADR-0030 分域零改动 |
| tar 面 | linux tar 四 bin → **五 bin**（新增 `partisync-mcp-http`，见改进项 c)）；macOS tar 同步扩（cli+mcp → cli+mcp+mcp-http） | 远程 MCP 是 M10 主线叙事的最大增量面，实际入口不进产物面则叙事无载体（beta 判例：「实际入口即产物面」M9-WP06 §2.1）；两平台面保持一致便于用户对照 |
| 改进项处置 | 见 §2.2 逐项 | — |

### 2.2 rc 改进项逐项处置建议（a)–d) 非空）

| # | 改进项（来源） | 处置建议 | 依据 / 边界 |
|---|---|---|---|
| a) | **strip 打包步**（报告 §3-3：linux tar 276MB 未 strip，体积改进项非缺陷） | **建议实施**：release.yml 打包步嵌入 strip（linux runner 原生 `strip`，macOS 同），先 strip → 后 tar → 后 sha256/签名，B6 校验链顺序不变 | 实际体积降幅**留发布窗口实测登记，本档不预估**；已知代价 = 分发物无符号影响用户侧 backtrace 可读性，rc 验收期可接受；实施以 tar 体积前后对照写入发布报告 |
| b) | **reproducibility 确定性构建**（报告 §2-B7 REPRODUCIBLE=no 清偿目标） | **建议评估后择一**：优先试 `CARGO_PROFILE_RELEASE_DEBUG=false`（去 debuginfo——路径相关性主源）；与 a) **联动评估**（strip/去 debuginfo 可能直接消解路径相关性主源，一次实验同时裁决两项）；评估不达标则 rc 期维持 REPRODUCIBLE=no + reproducibility.txt 如实登记（**不假绿**），GA 前清偿 | M8-WP02 R1 判例：偏差登记不假绿；评估结论（含复现命令）随 rc 发布报告落档 |
| c) | **`partisync-mcp-http` 入包**（M10-WP05 新交付，`crates/partisync-gateway/src/bin/partisync-mcp-http.rs` 实测在位） | **建议入包**（linux + macOS 两 tar 各扩一 bin）；授权面 = P23 fail-closed 墙语义不变（M10-WP05-T04 探针矩阵 13 例）；**已知限制随附**：NB-WP05-1（真实外接 AS 端到端未验，当前验证边界 = mock AS，ADR-0031 / M10-WP05-T06 关账卡）随 Release notes 声明 | 不入包则 M10 远程化主线在发布面无载体；fail-closed 默认态保证未配置即拒绝，入包不扩攻击面（M10-WP05 §2 启动守卫语义） |
| d) | **macOS fuse 偏差 / hub demo 面债**（报告 §3-1 / §3-4） | **维持**：macOS 产物不含 `partisync-fuse`（osxfuse 系统件 runner 必败，linux tar 全量对照）；hub 产物 = `hub-demo-web` demo 面如实标注（hub 正式 bin 未立，债延续） | 两项均为既登记偏差非缺陷，rc 期无新事实，处置不变；Release notes 已知限制节沿 beta 措辞 |

## 3. 发布触发节奏轴

### 3.1 推荐案

| 项 | 推荐 | 理由 / 依据 |
|---|---|---|
| rc 发布前置 | ① **本 WP 收口**（T02–T05 全关账）；② **用户对本档三轴口径确认（或修订后确认）** | M9-WP06 §2.1 红线：确认前不 tag、不建 Release、不 publish；本 WP 本身不执行发布动作（SPEC §1/§4） |
| bump 实施窗口 | rc 发布窗口**首个 PR**（独立实施 PR：§1.2 清单 49 处 + 附加面原子切换 + `cargo check` 全图验收），随后才是 tag/Release | SPEC §4 非目标行；bump 与发布动作分离保持每 PR 单一职责 |
| 已知限制清单 | 随 Release notes 更新，沿 beta 判例（**手写正文优先** + `generate_release_notes` 附录，CHANGELOG.md 为详版源——报告 §2-B8 口径）：① NB-WP05-1 远程 MCP 真实外接 AS 端到端未验（mock AS 边界）；② 桌面记忆编辑/删除 UI 条件触发（M10-WP00 §2-NB2 / §3-T03 滚动登记，GUI 立项为承接位）；③ 语义向量检索暂不实施（M10-WP04-T05 评估拍板「条件不成熟」，PR #180）；④ CAS 存量无清单数据 read_errors 维持（M10-WP00 §2-NB5「存量不可回填」，M10-WP04 §4）；⑤ 沿袭 beta 三项：桌面 dmg 未公证 / hub demo 面 / macOS 无 fuse | 五项均为已登记条件触发债，如实随附不假绿；CHANGELOG.md 增 `[0.2.0-rc.1]` 节随发布窗口起草（「AI 起草人终审」判例 M8-WP02 §2.4） |

### 3.2 发布序列（确认后执行，供 rc 窗口对单）

1. bump 实施 PR（§1.2 施工图）→ 三门禁 + `cargo check` 全图绿；
2. `docs/release/CHANGELOG.md` rc 节起草 + release.yml 修订（产物名/注记/strip/入包 bin）；
3. 用户终审口径 → tag `v0.2.0-rc.1` push → draft Release；
4. checklist 核验（沿 RELEASE-CHECKLIST-beta.md B1–B8 复跑）→ 用户终审 publish。

## 4. 非目标（本档不执行的动作）

tag/Release/publish、workspace bump 实施、strip/reproducibility 评估实验、
mcp-http 入包 workflow 修订——全部留 rc 发布窗口（SPEC §4；本档只拍板
与清单化）。

## 5. 修订记录

| 版本 | 日期 | 内容 |
|---|---|---|
| 1.0 | 2026-10-08 | 初版：三轴推荐案 + 改进项 a)–d) 处置 + bump 改动面实测清单（49 处 / 14 文件 / 附加面 5 项）。待用户确认 |
