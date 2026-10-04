# Task: M9-WP03-T01 UI 硬化——escapeHtml 统一 + 详情空库测试 + 转写开关接线

> **范围外**：记忆浏览面板随 T02；旗舰记忆通道随 T03；GUI 三态截图随 T04。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP03-T01 |
| **类型** | 实装（桌面 UI + IPC 参数透传；R1 全审——24 站点回归面大） |
| **优先级** | P0（SPEC M9-WP03 三主线之一；D2/D3/D4 债清偿） |
| **范围** | SPEC M9-WP03 §2.1 + §3 前三条验收 + §5 清单（ipc.rs/commands.rs/ui_hardening.rs/app-core-v3.js）+ **偏差补记：`crates/partisync-index/src/search/bm25.rs`**（见下）+ 本卡 |
| **创建日期** | 2026-10-04 |
| **来源** | SPEC M9-WP03 批准（PR #134，main=`5e2be3c`，§6-R1 拍板 = 开关化）+ M8-WP05-ui-report D2/D3/D4 |

## R5 落锤（SPEC §6-R5，本卡记录）

JS 单测基建：**Rust 侧静态契约探针**（`tests/ui_hardening.rs`，
`include_str!` 读 `ui/app-core-v3.js` 源码做规则断言，CI 常绿）+
T04 全 tab GUI 巡检截图兜底。不引 Node 测试链（零新增依赖）。

## 交付物

1. **esc() 统一**（app-core-v3.js）：helper 转义 `& < > " '` 五字符；
   24 处 innerHTML 站点中全部动态插值收敛（清单见下）；hex-only 字段
   （content_id）同样包 esc（后端库被篡改时仍是安全文本，一致性口径）。
2. **asset_detail 空库形状测试**：空库查询不存在 content_id →
   `Ok` 且 copies 空.vec + size 0，不 panic 不缺 key。
3. **include_transcript 开关接线**（§6-R1 开关化）：
   - ipc.rs `SearchArgs` 增 `include_transcript: Option<bool>`
     （None = 后端常开现状，`unwrap_or(true)`）；`search` /
     `search_hybrid` 透传给引擎；
   - UI「含转写文本」radio 显式传 `include_transcript: true`，
     关键词/语义 radio 不传（维持常开语义）；modeLabel 增「含转写」。

## innerHTML 站点清单（24 处 → T01 收敛面）

| 行 | 容器 | 动态插值 | 处置 |
|---|---|---|---|
| 11 | document.body | 静态串 | 无需（非插值） |
| 95 | #crumbs | e.name / e.path(attr) | esc |
| 103 | #rows | e.name / e.path / e.content_id / data-name attr | esc |
| 111 | #rows | 静态 + curPath 比较 | 无插值 |
| 125 | detail-panel | 静态 | — |
| 131 | detail-panel | e?.kind | esc |
| 137 | detail-panel | name / c.path / contentId | esc |
| 172-176 | #srows / meta | 静态骨架 | — |
| 177-184 | meta | modeLabel（常量）| — |
| 187 | #srows | h.content_id / h.highlight / h.score | esc（highlight = OCR/转写纯文本片段，bm25.rs:368 实证无 markup） |
| 198 | #srows | q（用户输入！） | esc |
| 204-208 | #srows | kind | esc |
| 215 | #view-dups | g.content_id / c.path | esc |
| 228 | #view-jobs | r.id / r.kind / r.status_name / r.checkpoint | esc |
| 247 | #ext-list | t.name / capabilities | esc |
| 253 | #ext-list | 静态 | — |
| 309/312 | #ext-history | h.tool / h.input / h.output（用户输入） | esc |
| 378/384 | sync-banner | stats.devices / total（number） | esc（一致性） |
| 393 | timeline | 静态 | — |
| 412 | timeline | it.name / it.origin_device / it.dir / it.content_id | esc |
| 415 | timeline | kind | esc |

## 实施期发现（偏差登记）

1. **SPEC §2.1「索引引擎透传已支持」假设不成立**：`Bm25Query::
   include_transcript` 在 bm25 路径从未被 `Bm25Index::search()` 尊重
   （QueryParser 恒含 tx 字段）——参数名存实亡。**补齐**：Bm25Index 增
   `parser_no_tx`（无转写字段解析器），`include_transcript=false` 走
   之。这是对既有公开参数语义的补全（原注释即声明「关闭可提升速度」），
   hybrid 路径不受影响。涉及文件偏差：bm25.rs（上表已补记）。
2. **D3 台账过期**：M8-WP05-ui-report D3「详情面板空库测试」实际已被
   `t02_asset_detail_empty_db_returns_empty_copies` 覆盖（验证后清账，
   未新增重复测试）——沿「先验证再动手」判例。
3. **GUI 验收阻塞（环境异常）**：本机（darwin 25.6.0，单显示区
   2560×1600）构建后的桌面壳窗口退化 210×141 @ 负坐标
   （`window-state.json` 记录 1200×800 未被应用；`open`/nohup 两种
   启动方式同现；System Events AX 窗口数 = 0；CUA 捕获拒绝）。main
   同样复现——非本任务代码引入。**PR 保持 OPEN 标注「待 GUI 验证」**
   （AGENTS.md 桌面端硬性规则），T04 GUI 三态验收前置排查：疑似
   window_state::apply 失效或 tauri 窗口初始化时序，需人工点验/环境
   复测。

## 验收

- [ ] 静态契约探针（ui_hardening.rs）：esc() 定义唯一且含五字符转义；
      禁止裸插值清单（`${e.name}` / `${it.name}` / `${c.path}` /
      `${e.path}` / `${h.tool}` / `${it.origin_device}` / `${it.dir}` /
      `${name}` 等）在源码零命中；
- [ ] asset_detail 空库形状测试绿；
- [ ] include_transcript 接线测试：种子 Bm25Index 直写 transcript 文档
      （`engine.bm25_index().upsert(IndexedDoc)`），transcript 查询
      None（默认 true）命中 / Some(false) 不命中；
- [ ] fmt/clippy/test 三件套全绿；既有 19 commands 测试零回归；
- [ ] 提交挂 Task-ID `M9-WP03-T01`。
