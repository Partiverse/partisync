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

- [x] e2e `t02_highlight_centers_on_hit_terms`：命中词置 >200 字符偏移
      （现状实现必败的判别用例）+ 中文例 + 无词面回落例；
- [x] 引擎级测试：窗口有界 + hybrid 继承断言；
- [x] 静态探针：esc-后-替换接线 + 无未转义插值；
- [ ] GUI 实操截图 `docs/screenshots/M10-WP01-T02-*.png`（高亮入镜，
      filename 卡同框补 T01 截图欠账）；【待 GUI 验证】桌面实例启动
      成功但本会话锁屏无法操作（无密码不可解锁）——PR 保持 OPEN
      不合入，解锁后实操补录
- [x] fmt/clippy/test 绿；零新增顶层依赖。

## 落地实况（2026-10-05）

- 实作发现 tantivy 0.26 `TEXT` 常量 `stored: false`——旧「头部 200 字
  截断」运行时恒 None（stored 值不存在，SPEC §1 描述与运行时行为有
  出入）；修复补 `ocr_text`/`transcript_text` STORED（`schema()`，
  bm25.rs）+ `SnippetGenerator` 定位摘要。
- 存量索引按目录内嵌旧 schema 打开：打开/检索不受影响，highlight 回
  落 None，重建（`partisync reindex`）后摘要生效。
- 后端自拼 sentinel（fragment + highlighted ranges + collapse），不用
  `Snippet::to_html()`（其内部转义会与前端 esc 叠成双重转义）。
- 中文 fragment 碎片化实测（超 §6-R1 预判）：stored = fan-out 文本 →
  fragment 为「单字+bigram 空格交错」碎片流非可读原文；命中定位/
  sentinel 契约不受影响，处置接受；设计债（stored 膨胀 ~2x、原文摘要
  需 stored-only 字段+reindex）登记 SPEC §2.2 注记④ + §4 非目标表。
- 回落路径顺手修正旧 `&s[..200]` 按字节截断的 UTF-8 中界 panic 隐患
  （改按字符边界，`head_truncate`）。
