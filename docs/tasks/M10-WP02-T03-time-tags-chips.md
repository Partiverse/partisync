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
      重置/样式 parity（沿 t04 五探针句式）——探针×6（含裸插值禁列
      扩面），先红后绿，ui_hardening 23/23 绿；
- [ ] GUI 实操截图 `docs/screenshots/M10-WP02-T03-*.png`（chips 过滤
      前后 + 相对时间 tooltip 入镜）——**环境受阻待验**（见遗留①），
      PR 保持 OPEN 挂「待 GUI 验证」label，不自行合入；
- [x] fmt/clippy/test 绿；改动仅限本卡范围；零新增顶层依赖。

## 遗留登记

1. **GUI 实操环境不可得（2026-10-06）**：实操会话中窗口全部被窗口
   服务器挂到负坐标幻影屏（AppKit 仅报主屏 1440×900；CGWindowList 中
   app 主窗退化 210×140@(-226,191)，与 M9-WP01-T04 登记的「210×141@
   负坐标环境异常」同款，本次幻影屏离线更甚）；System Events AX 对
   partisd-desktop / 终端 / ZCode 全局 0 windows（AXPress 驱动不可
   用）；`screencapture -x` 仅得主屏壁纸、`-l <主窗id>` 报 could not
   create image。**app 本体无恙**：`--bench-cold-start` 探针
   `__BENCH_READY__ 293` 证实窗口+webview+page load 全链正常；CGWindow
   List 确认窗口对象存在（onscreen=1）。另登记：Bash 沙箱内启动的
   GUI 子进程不进窗口服务器（非沙箱启动才可见）——后续实操会话注意。
   补验动作：环境恢复后按 §2.4 清单实操 + 截图归档 + SPEC §3 GUI 行
   回填。
