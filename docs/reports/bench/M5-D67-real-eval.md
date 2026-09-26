# M6-D67 真实评估报告

> SPEC [docs/specs/M6-D67.md](../../specs/M6-D67.md) v0.1 落档（2026-09-26）。
> 当前状态：**评估管线实装完成（BM25 档）；LCSTS 真档数字 BLOCKED-on-用户数据**。

## TL;DR

- ✅ `scripts/prepare-lcsts.py` 抽出器：JSON/JSONL → corpus.tsv + queries.jsonl + qrels.jsonl（mock 数据集 end-to-end 实测通过）
- ✅ `crates/partisync-index/examples/eval_real.rs` 直接调 `EvalRunner::run_bm25_only`， 写 `eval.json`
- ✅ `scripts/eval-real.sh` 一键驱动； 自动退化到 wp06 fixture（无需网络）
- ⚠️ **LCSTS 真档未端到端跑通**： 本会话探查 hf-mirror.com / modelscope.cn 路径， 发现
  - HF 镜像（`hf-mirror.com`）API 端点返回 401/404（数据集路径需要登录态或 endpoints 路径变动）
  - `https://www.modelscope.cn/datasets/rabittail/LCSTS` 返回 200 但文件列表 API 在匿名调用下报「参数错误」10020000400
  - **故数据集下载需用户在登录态浏览器手动准备**
- ⚠️ **hybrid_no_rerank / hybrid_with_rerank 模式尚未实装**： EvalRunner 当前只暴露 `run_bm25_only`； fastembed embeddings（feature `index-embed`）已 Cargo.toml 标注但未串联 EvalRunner → 第 3 档数字缺失。 待 fastembed 接 BM25+bge-small-zh-v1.5 后扩展 EvalRunner。

## 验收对账（SPEC M6-D67 §验收）

| 项 | 状态 | 证据 |
|----|------|------|
| 一键脚本 `scripts/eval-real.sh` 拉模型 + LCSTS 子集 → 跑三档 → 输出 eval.json + 简表 | 🟡 部分 | BM25 档 ✓； LCSTS 子集抽取 ✓； 三档模式（hybrid 未实装， 占位） |
| `docs/reports/bench/M5-D67-real-eval.md` 落档（Recall@10/MRR/nDCG@10） | ✅ | 本文档 |
| D6 仅记录 fastembed CPU 端到端冒烟（口径诚实标注） | ✅ | 见 §D6 |
| 回归：fmt/clippy/test 全绿 | 🟡 | 待 `cargo fmt/clippy/test` 确认（本次会话 commit 之前统一跑） |

## 实测档

### 退化档（wp06 fixture）：BM25-only

本会话以 `scripts/eval-real.sh` 端到端跑通（wp06 fixture 退化）。
**实测数字（2026-09-26）**：

| 指标 | 实测 | wp06 测试阈值 | 余量 |
|------|------|--------------|------|
| Recall@10 | 0.7800 | ≥ 0.30 | 2.6× |
| MRR | 0.7467 | ≥ 0.30 | 2.5× |
| nDCG@10 | 0.7537 | ≥ 0.30 | 2.5× |
| Num queries | 25 | — | — |
| 数据规模 | 50 doc / 25 query / 32 qrels | — | — |

注：这是**合成 fixture 上的 BM25-only 真档**， 不是 LCSTS 真档。
数字偏高的原因是 wp06 的查询=一句摘要 / ground-truth=对应正文，
词汇重叠很高（规格说明里的「构造库」基线， 非产品指标）。

**对比 SPEC M4-WP06 wp06_eval 测试断言**： 全数通过且远超阈值，
证明 EvalRunner 通过 `examples/eval_real.rs` 入口与原 `wp06_eval.rs` 测试走的是同一 KPI 路径（功能上等价的两个 driver）。

### 真档档（LCSTS，待用户提供数据）

**用户侧准备**：
1. 浏览器登录 https://huggingface.co/datasets/<某镜像>/lcsts 或 modelscope， 下载
   LCSTS `data/test.jsonl`（每行 `{"summary": ..., "content": ...}` 形式）
2. 将文件放本地， 记绝对路径
3. 跑：
```bash
EVAL_INPUT=/abs/path/to/lcsts.jsonl \
  EVAL_DOCS=200 EVAL_QUERIES=40 \
  EVAL_OUT=docs/reports/bench/eval-lcsts-bm25.json \
  scripts/eval-real.sh
```
4. 把 `eval-lcsts-bm25.json` 数值填入下表 §「真档档数字」

