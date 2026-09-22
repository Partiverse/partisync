# WP06 KPI 底稿（M4-WP06）

> 报告编号: BENCH-M4-WP06-001
> 关联: `docs/specs/M4-WP06.md`（G0 批准 2026-09-23）
> 任务范围: T01–T06 全部关账
> 数据日期: 2026-09-23（partisync-big.db-wal 反映 WP06 关账前的最近写入）
> CI 环境: Apple Clang 16 / Rust 1.94（workspace 钉版）+ NK_TARGET_*=0（numkong SIMD 绕过）

## §0. 执行摘要

| 维度 | 数值 | 备注 |
|---|---|---|
| 评估集规模 | 50 语料 + 25 查询 + 30 qrels | SPEC 计划 200/40，**偏离**披露见 §3 |
| BM25-only nDCG@10 | **0.754** | 合成库基线；真实 INBOX 待 M5+ |
| BM25-only Recall@10 | 0.780 | |
| BM25-only MRR | 0.747 | |
| 内部渗透探针 | 18 用例 / 18 pass / 3 项 P1 发现 | 见 `wp06-pen-test-internal.md` |
| 外部渗透 | RFC 已就绪（M4 关账前由用户委托 vendor） | `docs/security/pentest-rfc.md` |
| 文档产出 | SPEC + 威胁模型 + 攻击面 + RFC + 仲裁披露 + 内部报告 = 6 件 | |
| 代码产出 | eval 模块（metrics+runner+4 测）+ gateway 18 探针 + Bm25Index::reload | 零新顶层依赖 |
| Clippy | `cargo clippy -p partisync-index -p partisync-gateway --all-targets -D warnings` ✅ | |
| cargo deny | 待 T07 终跑（零新依赖→零新 deny ignore） | |

## §1. 评估集产物（SPEC §裁定 1）

### 1.1 语料（wp06_corpus）

| 维度 | 数值 |
|---|---|
| 文件数 | 50 markdown（d0001–d0050） |
| 总字数 | ~30k 字（avg ~600 字/篇） |
| 内容类型 | 技术笔记/日记/旅游/烹饪/读书/会议等 |
| 结构化字段 | frontmatter：title / filename / tags / updated_ns |
| 评估字段（runner 视角） | filename + tags（join space）+ ocr_text（= markdown 主体）+ updated_ns |

### 1.2 查询（wp06_queries.jsonl）

| 类型 | 数量 | 示例 |
|---|---|---|
| 短查询（1-2 词） | 3 | "夏威夷 度假" / "C2PA" / "夏威夷 度假" |
| 长查询（≥6 词） | 4 | "2024 年我去看樱花的地方 行程" |
| filename-only 命中 | 1 | "argon2_migration_notes" |
| tag-only 命中 | 1 | "kdf 密码学" |
| OCR 文本命中 | 多 | "川菜 家常菜 鸡丁" |
| 跨语种/同义词 | 1 | "travel itinerary California park" |
| 精确词（BM25 优势） | 多 | "tantivy BM25 多字段" / "usearch HNSW 索引 reserve" |
| 过滤型 | 2 | "2024 秋季 周报" / "预算 100平 装修 流程" |
| 完全无命中 | 1 | "区块链 以太坊 智能合约 部署"（档=1 弱相关） |

### 1.3 Ground Truth（wp06_qrels.jsonl）

| 维度 | 数值 |
|---|---|
| 总条数 | 30（4 档：relevance 0/1/2/3） |
| relevance=3（高度相关） | 23 |
| relevance=2（相关） | 4 |
| relevance=1（弱相关） | 3 |
| relevance=0（不相关） | 0（不计入 qrels） |
| 平均每查询相关文档 | 1.20 |
| **单人标注 + 自检两轮** | 标注者披露 + 边界用例登记见 `docs/qa/wp06-qrels-arbitration.md` |

## §2. 评估结果（wp06_bm25_eval_pipeline）

### 2.1 模式对比（三档计划 / 一档实跑）

| 模式 | Recall@10 | MRR | nDCG@10 | 状态 |
|---|---|---|---|---|
| **bm25_only** | **0.780** | **0.747** | **0.754** | ✅ 实跑（c40a895） |
| hybrid_no_rerank | — | — | — | ⏳ 留 P0：fake 向量作架构基线；embedding 模型到位后**重新跑** |
| hybrid_with_rerank | — | — | — | ⏳ 留 P0：feature `index-rerank` 默认关 + 真实 rerank 模型依赖未满足 |

### 2.2 基线含义

**Recall@10 = 0.780** 在 50 语料小规模 + 25 查询 + 单人标注 ground truth 下：
- BM25 多字段（filename + tags + body as OCR text）对中文短查询 + 英文 + 跨语种
  表现稳定
