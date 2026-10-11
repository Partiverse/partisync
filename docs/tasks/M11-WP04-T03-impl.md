# Task: M11-WP04-T03 检索深化实现（stored 原文域 + 反 fan-out 摘要 + 中文高亮修复）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP04-T03 |
| **SPEC** | docs/specs/M11-WP04.md（已批准；本卡实现 §2.1 schema/写入、§2.2 摘要回落链、§3 验收） |
| **类型** | dev-target 代码（R1 全审面：search highlight 载荷语义；零新依赖/零公共 API 签名变更/零 graph schema 迁移/零 unsafe） |
| **创建日期** | 2026-10-10 |

## 交付

bm25.rs 单面：①schema 增 ocr_text_orig/transcript_text_orig，stored-only 形态 = `add_text_field(name, STORED)`（tantivy 0.26.2 text_options.rs:307-316 `From<StoredFlag>`=indexing:None+stored:true，零 postings/零 fieldnorm，不进 QueryParser）；②upsert/upsert_batch 同源双写，orig 域按索引内嵌 schema 解析 `Option<Field>`（get_field_entry 裸 Vec 下标 schema.rs:281-283，旧 6 字段 schema 写新序号 panic → 缺域即整体跳过 orig，存量索引打开/检索/写入不受影响）；③search 摘要四级回落链：①原文定位（sanitize+cjk_fan_out token 集字面子串、Latin 大小写不敏感、char 窗口≤200、复用 [[/]] sentinel 与 collapse_overlapped_ranges）→②扇出串定位（存量索引触达）→③head_truncate→④None，逐字段回落、①②不混搭。测试形态与覆盖论断：t02 中文用例按 §3 改写（可读原文+[[夸克]]，改写理由随 PR 正文）；旧 6 字段 schema 内嵌索引 fixture（②③回落+写侧守卫不 panic）；检索等价 = 结构断言（orig 不在 parser 注册面）+ 固定 fixture 快照对照（main 版临时 harness 采集 4 文档×5 查询命中集/分数逐项一致；orig 不进 parser/不参与评分，BM25 分数确定性，容差 1e-6）。

## 涉及文件清单（Iron Rule 9）

crates/partisync-index/src/search/bm25.rs（唯一产品代码面，hybrid.rs 零 diff 实证）· docs/tasks/M11-WP04-T03-impl.md（本卡）

## 验收

- [x] 三门禁绿（cargo fmt --all --check / cargo clippy --workspace --all-targets -- -D warnings / cargo test --workspace）——PR #229 CI 全绿（合入 2026-10-11T03:36Z）；本地复跑 2026-10-11 fmt/clippy 绿 + workspace test 绿（期间唯一红灯 `partisync-desktop::mcp_call_real_sidecar_ext_list` 为本机环境态：无签名 demo 扩展触发 [P21] verify-mandatory 拒载，恢复设计跳过态后 25/25 绿，根因与处置详见审查报告 §1，与 T03 diff 零代码交集）
- [x] t02 中文摘要改写（可读原文、bigram [[..]] 包裹、无「字 间空格」碎片流；仅此项按 SPEC §3 改写，其余用例零断言放宽）——审查核实改写为 SPEC §3 唯一授权项且断言实质收紧（`[[夸克]]` → `发现了新的[[夸克]]` + 碎片流负断言）
- [x] 旧 schema fixture 回落链测试（②③与现状一致、全链不 panic）——`t03_legacy_schema_six_fields_fallback`（读侧②③ + 写侧降级不 panic + 检索不受影响）；跨字段优先级维度缺口见审查报告 §5-建议 1（已回执登记，见 PR 正文）
- [x] 检索等价测试（结构钉+快照钉，形态与覆盖论断见交付段）——`t03_search_equivalence_with_orig_fields`：parser 注册面负向结构钉（tantivy query_parser.rs:541-543 源码核实）+ 4 文档×5 查询快照钉逐项 assert（容差 1e-6）；provenance 缺口回执见 PR 正文
- [x] 零新依赖、零 unsafe、IndexedDoc/Bm25Index/IndexEngine 公共签名不变——Cargo.toml/lock/deny.toml/rust-toolchain.toml 零 diff；diff 零 unsafe；公共面仅私有字段增量
- [x] T04 实测报告（存储域 ≤+35% / reindex <5% / CLI 高亮对照）为独立任务 M11-WP04-T04，不在本卡——T04 已独立收官（#232–#234，SPEC v0.2 预算裁决）

> 勾闭依据：AI 对抗审查 R1 全审报告 docs/reviews/M11-WP04-t03-review.md（2026-10-11，阻断 0 / 建议 3 / 风格 3，结论「可勾闭」；建议条回执随勾闭 PR 正文），合入即人工终审面。