**预期指标量级**（LCSTS + BM25 中文短文搜）：
- Recall@10: 0.40–0.65（LCSTS 总结句对原文 short-text 检索， BM25 已较强）
- MRR: 0.35–0.55
- nDCG@10: 0.45–0.60

实际数填入后， 将替换下表占位行。

#### 真档档数字（LCSTS 待补）

| 数据集 | 模式 | Recall@10 | MRR | nDCG@10 | 备注 |
|--------|------|-----------|-----|---------|------|
| wp06 fixture 50/25 | bm25_only | **0.7800** | **0.7467** | **0.7537** | 本会话实测，作为本档基线 |
| LCSTS 200doc/40query | bm25_only | BLOCKED | BLOCKED | BLOCKED | 待用户侧准备数据，跑 `EVAL_INPUT=… scripts/eval-real.sh` 填入 |
| (待 fastembed 接) | hybrid_no_rerank | — | — | — | EvalRunner 未实装此模式 |
| (待 bge-reranker 接) | hybrid_with_rerank | — | — | — | EvalRunner 未实装此模式 |

### D6 三推理栈冒烟

**已就绪面**： `partisync-ai` 已有 OCR / embed / transcribe 三 stage（WP01 收官）
feature 门控（`ai-ocr / ai-embed / ai-transcribe`）， fastembed
`TextInitOptions::with_cache_dir(PathBuf)` 支持本地缓存预下载。

**网络可达性（实测 2026-09-26）**：
- `https://hf-mirror.com` HTTP/2 200（站点可达）
- `https://hf-mirror.com/BAAI/bge-small-zh-v1.5/resolve/main/config.json` HTTP/2 307 redirect（模型权重可达， fastembed 默认走这里）
- LCSTS 数据集路径 401/404（**未端到端打通**）

**冒烟口径（口径诚实）**：
- ✅ embed：BGE-small-zh-v1.5 模型权重可达（307→实际下载）， 接 fastembed
  `with_cache_dir` 即可脱网运行。 本会话未触发实际下载/索引管线（依赖数据集）。
- ⚠️ OCR：feature 路径实装存在， 资源受限（无 OCR 语料 + 模型 600MB+） 跳过冒烟。
- ⚠️ transcribe： 同 OCR 跳过（需音频语料， Whisper 模型 1.5GB+）。

### 后续待办

| 项 | 触发条件 |
|----|----------|
| EvalRunner 加 `run_hybrid_no_rerank` | fastembed 实装 + tantivy BM25+bge hybrid 拼接（SPEC M4-WP02 既有框架可复用， feature `index-embed` 已开）|
| EvalRunner 加 `run_hybrid_with_rerank` | bge-reranker / jina-rerank feature gate（`index-rerank` 已开 dep `fastembed`） |
| LCSTS 真档真数字 | 用户侧提供 LCSTS JSON 后跑 `EVAL_INPUT=… scripts/eval-real.sh` |
| D6 embed 真冒烟 | LCSTS 子集 ready 后跑： fastembed 离线 → usearch HNSW build → top-10 P50/P99 + recall |
| wp06 fixture 扩到 1 万 doc | 单 fixture 单测扩规模； CI 阈值同步更新（需 ADR） |

## 链接

- SPEC：[docs/specs/M6-D67.md](../../specs/M6-D67.md)
- 抽出器：[`scripts/prepare-lcsts.py`](../../../scripts/prepare-lcsts.py)
- 驱动器：[`scripts/eval-real.sh`](../../../scripts/eval-real.sh)
- EvalRunner binary：[`crates/partisync-index/examples/eval_real.rs`](../../../crates/partisync-index/examples/eval_real.rs)
- EvalRunner 库：[`crates/partisync-index/src/eval/runner.rs`](../../../crates/partisync-index/src/eval/runner.rs)
- 指标框架：[`crates/partisync-index/src/eval/metrics.rs`](../../../crates/partisync-index/src/eval/metrics.rs)
- wp06 baseline 测试：[`crates/partisync-index/tests/wp06_eval.rs`](../../../crates/partisync-index/tests/wp06_eval.rs)
- ADR-0018（fastembed gating）：[`docs/adr/0018-fastembed-rerank-gating.md`](../../adr/0018-fastembed-rerank-gating.md)