- top-10 中 78% 命中率（每查询平均 1.2 个相关文档）
- 首个相关文档平均在 rank 1.34（MRR=0.747）

### 2.3 不可作为生产承诺

按 §3 披露：单人标注 + 50/25 规模 + 合成库，**禁止**作为真实 INBOX 场景召回质量
承诺。生产档评估（真实 OCR + BGE-M3 向量 + 双盲标注 N=200）留 M5+。

## §3. 评估集偏离披露

| 偏离 | 计划 | 实际 | 原因 |
|---|---|---|---|
| 语料规模 | N=200 | N=50 | 手工精挑 > 大规模合成；真实 INBOX mime 全 null、OCR 全空无法分层采样 |
| 查询数量 | 40 | 25 | 同上；单人设计 25 比合成 40 更诚实 |
| Ground truth | 双人盲标 + 仲裁 | 单人标注 + 自检两轮 | 单 AI agent 无法独立完成双人流程；@lead 终审 T07 |
| 检索质量 | 实生产档 | 合成基线 | 真实 OCR/embedding 依赖未满足，登记 P0 |

详细见 `docs/qa/wp06-qrels-arbitration.md`。

## §4. 渗透探针（pen_test.rs）

| 组 | 用例 | pass | 行为发现 |
|---|---|---|---|
| 注入 | 8（SQL 注入 ×2 / 畸形 ×3 / Unicode / 空 / 类型错 / 嵌套过深 / 特殊字符） | 8 | 0 |
| 鉴权 | 4（跨库 ID / 跨库导出 / 组织越权 / 路径穿越） | 4 | 1 P1（organize_unknown_content） |
| DoS | 3（超大 limit / shard_size=0 / 100 次连续） | 3 | 1 P1（huge_batch） |
| C2PA 边界 | 3（Invalid / 缺失 manifest / 1 MiB detail） | 3 | 1 P1（absent_c2pa） |
| **合计** | **18** | **18** | **3 项 P1** |

详细见 `docs/reports/security/wp06-pen-test-internal.md`。

## §5. 端到端 KPI（执行方案 §6.5 M4 DoD）

| DoD | 状态 | 证据 |
|---|---|---|
| 10⁶ 条目语义问答 p95 <200ms | ⚠️ 待 M5+ 真实环境 | 合成库 50 文档不达 10⁶ 规模 |
| MCP Agent 端到端「归类→预览→提交→导出 manifest」 | ✅ M4-WP03 已闭环 | `6979b04` L2 DoD |
| 外部渗透测试无高危 | ⏳ M4 关账前委托 | RFC 已发（候选 4 家） |
| Sidecar 崩溃恢复 | ✅ M4-WP01 | 既有 |
| **评估报告** | **✅ 本文件 + pen-test-internal** | **本 WP06 交付** |

## §6. 涉及文件清单（T01–T06）

```
docs/specs/M4-WP06.md                                       # T01 SPEC (983298e)
docs/security/threat-model-mcp-gateway.md                    # T02 (611ee09)
docs/security/attack-surface.md                              # T02
docs/security/pentest-rfc.md                                # T02
docs/qa/wp06-qrels-arbitration.md                            # T03 (b8a1609)
crates/partisync-index/tests/fixtures/wp06_corpus.tsv        # T03
crates/partisync-index/tests/fixtures/wp06_*.jsonl           # T03
crates/partisync-index/tests/fixtures/wp06_corpus/*.md (×50) # T03
crates/partisync-index/src/eval/mod.rs                       # T04 (c40a895)
crates/partisync-index/src/eval/metrics.rs                   # T04
crates/partisync-index/src/eval/runner.rs                    # T04
crates/partisync-index/tests/wp06_eval.rs                    # T04
crates/partisync-index/src/search/bm25.rs (reload 添加)      # T04
crates/partisync-gateway/tests/pen_test.rs                   # T05 (9bf997f)
docs/reports/bench/m4-wp06-kpi.md (本文件)                  # T06
docs/reports/security/wp06-pen-test-internal.md              # T06
```

## §7. 后续行动项

| 优先级 | 项 | 来源 |
|---|---|---|
| P0 | M5+ 真实环境条件下评估（10⁶ 条目 + 真实 OCR/向量） | §5 DoD |
| P0 | embedding 模型到位后**重新跑** hybrid_no_rerank | §2.1 |
| P0 | reranker 模型到位后**重新跑** hybrid_with_rerank | §2.1 |
| P1 | 修复 asset_organize 批大小上限 | §4 / pen_test |
| P1 | 修复 asset_organize 跨库 content_id 校验 | §4 / pen_test |
| P1 | 修复 absent c2pa stage 返回形态 | §4 / pen_test |
| P1 | M4 关账前外部渗透 vendor 委托 | §4 / pentest RFC |
| P2 | 评估集 N=200 双盲扩 | §3 偏离披露 |