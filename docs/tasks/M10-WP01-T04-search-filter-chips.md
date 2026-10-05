# Task: M10-WP01-T04 检索过滤面（结果集扩展名 chips，客户端）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP01-T04 |
| **类型** | 功能补全（M10-WP00 §1-WP01「UX 深化」之二） |
| **范围** | desktop ui/app-core-v3.js + ui/index.html + ui/styles-v3.css + tests/ui_hardening.rs + 本卡（零 IPC / 零引擎改动） |
| **创建日期** | 2026-10-05 |
| **SPEC** | docs/specs/M10-WP01.md §2.4 / §3 |

## 根因

检索 tab 除三态 radio + 含记忆外没有任何筛选维度；命中集大时无收敛
手段。诚实边界：只做 filename 可派生的维度——生产索引 tags 恒空
（reindex.rs:146）、mime 未入索引，对空维度做过滤面 = 「形同虚设」
二次犯（SPEC §4 登记后续承接）。

## 修复

1. 结果渲染后从命中集 `filename` 派生扩展名集合（大小写归一；无扩展
   名/孤儿行归「(无)」），渲染 chips 行；
2. 点击 chip 切换选中态，客户端过滤**已渲染命中**（不重发查询、不触
   后端）；meta 行同步「显示 n / 共 m」；全不选 = 不过滤。

## 验收

- [ ] 静态探针：chips 派生归一逻辑 + 选中过滤仅影响展示 + meta
      「显示 n / 共 m」接线断言；
- [ ] GUI 实操截图 `docs/screenshots/M10-WP01-T04-*.png`（过滤前后
      两帧）；
- [ ] fmt/clippy/test 绿；改动仅限本卡范围。
