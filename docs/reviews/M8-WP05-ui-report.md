# M8-WP05 桌面壳功能面一期 UI 报告（T01–T04 收尾）

> Task-ID: M8-WP05-T04 · 日期: 2026-10-02 · SPEC: docs/specs/M8-WP05.md
> （批准 PR #93）· 设计: docs/design/M8-WP05-design.md（v4.3）·
> 功能对照基准: docs/reviews/M8-WP05-function-map.md · 执行: GLM-5.3-Flash
> (ZCode)

## 1. 交付总览（SPEC §2 对照）

| 任务 | 交付 | PR | GUI 证据 |
|---|---|---|---|
| T1 语义检索旗舰 | `search_hybrid` IPC（fastembed BGE-small-zh-v1.5 独立懒加载，启动路径零模型加载）+ 检索 tab 关键词/语义/含转写三态开关 + 结果统一渲染（`mode` 标注）+ 加载/空/错三态 | #95 | loading / keyword-empty / error-fallback 三态截图（§3） |
| T2 浏览详情 + 转写检索 | `asset_detail` IPC + 浏览行点击详情面板（内容身份 blake3 色带 + 副本路径）+ `include_transcript` 透传（N4：后端常开，开关为语义标注，零后端增量） | #96 | detail 截图（§3） |
| T3 同步状态界面 | partisync-sync 只读依赖（N2：无写路径/网络面）+ `sync_stats`/`sync_recent` IPC（sync_oplog/sync_conflict 只读派生）+ 同步 tab（状态横幅 + 四格 data-count 仪表 + 按日时间线 + 三态） | #97/#98 | sync-empty / sync-data 双截图 + 5s 轮询实证（§3） |
| T4 扩展面板增强 + 本报告 | styles v4.3 tokens（T01 随旗舰重写交付：星点底/扫描线/全直角/全 mono/`fpOf` 指纹派色）+ ext 面板 manifest capabilities 展示（T01 起已在）+ **会话内调用历史（最近 20 次入参/出参，纯前端态，行点击回看）** | 本 PR | ⚠️ 待 GUI 验证（§5） |

## 2. GUI 实操记录（AGENTS.md 硬规则流程）

- **工具链判例**：cliclick（点击/鼠标）+ osascript frontmost（按 **unix id** 定位实例——多实例重名时按名匹配会撞错进程）+ `screencapture -x` 全屏 + `sips -c 1600 2400 --cropOffset 100 240` 裁 1200×800 逻辑窗（2x retina）。
- **数据态种子判例**（T3 首创）：CLI 无 capture 写路径——对**运行中**实例的 WAL DB 直接 `sqlite3 INSERT` oplog/conflict 种子行，前端 5s 轮询自动刷新，一张截图同时实证渲染 + IPC 读链路 + 轮询（H3）。
- **视觉查证**：截图经 visual-judge 子代理对照设计 v4.3 规范独立评审（T3 双图 PASS：布局对齐 wireframe、全直角/全 mono、琥珀仅冲突语义、空态动作邀请文案）。
- **T4 全 tab 巡检（已补验，2026-10-02 晚）**：初轮因显示器锁定改走「待 GUI 验证」（PR 正文签收动作）；用户解锁后按签收动作补齐——7 张截图归档 + demo_echo 真调用（AXPress 路径）出参回显 + 调用历史数据态；visual-judge 对照 v4.3 评审 7 张（首轮 4 pass + 3 fail：browse/dups/jobs 空态缺动作邀请——即修复验，见 §6）。

## 3. 截图索引（docs/screenshots/）

| 文件 | 任务 | 内容 |
|---|---|---|
| M8-WP05-design-search.png / -browse.png / -sync.png | T00 | 设计稿三页原型（1280 headless） |
| M8-WP05-design-states.png | T00 | 组件状态画廊（Skeleton/Empty/Button/Toast/Dialog） |
| M8-WP05-T1-loading.png | T1 | 语义检索 loading 态（嵌入模型首次加载） |
| M8-WP05-T1-keyword-empty.png | T1 | 关键词检索空态 |
| M8-WP05-T1-error-fallback.png | T1 | 语义检索错误态（降级提示切回关键词） |
| M8-WP05-T2-detail.png | T2 | 浏览详情面板（blake3 色带 + 副本路径） |
| M8-WP05-T3-sync-empty.png | T3 | 同步 tab 空态（动作邀请） |
| M8-WP05-T3-sync-data.png | T3 | 同步 tab 数据态（琥珀横幅 + 3/1/0/1 仪表 + 时间线，轮询实证） |
| M8-WP05-T4-tab-browse.png | T4 | 浏览 tab（根路径空态 + 动作邀请） |
| M8-WP05-T4-tab-search.png | T4 | 检索 tab（关键词模式 + hero 框 + 三开关） |
| M8-WP05-T4-tab-sync.png | T4 | 同步 tab 空态（T03 交付复核） |
| M8-WP05-T4-tab-dups.png | T4 | 重复内容 tab（空态 + 动作邀请） |
| M8-WP05-T4-tab-jobs.png | T4 | 作业 tab（空态 + 动作邀请） |
| M8-WP05-T4-tab-ext.png | T4 | 扩展 tab（工具表 + capabilities 展示 + 调用历史空态） |
| M8-WP05-T4-ext-history.png | T4 | 扩展调用历史数据态（demo_echo 真调用：出参 JSON + 历史行 23:04:23 OK in（空）· out） |

