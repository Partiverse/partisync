# Task: M10-WP02-T03 时间展示深化 + tag 过滤 chips

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP02-T03 |
| **类型** | 功能补全（M10-WP00 §1-WP02「时间展示」「筛选」项） |
| **范围** | desktop ui/app-core-v3.js + ui/index.html + ui/styles-v3.css + tests/ui_hardening.rs + 本卡（零 IPC / 零引擎 / 零 gateway） |
| **创建日期** | 2026-10-05 |
| **SPEC** | docs/specs/M10-WP02.md §2.3 / §3 |

## 根因

创建时间列只有绝对本地时间（timeFmt，app-core-v3.js:43-48），扫一眼
分不清「刚写入」和「上周」；tags 只能精确输入过滤（mem-tag 输入框），
看不到当前数据里有什么 tag 可选——与资产检索 T04 chips 相比缺同款
过滤面。

## 修复

1. 创建时间列改相对显示（「刚刚 / n 分钟前 / n 小时前 / n 天前」），
   >30 天回落绝对日期；title 悬浮 = 完整本地时间（timeFmt 输出不
   回退）；created_ns 缺失/0 → 既有「—」回落。
2. tag 过滤 chips：从当前已渲染行集派生 tag 集合（memTags() 解析），
   复用 `.chips/.chip` 既有样式（styles-v3.css:150-159）；点击仅
   客户端过滤已渲染行（不重发查询），meta 同步「显示 n / 共 m」；
   全不选 = 不过滤；新检索后重置重派生（T04 判例）。与 T01 排序
   正交可叠加。诚实边界：只反映 50 条窗口内的 tag（SPEC §2.3）。

## 验收

- [x] 静态探针：相对时间四档 + >30 天回落 + title 完整时间 + 缺失
      「—」不回退 + chips 派生/客户端过滤/meta 计数/全不选/换查询
      重置/样式 parity（沿 t04 五探针句式）——t03 探针×9（五探针 +
      memTime 阈值字面量/档序加固 + 行源单点组合锁 + T02 组合锁×2：
      行点击绑定在重渲管线内/展开态重渲重放，含裸插值禁列扩面），先
      红后绿 + memTime 探针与行点击绑定探针突变验证，T02（bfded23）
      合入 rebase 后 ui_hardening 合入基线 45/45 绿（2026-10-08 实测）；
- [ ] GUI 实操截图 `docs/screenshots/M10-WP02-T03-*.png`（chips 过滤
      前后 + meta「显示 n / 共 m」+ 相对时间 tooltip 入镜 + **四档边界
      全覆盖**：沿 T04 判例 `--data-dir` 隔离目录 CLI 播种 created_ns =
      now−30s（「刚刚」）/ −5min（「n 分钟前」）/ −2h（「n 小时前」）/
      −3d（「n 天前」）/ −40d（>30 天回落绝对日期）五条记忆，一帧截全
      四档 + 回落；执行时勿省边界档）——**环境受阻待验**（见遗留①），
      PR 保持 OPEN 挂「待 GUI 验证」label，不自行合入；
- [x] fmt/clippy/test 绿；改动仅限本卡范围；零新增顶层依赖。

## 遗留登记

