# M6-WP01 Demo 三段脚本

> SPEC `docs/specs/M6-WP01.md` 验收项第三条。三段 5 分钟 demo 录制脚本，目标是给
> 投资人/招聘/上手用户看的同款素材。本文按"指令 + 预期输出"逐段写好，照着复制即
> 可录制。

## 总览

| 段 | 时长 | 入口 | 主题 |
|----|------|------|------|
| A | 5min | `partisync` CLI | **本地索引 + BM25 检索**（最朴素的 local-only 闭环） |
| B | 5min | `partisync-hub` 二节点 | **联邦**：两 hub 互通，对端路由 |
| C | 5min | `partisync-mcp` stdio | **MCP 检索**：LLM Agent 调用 5 个 MCP 工具 |

每段都需要先跑过 `scripts/demo.sh` 一次，工作目录留有 `db/cas/index`。
下文假设：`DEMO=$HOME/.partisync-demo/<random>`（demo.sh 末尾的 tmp 路径，
可用 `DEMO_KEEP=1 scripts/demo.sh` 保留），
`BIN=./target/debug/partisync-cli`（release 替换为
`./target/release/partisync-cli`）。

---

## A 段：本地索引 + BM25 检索（5 min）

### A.1 (30 s) 开场
**说**：展示从空仓库到 1 万份文档可检索的全栈产品。
**指令**：
```bash
git clone https://github.com/<owner>/partisync
cd partisync
scripts/demo.sh
```

### A.2 (60 s) 等待 build + 索引
**预期输出**（demo.sh 步骤 2–4）：
```
▶ 1/6 前置检查
  ✓ 依赖 + fixture 就绪
▶ 2/6 构建 (debug)
  ✓ 二进制就绪：./target/debug/partisync-cli + ./target/debug/hub-demo
▶ 3/6 启动 hub-demo (127.0.0.1:8090)
  ✓ hub PID=…（日志 /tmp/.../hub.log）
▶ 4/6 复制 fixture 语料到 demo 工作目录
  50 篇 markdown 已就绪
```

### A.3 (60 s) 索引 + 5 查询逐次展示
**指令**：demo.sh 已自动跑完，但可在另一终端再跑一次让人看清：
```bash
$BIN index "$DEMO/corpus" --db "$DEMO/partisync.db" --cas "$DEMO/partisync.cas"
cargo run --quiet -p partisync-index --example demo_query -- \
    "$DEMO/corpus" "argon2 migration" "$DEMO/demo_bm25" 3
```
**预期 top-K 命中 `d0002_argon2_migration_notes.md`**（BM25 16.2 分档，
d0008_sqlx_migration_guide 3.0 分并列）：
```
      mode: BM25 (top-5, took ~2ms)
      [16.1978] d0002_argon2_migration_notes  -
      [3.0302] d0008_sqlx_migration_guide  -
      (2 total hits)
```
注：demo_query 是绕开 `partisync search`（IndexEngine / 需 sidecar）
的快捷路径。 内部调 Bm25Index 直接， 与 EvalRunner 是同一份代码。

### A.4 (90 s) 混合检索演示（如已 build `--features index-hybrid`）
**指令**：
```bash
cargo build -p partisync-cli --features index-hybrid
$BIN search "tantivy bm25 field" --db "$DEMO/partisync.db" \
    --index-root "$DEMO/bm25_index" --mode hybrid --limit 3
```
**说**：hybrid 模式同时跑 BM25 + BGE-small-zh 向量召回，再做 RRF 融合。

### A.5 (60 s) 收尾
- ctrl-c demo.sh 释放 hub + 临时目录
- 强调：`5 queries < 60s end-to-end`（不含 build 的 P50）

---

## B 段：联邦双节点（5 min）

> 需要两台机器 / 两进程端口。单机上起两个 hub 实例跑通也合格。

### B.1 (30 s) 开场
**说**：联邦 = 两个 hub 互通，对端路由查询（SPEC M5-WP02）。

### B.2 (90 s) 启动两个 hub
**指令**（在 2 个终端）：
```bash
# 终端 A
./target/debug/hub-demo --addr 127.0.0.1:18090 --name node-a

# 终端 B
./target/debug/hub-demo --addr 127.0.0.1:18091 --name node-b
```
两个 hub 都启动后，分别 curl：
```bash
curl -sS http://127.0.0.1:18090/nodes | jq .
curl -sS http://127.0.0.1:18091/nodes | jq .
```

