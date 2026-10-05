# Task: M10-WP02-T02 记忆详情联动展开 + 复制

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP02-T02 |
| **类型** | 功能补全（M10-WP00 §1-WP02「详情联动」「复制」项） |
| **范围** | desktop ui/app-core-v3.js + ui/index.html + ui/styles-v3.css + tests/ui_hardening.rs + 本卡（零 IPC / 零引擎 / 零 gateway） |
| **创建日期** | 2026-10-05 |
| **SPEC** | docs/specs/M10-WP02.md §2.2 / §3 |

## 根因

内容列一律 90 字截断（app-core-v3.js:625），metadata 字段返回不展示，
memory_id 无任何可见位置（只在 DOM data-* 属性）——内容全貌不可见、
id 不可复制（MCP 工具联调只能开 DevTools 抄 data 属性）。

## 修复

1. 点击记忆行（「验证」按钮 stopPropagation 隔离）展开
   `tr.mem-detail` 行（沿 .mem-proof 展开行判例，styles-v3.css:367）：
   完整 content、tags 完整、metadata pretty JSON（esc 后）、完整
   memory_id、created_ns 完整本地时间、score + 口径注记（FTS 匹配秩 /
   LIKE 恒 1.0，store.rs:2279-2282 语义诚实透出）；再点收起。
2. 详情行内「复制内容」「复制 ID」按钮 → `navigator.clipboard`
   `.writeText`（webview 内建 API，零新增依赖）；成功反馈按钮文案
   瞬变「已复制」≈1.5s 回落；剪贴板不可用回落方案实现期定（SPEC
   §6-R1），GUI 实操必测复制成功。
3. 硬约束：展开/收起不关闭既有证明行、不清空列表；证明行单开语义
   维持；详情模板动态段 esc() 全覆盖（无未转义插值）。

## 验收

- [ ] 静态探针：详情模板字段齐全（含 score 口径注记）+ esc 全覆盖 +
      展开不关证明行/不清列表 + 行点击与验证按钮事件隔离 + 复制经
      clipboard.writeText + 反馈回落；
- [ ] GUI 实操截图 `docs/screenshots/M10-WP02-T02-*.png`（详情展开 +
      复制粘贴到别处验证内容/ID 一致）；
- [ ] fmt/clippy/test 绿；改动仅限本卡范围；零新增顶层依赖。
