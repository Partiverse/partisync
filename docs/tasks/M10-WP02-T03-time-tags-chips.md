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

- [ ] 静态探针：相对时间四档 + >30 天回落 + title 完整时间 + 缺失
      「—」不回退 + chips 派生/客户端过滤/meta 计数/全不选/换查询
      重置/样式 parity（沿 t04 五探针句式）；
- [ ] GUI 实操截图 `docs/screenshots/M10-WP02-T03-*.png`（chips 过滤
      前后 + 相对时间 tooltip 入镜）；
- [ ] fmt/clippy/test 绿；改动仅限本卡范围；零新增顶层依赖。
