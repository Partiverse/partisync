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

- [x] 静态探针：chips 派生归一逻辑 + 选中过滤仅影响展示 + meta
      「显示 n / 共 m」接线断言——`t04_filter_chips_derive_from_filename_normalized`
      / `t04_chip_toggle_filters_client_side_only` / `t04_filter_applies_to_rendered_hits_only`
      / `t04_meta_shown_over_total_and_state_reset` / `t04_filter_chips_dom_and_style_parity`
      / `t04_search_form_never_reloads_page`（ui_hardening 15/15 绿）；
- [x] GUI 实操截图 `docs/screenshots/M10-WP01-T04-*.png`（过滤前后
      两帧 + md/txt 叠选大小写归一帧，共 3 帧）；
- [x] fmt/clippy/test 绿；改动仅限本卡范围。

## 落地实况（2026-10-05，GUI 实操发现两项并当场上锁）

1. **检索 form 整页重载（既有 bug，本卡修复）**：CSP `script-src
   'self'` 必拦 inline `onsubmit="return false"`（形同虚设），点 [检索]
   / Enter 均触发 form 默认提交 = 整页刷回 browse tab、检索结果全丢，
   chips 面在 GUI 不可演示。最小修复（清单内文件）：按钮 `type=submit`
   → `type=button`、摘除死 inline onsubmit、JS 侧 Enter keydown
   preventDefault + form submit 兜底 preventDefault；回归锁
   `t04_search_form_never_reloads_page`。**记忆面板同名 form 模式为既
   有遗留未动**（不在本卡清单），后续任务承接。
2. **chips 点击接线遗漏（实现期自查）**：首版 renderChips 漏绑
   `onclick → toggleExt`（GUI 实测点击无效果，hover 态伪装选中）；
   补接线 + `t04_filter_chips_dom_and_style_parity` 增接线断言锁死。
3. GUI 验证路径：`--data-dir /tmp/m10-t04-gui` 隔离数据目录（5 文件
   语料：txt/md/json/大写 TXT），CLI index+reindex 播种，BM25 检索
   `quantum` 4 hits——chips 派生 json/md/txt（大小写归一实证）、点
   md → 「资产 显示 1 / 共 4」仅 report.md、叠 txt → 3/4 含大写
   .TXT；全程无重载（query 保留）。
