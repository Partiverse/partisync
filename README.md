# PartiSync

> 个人级联邦检索：把全设备/全家庭的文档、日志、照片 OCR、音频转写、邮件
> 附件统一编入本地索引，跨设备联邦查询；不上云，不复制原文。
>
> **状态**：M5 全收官（WP01–WP09 + ADR-0021 云事件 SDK）。M6（产品
> 化演示 + 真实评估）落地中。详细进度 [M4-report §5](docs/M4-report.md) /
> [M5 KPI](docs/reports/bench/m5-wp05-scale.md)。

---

## 5 分钟上手

### 1. 一键 demo

```bash
git clone https://github.com/<owner>/partisync
cd partisync
scripts/demo.sh
```

`scripts/demo.sh` 会完成：
1. build `partisync-cli` + `partisync-hub`（首次 2–4 min，二次秒开）
2. 启动本地 hub 演示面（http://127.0.0.1:8090）
3. 把 50 篇真实 markdown 笔记（仓库内置的 wp06 fixture）灌进 BM25 索引
4. 跑 5 个示例查询，列出 top-5 命中

退出后查 transcript 留在 `$DEMO_DIR`（DEMO_KEEP=1 保留）。

调优：
- `DEMO_RELEASE=1 scripts/demo.sh` —— release 构建（更慢但运行时更快）
- `DEMO_PORT=9000 scripts/demo.sh` —— 自定义端口
- `DEMO_QUERIES="arg 2pa usearch" scripts/demo.sh` —— 自定义查询

### 2. 三段 demo 录制脚本

录制时逐字照搬：[`scripts/demo-narratives.md`](scripts/demo-narratives.md)

| 段 | 时长 | 入口 | 主题 |
|----|------|------|------|
| A | 5min | CLI | 本地索引 + BM25 检索 |
| B | 5min | hub-demo | 联邦双节点（对端路由） |
| C | 5min | MCP server | LLM Agent 通过 stdio 调 5 工具 |

### 3. MCP 接 Claude Desktop / Cursor

把仓库 `partisync-mcp` binary 加到 MCP server 列表（stdio 模式）：
```json
{
  "mcpServers": {
    "partisync": {
      "command": "/abs/path/to/partisync-mcp",
      "args": ["--db", "/abs/path/to/partisync.db",
               "--index-root", "/abs/path/to/bm25_index",
               "--transport", "stdio"]
    }
  }
}
```
详见 [scripts/demo-narratives.md §C](scripts/demo-narratives.md) 与
[MCP SPEC](docs/specs/M4-WP03.md)。

---

## 真实评估（M6-D67）

LCSTS 子集抽 + IR 指标一把跑：
```bash
scripts/eval-real.sh                                   # 退化为 wp06 fixture
EVAL_INPUT=/path/to/lcsts.json scripts/eval-real.sh    # LCSTS 真档
```

输出 `eval.json`（含 Recall@K / MRR / nDCG@K 三档 × 每查询明细）。
评估框架 `partisync_index::EvalRunner`，规格见
[docs/specs/M6-D67.md](docs/specs/M6-D67.md)，报告见
[docs/reports/bench/M5-D67-real-eval.md](docs/reports/bench/M5-D67-real-eval.md)。

---

## 工作流

### 仓库骨架

```
crates/
  partisync-cli/           # CLI 入口（14 子命令：index/ui/watch/.../search/scan-plan/event-drain）
  partisync-hub/           # 联邦 + 持久化 + 演示面
  partisync-mcp/           # MCP stdio server
  partisync-ai/            # OCR / embed / transcribe 推理栈 (feature-gated)
  partisync-index/         # tantivy BM25 + usearch 向量 + eval runner
  partisync-sync/          # iroh 联邦接入 + SQS/Kafka 事件流 + 扫描调度
  partisync-graph/         # Merkle 同步 + graph apply
  partisync-cas/           # 内容寻址存储 (blake3 chunks)
  partisync-provider/      # fs/s3/webdav/s3api
  partisync-transfer/      # iroh transfer + chunk adaptors
  partisync-core/          # 基础类型（被所有依赖）
  xtask/                   # 开发工具（trace/report/benchmark）
```

### CLI 子命令速查

```bash
partisync index <root> --db ./p.db --cas ./p.cas       # 索引入库
partisync search <q> --db ./p.db --index-root ./idx \
    --mode bm25|hybrid --limit N                         # 检索
partisync scan-plan --scheme fs --root <dir> --concurrency N  # 扫描调度 dry-run
partisync event-drain --source mock --journal <path>    # 云事件流增量
partisync watch <root>                                    # 增量 watch
partisync ui --addr 127.0.0.1:8080                        # 网页演示面
```

### 常用命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p partisync-cli

# 性能测量（必设）
NK_TARGET_SME=0 NK_TARGET_SME2=0 NK_TARGET_SMEF64=0 NK_TARGET_NEON=1 \
  cargo bench --workspace --bench wp05
```

详细仓库治理 / 提交格式 / 风险分级见 [AGENTS.md](AGENTS.md)。

---

## 路线图

| 里程碑 | 状态 | 关键交付 |
|--------|------|----------|
| M0 调研 | ✅ | proto 清理、Go prototype 冲突发现 |
| M1 基础 | ✅ | 高层目录布局、基础类型 |
| M2 闭环 | ✅ | bisync + Merkle + 密码学审计 Conditional Pass |
| M3 数据面 | ✅ | pack v2、EC、分层存储、GC 设计 |
| M4-WP01 资产 | ✅ | 元数据查询 + 标签 + 导出 |
| M4-WP02 混合检索 | ✅ | BM25 + 向量 + RRF（tantivy/usearch/fastembed） |
| M4-WP03 MCP | ✅ | 5 工具闭环，GLM Agent 自主调用 |
| M4-WP05 内容感知 | ✅ | C2PA 防护 + OCR/Whisper 实装 |
| M5-WP01–WP09 联邦 | ✅ | iroh 接入、路由、扫描调度、事件流、规模、混沌、性能、图谱 |
| **M6 产品化 + 真实评估** | 🟡 in progress | demo 包（`scripts/demo.sh`）+ LCSTS 真档（`scripts/eval-real.sh`） |

完整报告见 [`docs/M4-report.md`](docs/M4-report.md) /
[`docs/reports/bench/`](docs/reports/bench/)。

---

## 贡献与开发

提交格式（AGENTS.md §铁律 2）：

```
<type>(<scope>): <subject> [M{m}-WP{nn}-T{nn}]

Task-ID: M{m}-WP{nn}-T{nn}
Spec: docs/specs/M{m}-WP{nn}.md
AI-Assist: <agent/model>
AI-Review: <agent/model>
Reviewed-By: <human-id>
```

禁止事项（红线）见 AGENTS.md —— 包括禁止新增顶层依赖（须 ADR + cargo deny 通过）、
禁止放宽断言、禁止改动任务清单外的文件等。