1. **GUI 实操环境不可得（2026-10-06，二次重试仍未恢复）**：实操会话中
   窗口全部被窗口服务器挂到负坐标幻影屏（AppKit 仅报主屏 1440×900；
   CGWindowList 中 app 主窗退化 210×140@(-226,191)，与 M9-WP01-T04 登记
   的「210×141@负坐标环境异常」同款，本次幻影屏离线更甚）；System
   Events AX 对 partisd-desktop / 终端 / ZCode 全局 0 windows（AXPress
   驱动不可用）；`screencapture -x` 仅得主屏壁纸、`-l <主窗id>` 报
   could not create image。**app 本体无恙**：`--bench-cold-start` 探针
   `__BENCH_READY__ 293` 证实窗口+webview+page load 全链正常；评审同日
   二次实测（启动 → CGWindowList 主窗 47998 仍 @(-226,191) 退化几何、
   AX 对 Terminal 仍 0 窗口、`screencapture -l` 主窗仍失败、辅助窗
   48000 仍黑面）。另登记：Bash 沙箱内启动的 GUI 子进程不进窗口服务
   器（非沙箱启动才可见）——后续实操会话注意。**评审后第三轮重试（同
   日）仍未恢复**：启动实例 → 主窗 48008 仍 @(-226,191) 退化几何挂幻
   影屏、AX 仍 0 窗口；追加 cliclick CGEvent 拖拽营救（dd/dm 多中间点
   把标题栏从 (-121,201) 拖向主屏）窗口纹丝不动——幻影屏不接受合成
   事件，拖拽通道亦断。**二轮评审后第四轮重试（同日）一致**：主窗
   48013 仍 @(-226,191)、AX 仍 0（app 与 Terminal 同盲）、
   `screencapture -l` 主窗仍 could not create image——四轮证据一致，
   环境持续不可得。补验动作：环境恢复后
   按 §2.4 清单实操 + 截图归档 + SPEC §3 GUI 行回填。
   **第五轮重试（2026-10-08）环境部分恢复、根因收窄为 console 锁屏**：
   幻影屏消退（CGWindowList 主窗 331 正常几何 1200×800@(120,50)
   onscreen=true layer=0）、`screencapture -l 331` 通道已通（成功出
   2400×1600 图，标题栏「PartiSync · 资产图谱」清晰）——前四轮的截图
   通道断与 AX 全盲在本轮定位为锁屏衍生症状：`CGSessionCopyCurrent
   Dictionary` 实证 `CGSSessionScreenIsLocked = 1`（锁于 1791388188
   ≈ 2026-10-08 00:29，`CGSSessionSecureInputPID = 485` 即 loginwindow
   持安全输入）；锁屏下全屏截图仅锁屏壁纸（含「Touch ID or Enter
   Password」）、WKWebView 因 occlusion 从未绘制（窗口截图黑面）、
   AX 对用户会话窗口不可见。两实例（cargo run / 裸二进制重启 + 清
   savedState）一致；caffeinate -u 无法越过硬锁；解锁凭据（Touch ID/
   密码）在本会话不可得，如实登记不合入。**补验前置检查（经验沉淀）**：
   环境恢复后先查 `CGSSessionScreenIsLocked`，锁屏状态下窗口黑面/
   全屏壁纸/AX 失明均为衍生症状，勿再按幻影屏误诊。
2. **T01/T02 合入时的组合验证义务（评审 finding，medium）**：本 PR 基
   线无 T01（排序）/T02（详情展开），「过滤×排序」「过滤×详情展开」
   组合结果正确性当前不可验证。已落结构锁：探针
   `t03_mem_rows_source_through_filtered_single_point` 锁 renderMemRows
   行源唯一经 `filteredMemRows()`（行模板禁止直取 lastMemRows 原始快
   照——绕过滤即组合失效，合入即红）。**后续流程义务：T01/T02 合入时
   必须显式验证组合正确性**（排序管线落在 renderMemRows 内沿此单点叠
   加；详情展开行不得破坏过滤后行集与 meta 计数），以探针或 GUI 任一
   形式留档，否则 SPEC §2.3「正交可叠加」验收悬空。
   **【2026-10-08 清账】T02 已于 bfded23 合入 main，本 PR rebase 整合，
   「过滤×详情展开」组合义务到期即落**：memIndex 数据源随行集在
   loadMemories 单点同步（失败路径同步清空）、T02 行点击绑定接入
   renderMemRows 重渲管线（chip 过滤整表重建后行点击不丢）、详情展开
   态经 `openMemDetails` 集合重放（仍命中过滤行集者保持展开、被过滤掉
   者移出集合防死条目）；组合锁探针
   `t03_mem_row_click_binding_inside_rerender_pipeline` /
   `t03_mem_detail_state_restored_after_filter_rerender` 落地（行点击
   绑定突变验证红→还原绿，ui_hardening 合入基线 45/45 绿）。「过滤×
   排序」组合义务随 T01（PR #165）合入到期，届时沿本卡句式落锁。
3. **TDD 红态留痕不可复核（评审 finding，low，非缺陷指控）**：首 commit
   单 commit 无法事后复核「先红」。如实记录：红态验证发生于 2026-10-06
   实现会话——6 个 t03 探针先行全红（`function memTime(ns) {` 必须存在
   /`filteredMemRows` 必须存在 等 6 处 panic，17 既有探针同时全绿），
   实现后 23/23 绿；本修复 commit 追加第 24 个探针并对 memTime 阈值探
   针做**突变验证**（`s<3600` 临时写错为 `s<6000` → 探针红「memTime 缺
   档界 if (s < 3600)」→ 还原 24/24 绿），红绿两侧均有本会话可复算的
   命令输出佐证。
