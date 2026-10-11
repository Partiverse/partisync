# Task: M11-WP06-T06 — 预算实测 / 真模型探针 / e2e 语义收益演示

> SPEC: docs/specs/M11-WP06.md §2.6/§3 · ADR: docs/adr/ADR-0035 · 
> 证据件: docs/reviews/M11-WP06-t6-eval-evidence.txt（全链实录）
> 执行: 2026-10-11 @ /tmp/wp-m11-wp06（m11/wp06-t05-fix 基线 97d4cb7）

## 验收项（SPEC §3 对应）

- [x] **下载腿复测（§6-3 实施首日义务）**：HF_ENDPOINT=hf-mirror 直连
  → `HuggingFace API error: Header Content-Range is missing` 复现
  （EXIT=101，证据件 ②）；改走 HTTPS_PROXY=127.0.0.1:7897 代理下载
  Xenova/bge-small-zh-v1.5 成功（证据件 ③，snapshot 75c43b06…2781 与钉版一致）。
- [x] **逐文件 blake3 核对（§2.3 本地验收）**：五文件 blake3 + 字节数与
  models.rs `BGE_SMALL_ZH_V15_FILES` 逐字一致（证据件 ④），零放宽。
- [x] **维度契约真模型探针（§3）**：新增 graph-embed 门控单测
  `real_model_probe::dimension_contract_12_sentences_all_512`
  （memory_vector.rs），12 句（≥10）全 512 维 PASS（证据件 ③）。
- [x] **全量嵌入时长预算（§2.6/§3）**：118076 叶回填 1450s = 24m10s
  ≤ 1800s 达线；对账 sweep 15ms 重嵌 0；failed 0（证据件 ⑤）。
- [x] **usearch artifact 体积预算（§2.6/§3）**：259,347,120 B =
  1.015× 运行时主文件 / 1.112× 预算锚 233,230,336 B，均 ≤1.5× 达线
  （主文件伴表增长双口径并报，证据件 ⑤/⑧-6）。
- [x] **MCP 契约探针（§3）**：tools/list memory_search schema 透出
  `mode: enum[bm25,hybrid]`（HTTP 200，证据件 ⑥-1）。
- [x] **partisd loopback hybrid 冒烟（§3）**：127.0.0.1:7650/mcp
  mode=hybrid 正控命中（⑥-5）；关闭态对照 mode=hybrid → BM25 结果 +
  `degraded: vector_unavailable`（⑥-7，fail-open 显式降级）。
- [x] **e2e 语义收益演示（§3）**：「超级富豪家庭把子女送出国读书的平均岁数」
  BM25 total=0（负控）vs hybrid total=5（目标叶 rank 2），正控字面子串
  total=2；第二组储蓄银行改写 rank 1 命中（⑥-2/③/④/⑥-6）。
- [x] **本地门禁**：cargo fmt --check / clippy -D warnings / cargo test
  --workspace 全绿（证据件 ⑦）。

## 诚实边界（详见证据件 ⑧）

单机口径；模型经代理下载（hf-mirror 协议不兼容复现）；位级确定性未证
（SPEC §6-7）；语义收益为单库抽样演示（人工改写查询对，非真实流量/
无 golden 集）；hybrid 延迟为 curl 墙钟口径。

## 产物清单（Iron Rule 9）

- crates/partisync-graph/src/memory_vector.rs（+graph-embed 门控探针）
- docs/reviews/M11-WP06-t6-eval-evidence.txt（新）
- docs/tasks/M11-WP06-T06-eval.md（本卡）