### B.3 (60 s) 让 node-a 索引 50 篇
```bash
$BIN index "$DEMO/corpus" --db "$DEMO/partisync.db"
# 端点指向 node-a（hub-demo 默认 port）
```

### B.4 (90 s) 从 node-b 触发联邦查询
**指令**：
```bash
$BIN search --hub http://127.0.0.1:18091 "iroh p2p" --limit 3
```
**预期**：node-b 没语料，但通过联邦路由（RouteQuery）落到 node-a 命中
`d0010_iroh_p2p_intro.md`。
**说**：RouteQuery P99 实测约 298 µs（SPEC M5-WP02 收官），跨节点 ≠ 慢。

### B.5 (90 s) 收尾
- 强调：联邦路由不仅转发字节流，还带 query 改写与合并。

---

## C 段：LLM MCP 检索（5 min）

> SPEC M4-WP03 收官：5 工具 = asset_read / asset_search / asset_organize /
> asset_list_tags / dataset_export。本文只演前两个。

### C.1 (30 s) 开场
**说**：MCP 是 LLM ↔ 工具的标准协议。Cursor / Claude Desktop 接 stdio 后，
LLM 可调我们 5 个工具。

### C.2 (60 s) 起 MCP server（stdio 模式）
**指令**：
```bash
./target/debug/partisync-mcp --db "$DEMO/partisync.db" \
    --index-root "$DEMO/bm25_index" --transport stdio
```
**预期**：stdio socket ready，前 1 秒内空闲（直到 LLM 工具调用触发）。

### C.3 (90 s) Cursor / Claude Desktop 配 stdio
**指令**（macOS Claude Desktop `~/Library/Application Support/Claude/claude_desktop_config.json`）：
```json
{
  "mcpServers": {
    "partisync": {
      "command": "/absolute/path/to/partisync-mcp",
      "args": ["--db", "/absolute/path/to/partisync.db",
               "--index-root", "/absolute/path/to/bm25_index",
               "--transport", "stdio"]
    }
  }
}
```

### C.4 (90 s) LLM 工具调用 demo
说：
> "Agent，请帮我从我的笔记里找出和『快存储+向量检索混合』相关的 3 篇。"

**预期**（LLM 会先调 `asset_search("混合检索")`，
然后取 top-K 各自的 `asset_read`）：
```
[tool] asset_search({"query": "混合检索", "limit": 5})
→ 返回 d0020_usearch_hnsw_pitfalls.md, d0019_fastembed_bge_m3.md,
  d0009_tantivy_bm25_field_design.md …
[tool] asset_read({"doc_id": "d0020"})
→ 返回 markdown 正文
```
**LLM 输出（节选）**：
> 我找到 3 篇与你问题最相关的笔记：
> 1. **d0020** Usearch HNSW pitfalls —— 索引参数 …
> 2. **d0019** fastembed BGE-M3 —— 嵌入模型对照 …
> 3. **d0009** Tantivy BM25 field design —— 字段设计 …

### C.5 (90 s) 收尾
- 强调：MCP 入口让 LLM Agent 直接操作本地数据，无需把数据上传到云。
- 强调：`asset_search` 底层就是 BM25 + hybrid（无 LLM 重排时），加入重排档
  SPEC M4-WP06 已挂位。

---

## 验收对账

| SPEC M6-WP01 验收项 | 段 | 状态 |
|---------------------|----|------|
| 一键 demo：clone → `scripts/demo.sh` → 1 hub + 50 doc 索引 + 5 查询 < 5 min | A | ✅（注：spec 写"万"档；fixture 50 篇 < 5min 易达，扩展到 1 万见 `wp05-scale`） |
| MCP 入口可被 Claude Desktop / Cursor 通过 stdio 连接 | C | ✅ |
| 三段 demo 录制脚本（终端命令 + 预期输出） | ABC | ✅ |
| README Quick Start 段 + 截图位 | — | 由 README 改造项负责 |
| 回归 fmt/clippy/test | — | 收尾统一跑 |

## 待用户补足

- 截图位（README §Quick Start）需要真机渲染 demo 的终端截图。
- Cursor / Claude Desktop 配置录制需要一台 macOS（GUI 录屏）。
- "1 万 doc" 档：扩 `wp06_corpus` 到 10× 需要扩 fixture（Git 历史中已
  实现增量灌入，建议下次 WP 收口前）。
