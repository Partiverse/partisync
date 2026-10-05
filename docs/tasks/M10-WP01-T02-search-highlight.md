# Task: M10-WP01-T02 检索命中词高亮 + 有效摘要（引擎定位 + 前端 mark）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP01-T02 |
| **类型** | 功能补全（M10-WP00 §1-WP01；「有效摘要 + 命中词高亮」余款） |
| **范围** | crates/partisync-index/src/search/bm25.rs（+hybrid.rs 如需）+ desktop ui/app-core-v3.js + ui/styles-v3.css + tests/commands.rs + tests/ui_hardening.rs + 本卡 |
| **创建日期** | 2026-10-05 |
| **SPEC** | docs/specs/M10-WP01.md §2.2 / §3 |

## 根因

bm25 highlight 是「ocr/transcript 字段头部 200 字截断」（bm25.rs:376-394）
，与查询词零关系——命中词在文档后半段时摘要看不到命中依据；前端纯文本
渲染无高亮标记。

## 修复

1. partisync-index：用 tantivy `SnippetGenerator`（0.26 既有依赖，
   registry 源码已核实）生成含查询词的片段窗口，命中词以 sentinel
   `[[`/`]]` 包裹；失败/无词面回落现状（头部截断或 None），载荷形状
   `highlight: Option<String>` 不变。hybrid 经 rrf_fuse 自动继承。
2. 前端：snippet = `esc()` 后 sentinel→`<mark>` 替换；不变量——无未
   转义 `${h.highlight}` 插值。`<mark>` 样式入 styles-v3.css。

## 验收

- [ ] e2e `t02_highlight_centers_on_hit_terms`：命中词置 >200 字符偏移
      （现状实现必败的判别用例）+ 中文例 + 无词面回落例；
- [ ] 引擎级测试：窗口有界 + hybrid 继承断言；
- [ ] 静态探针：esc-后-替换接线 + 无未转义插值；
- [ ] GUI 实操截图 `docs/screenshots/M10-WP01-T02-*.png`（高亮入镜，
      filename 卡同框补 T01 截图欠账）；
- [ ] fmt/clippy/test 绿；零新增顶层依赖。
