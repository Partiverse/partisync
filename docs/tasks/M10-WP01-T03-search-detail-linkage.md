# Task: M10-WP01-T03 检索结果-详情联动（命中卡点击 → asset_detail）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP01-T03 |
| **类型** | 功能补全（M10-WP00 §1-WP01「UX 深化」之一） |
| **范围** | desktop ui/app-core-v3.js + ui/index.html + ui/styles-v3.css + tests/ui_hardening.rs + 本卡（零 IPC 改动） |
| **创建日期** | 2026-10-05 |
| **SPEC** | docs/specs/M10-WP01.md §2.3 / §3 |

## 根因

命中卡是静态 article（app-core-v3.js:215-228），不可点击——检索结果与
既有详情面板（`asset_detail` IPC，M8-WP05-T02 `showDetail` 渲染）之间
没有联动，用户从检索命中的下一步动作断裂。

## 修复

1. 命中卡整卡可点击（cursor:pointer + 可访问语义），点击调**既有**
   `asset_detail({prefix: h.content_id})`，在检索视图内呈现详情
   （大小/副本数/指纹/副本路径）；面板形态（浮层/内联）实现期定，
   **硬约束：点击不得清空结果列表**，关闭后结果原样可继续点击；
2. 详情标题用 `h.filename`（缺失回落 content_id 切片）。

## 验收

- [x] 静态探针：命中卡 click → showDetail(h.content_id, h.filename)
      接线断言 + 结果列表保留断言（`t03_hit_card_click_wires_show_detail_and_keeps_results`）；
- [x] GUI 实操截图 `docs/screenshots/M10-WP01-T03-*.png`（点开一张
      命中卡的详情面板，结果列表同框在镜）——2026-10-05 补验通过：
      真实实例（--data-dir 隔离语料，检索 `test` 6 hits）实测命中卡
      点击 → 详情面板（filename 标题/大小/副本数/指纹色/副本路径/
      [×] 关闭）+ 结果列表 6 卡原样保留；[×] 关闭后结果原样可继续
      点击；快速连点两卡并发守卫生效（面板最终显示末次点击卡）。
      证据：`M10-WP01-T03-search-detail-linkage.png`（详情+结果同框）、
      `M10-WP01-T03-detail-closed-results-kept.png`（关闭后结果保留）。
      前史：首轮验证遇会话无人值守 + Stage Manager 窗口塌缩（210×141@
      负坐标）不可操控，曾按规则挂「待 GUI 验证」；会话恢复后补验解除。
- [x] fmt/clippy/test 绿（`mcp_call_real_sidecar_ext_list` 为 main 既有
      环境依赖失败，stash 基线同败，与本卡零接触面）；改动仅限本卡范围。
