# Task: M11-WP05-T04 T1 语料积累(LCSTS ≥10⁵ 叶导入 + FTS 低召回实证)

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP05-T04 |
| **类型** | dev-target 代码 + docs(WP05 首个含 dev-target 代码的任务:examples 不入产品依赖图、零 Cargo.toml 变更、零产品 API 面) |
| **来源** | memory-vector-eval §6 触发纪律 + T1 硬条件(memory 叶量 ≥10⁵ 且 FTS 通道零召回/低召回有实证记录,M11-WP05-T03 立项评估判定为实施暂缓主因) |
| **创建日期** | 2026-10-10 |
| **数据集** | hugcyp/LCSTS(hf-mirror.com 直连,train.jsonl 2400591 行固化于仓外 ~/Data/lcsts/) |
| **导入面** | partisync-graph example 批量直插(tests/wp02_memory.rs perf 探针同口径:hlc=NULL、oplog 零行、一次 refresh_memory_root),非 memory_write 逐笔 |
| **检索面** | partisd MCP Streamable HTTP(127.0.0.1:7650/mcp,真实 MCP 面,未降级) |

## 交付

crates/partisync-graph/examples/import_lcsts.rs(新,LCSTS JSONL → memory 域批量导入器,
身份/列级哈希与产品写路径同源,INSERT OR IGNORE 幂等)+ 实测:120000 尝试 → 实插
118076(语料内 1924 条重复幂等去重),root=b84dace1…27575,verify ok=true,118076 叶
重算 802ms;幂等重跑实插 0 且同根。docs/reviews/M11-WP05-t1-corpus-evidence.txt(新,
全链证据):正控「立法法」total=14 管道存活;100 条 LCSTS summary 语义缺口探针 0 命中
98%(98/100),耗时中位 3.5ms;LIKE 兜底旁证(2 字 query 全表扫 0.181s ≈ FTS 中位 ~51 倍,
且实测兜底 pattern 无 % 通配=零子串召回)。T1 判定对照:叶量 118076 ≥10⁵ ✓ + FTS 低召回
实证在案 ✓——判定请用户复核。

## 涉及文件清单(Iron Rule 9)

crates/partisync-graph/examples/import_lcsts.rs(新)·
docs/tasks/M11-WP05-T04-t1-corpus.md(本卡)·
docs/reviews/M11-WP05-t1-corpus-evidence.txt(新)

## 验收

- [x] LCSTS 真实语料 ≥200000 行已下载固化(实测 2400591 行,仓外 ~/Data/lcsts/)
- [x] memory 叶量 ≥10⁵(count(*) = 118076,verify_memory ok=true)
- [x] 正控 ≥1 命中(实测 total=14)
- [x] 100 条语义探针 0 命中比例记录在案(98/100,逐条 total/score/耗时全录)
- [x] 走真实 MCP 面(未降级 Store API example)
- [x] LIKE 兜底旁证在案(0.181s 全表扫 + 精确匹配语义发现)
- [x] 零新依赖、零既有文件修改(examples 仅用 partisync-graph 既有依赖)
- [x] cargo fmt --all --check 通过
- [x] cargo clippy -p partisync-graph --all-targets -- -D warnings 通过
- [x] cargo test -p partisync-graph 通过(27 passed / 0 failed)
- [ ] T1 判定经用户复核
