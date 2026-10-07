# Task: M10-WP04-T05 语义向量检索条件评估件（只评估不实施）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP04-T05 |
| **类型** | 评估件（纯文档；零代码零依赖变更） |
| **范围** | SPEC M10-WP04 §2.3 六问 + docs/reviews/M10-WP04-memory-vector-eval.md + 本卡 |
| **创建日期** | 2026-10-06 |
| **来源** | M9-WP02 §4 条件触发 + M10 拍板 #2「向量评估随行」+ M10-roadmap-proposal §7-2 |

## 交付物

`docs/reviews/M10-WP04-memory-vector-eval.md` 六问必答（契约全文见
SPEC §2.3）：① 既有件盘点（fastembed ai-embed + usearch VectorStore
+ hybrid，零新增依赖可走多远）；② memory 通道三方案对比；③ P20 承诺
边界；④ `memory_search` 公共面变更路径（pre-merge-safety R2 + ADR
触发条件）；⑤ 成本/性能口径；⑥ 拍板建议 + 实施触发条件。

证据须为本仓路径/行级引用（判例：M9-WP05-mcp2-remote-eval.md 文档
结构）。

## 验收

- [x] 六问齐备、行级证据可核；零代码 diff；
- [x] 提交挂 Task-ID `M10-WP04-T05`。