## 4. function-map 功能覆盖矩阵核对（§1 逻辑树）

| 节点 | 状态 | 落点 |
|---|---|---|
| A1 统计仪表（get_stats/cas_stats） | ✅（既有） | 常驻 tally |
| B1 目录浏览（list） | ✅（既有） | browse tab |
| B2 条目详情（asset_detail，T2） | ✅ | 详情面板 |
| C1 BM25（search） | ✅（既有） | search tab |
| C2 语义混合（search_hybrid，T1 旗舰） | ✅ | search tab 语义开关 |
| C3 转写检索开关（N4 透传） | ✅ | search tab 含转写开关 |
| D1 去重组（duplicates） | ✅（既有） | dups tab |
| E1 作业列表（jobs） | ✅（既有） | jobs tab |
| F1 同步统计（sync_stats，T3） | ✅ | 同步 tab 四格仪表 |
| F2 最近活动（sync_recent，T3） | ✅ | 同步 tab 按日时间线 |
| F3 作业列表联动复用 | ✅（既有 jobs tab 即载体） | — |
| G1 工具列表 + 调用沙盒（ext_list/mcp_call） | ✅（既有） | ext tab |
| G2 能力面展示 + 调用历史（T4） | ✅ / ⚠️ | capabilities 列已在；调用历史已实现，**待 GUI 验证** |
| H1 {kind,msg} 错误条 | ✅ | `call()` 包装 + #error-region 5s 自清 |
| H2 窗口状态持久化 | ✅（M6-WP03-T06） | window-state.json |
| H3 5s 轮询（stats + 当前 tab） | ✅（T3 同步 tab 已纳入） | setInterval |

## 5. 债与排期（本期登记）

| # | 债 | 来源 | 处置 |
|---|---|---|---|
| D1 | oplog ACK 排水后 sync_stats 归零，与「已应用」累计标签口径冲突——engine 接线时 devices/last_sync 改由 `sync_watermark`（持久、不被 trim）派生 | T3 AI 审查 F1（P2） | 引擎写路径接线任务（独立，对账语义先行）落地时一并处理 |
| D2 | ~~时间线/历史等 innerHTML 插值未转义（沿 v3 判例，CSP `script-src 'self'` 已挡 inline 事件，残留 markup 破格风险）——统一 `escapeHtml` UI 硬化~~ **已清账（2026-10-04）**：24 站点动态插值收敛 `esc()` helper（M9-WP03-T01，PR #135；ui_hardening.rs 三探针绿——esc 定义唯一含五字符 / 裸插值禁列零命中 / 收敛规模对账）；全 tab 巡检截图挂 T04 人工 | T3 AI 审查 F2（P3） | 已闭环（巡检挂 T04） |
| D3 | ~~详情面板测试空库路径覆盖（T02 登记延续）~~ **已清账（2026-10-04）**：空库路径已有 `t02_asset_detail_empty_db_returns_empty_copies` 覆盖（commands.rs:346）——M9-WP03-T01 验证后确认既有，未新增重复测试（沿「先验证再动手」判例） | T2 | 已闭环 |
| D4 | ~~转写检索开关为 N4 纯透传语义标注（include_transcript 后端常开）~~ **已清账（2026-10-04）**：IPC `include_transcript: Option<bool>` 显式接线 + UI「含转写」开关显式传值 + bm25 `parser_no_tx` 补齐（false 真实生效）——M9-WP03-T01（PR #135；`t01_search_include_transcript_toggle_wiring` + 静态探针绿） | T2 | 已闭环 |
| D5 | ~~Mimosa `scanner_enobufs` 反复出现，deep 完整审计重跑~~ **已清账（2026-10-02）**：deep 复扫 `scan-2026-10-02T15-25-41.420Z-0a39454d7cbc`（seal `sha256:6163689e…`）全程无 enobufs，唯一 HIGH advisory 与 M6-report §5.1 已签收误报同源（M8-WP00 台账行已更新复扫记录） | hook 提示 ×3+ | 已闭环（登记不构成安全放行结论） |
| D6 | ~~T4 全 tab 视觉巡检待 GUI 验证~~ **已清账（2026-10-02 晚）**：7 张截图归档 + demo_echo 真调用 + visual-judge 评审；随带修复 browse/dups/jobs 空态动作邀请缺口（judge 首轮 3 fail → 整改） | AGENTS.md 验收规则 | 已闭环 |

## 6. 复核日志

- 2026-10-02：GLM-5.3-Flash 初稿（T01–T03 证据回溯 + T4 交付 + 债表 D1–D6）。
- 2026-10-02 晚：D6 清账——锁屏解除后按 PR 签收动作补全 GUI 巡检；visual-judge 首轮 7 图评审（4 pass / 3 fail），3 fail = browse/dups/jobs 空态缺动作邀请（T01 重写遗留缺口，设计 §4 明文）→ 当场修复空态文案 + 重截 + 复评。GUI 工具链新增判例：WKWebView 对 cliclick 合成点击在部分静态按钮上不派发 onclick（hover 态正常）——改走 **macOS Accessibility AXPress**（computer-use）可靠触发；setValue 会遭智能引号替换（尾引号变 ”），绕法 = 空参调用或含数字值 JSON。
