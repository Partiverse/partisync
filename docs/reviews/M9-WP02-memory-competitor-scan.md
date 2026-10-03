# 竞品摸底：mem0 / Letta / MCP 生态记忆工具面（M9-WP02 SPEC 起草输入）

日期: 2026-10-03 · 任务: M9-WP02-T01 · 方法: 官方文档站 + 官方仓库抽样核查（web 调研），
逐项注明来源；查不到官方记载的项目显式标注「未查到」，不作推断性补齐（带 ※ 的条目为
按 REST API schema 还原，MCP 层 schema 本身官方未在文档页公开）。

## 1. mem0

形态：云端托管 MCP server（`https://mcp.mem0.ai/mcp`，HTTP transport，OAuth/bearer）。
自托管包 `mem0-mcp-server`（PyPI 0.2.1）的 GitHub 仓库已于 2026-03-24 归档并指向托管版。
npm `mem0-mcp@0.2.0` 为第三方包（file-backed + Ollama），非官方。官方 SDK：npm `mem0ai@3.3.1` /
PyPI `mem0ai@2.2.1`。

托管版暴露 11 个工具（来源：docs.mem0.ai/platform/mem0-mcp；官方文档只给工具名 + 一句话描述，
完整 schema 明示需经 `tools/list` 获取）。按 REST API 文档还原的参数面（※ = 推断）：

```
add_memory(text|messages, user_id?|agent_id?|app_id?|run_id?, metadata?,
           immutable?=false, expiration_date?, infer?=true, custom_instructions?, enable_graph※)
search_memories(query, filters{user_id|agent_id|app_id|run_id, AND/OR/NOT, 运算符},
                top_k?=10(1-1000), threshold?=0.1, rerank?=false, show_expired?=false,
                reference_date?, fields?, categories?, metadata?)                            ※
get_memories(filters, page?, page_size?)                                                      ※
get_memory(memory_id)                                                                         ※
update_memory(memory_id, text?, metadata?)                                                    ※
delete_memory(memory_id) / delete_all_memories(scope) / delete_entities(entity_id)            ※
list_entities() / list_events(filters, page?) / get_event_status(event_id)                    ※
```

数据模型（search 200 响应 schema，docs.mem0.ai/api-reference/memory/search-memories）：

```json
{ "id": "uuid", "memory": "string(事实文本)", "user_id": "string|null",
  "agent_id": "string|null", "app_id": "string|null", "run_id": "string|null",
  "metadata": "object|null", "categories": ["string"], "expiration_date": "date|null",
  "created_at": "date-time", "updated_at": "date-time|null",
  "score": 0.82, "score_breakdown": {"semantic": 0.x, "bm25": 0.x, "entity": 0.x} }
```

- v3 add（`infer=true`）为异步：返回 `{event_id, status: PENDING|SUCCEEDED|FAILED}`；
  `infer=false` 同步返回 `results[{id, data.memory, event: ADD}]`。
- 图谱能力收缩：Platform 原生图已改为「实体节点↔提及该实体的记忆」共现链接，
  **无带类型的 source/relationship/target 三元组**（`relations` 字段废弃恒为 `[]`）；
  OSS 的 `enable_graph`/`graph_store`（Neo4j/Memgraph/Kuzu/AGE/Neptune）已整体移除
  （docs.mem0.ai/platform/features/graph-memory、open-source/graph_memory/overview 迁移指南）。

## 2. Letta（原 MemGPT）

三个工具面，须区分：

**(a) 托管 MCP server**（`https://api.letta.com/mcp`，bearer）——暴露「有状态 agent」为 MCP
工具，不是记忆工具面（docs.letta.com/platform/hosted-mcp）：

```
list_agents(name?|tags?|query?) / list_models() / create_agent(...)
send_agent_message(agent_id | conversation_id)   # 等待≤110s，返回 status: completed(含回复)
                                                 # | queued | wait_failed | acceptance_unknown
                                                 # | submission_failed
get_run(run_id) / get_reply(conversation_id)
```

**(b) Agent 内置记忆工具**（agent 自编辑记忆，非 MCP 协议工具）。现行官方文档仅 archival-memory
页给出示例级签名（docs.letta.com/guides/agents/archival-memory）：

```python
archival_memory_insert(content: str, tags?: list[str])
archival_memory_search(query: str, tags?: list[str], page: int = 0)  # 语义相关 passages
```

`core_memory_append/replace`、`conversation_search` 的现行官方 schema 未查到（公开仓库 main
分支仅剩政策文档，代码已移出；以上为社区/旧版参照）。

**(c) MemFS**：记忆是 agent 拥有的 git 仓库（context repository），label 投影为 markdown 文件，
「编辑须 commit+push 才成为记忆」，并发用 git worktrees
（docs.letta.com/concepts/memfs）。Memory block 模型：`label/description/value/limit/read_only/id`，
**并发语义 last-write-wins，无版本快照**（docs.letta.com/guides/agents/memory-blocks）。

## 3. 可验证性对照（核心结论）

| 机制 | mem0 | Letta | partisync（WP02 目标态） |
|---|---|---|---|
| Merkle inclusion proof | 无 | 无 | **memory_write/search/verify + 只读 proof API** |
| 内容寻址（hash=id） | 无（id=uuid） | 无 | memory_id = blake3(canonical 内容) |
| hash chain / 防篡改审计 | 无 | 无 | root 全量可重算 + 篡改必报（P20） |
| 版本/溯源 | 无版本链；仅单条 `immutable` 标志 | MemFS git 历史 = 事实 provenance（不承诺哈希验证/签名） | oplog 溯源（origin_device + HLC 全序，沿既有） |
| 审计日志 | **有**：`GET /v1/memories/{id}/history/`（old_memory/new_memory/event/created_at）+ events API | run/step trace（可观测性，非密码学审计） | 同步域 oplog 即审计链（M2 既有） |

**「可验证记忆层」在 MCP 生态是空白**——两家头部产品均无 Merkle/hash chain/内容寻址；
最接近者：mem0 的 history 审计端点（中心化信任）、Letta 的 git 版本化 MemFS（无哈希承诺）。
partisync 的 CAS + Merkle + 本地优先组合差异化成立（M9-roadmap-proposal N2 判断经本次
核查维持）。

## 4. 对 SPEC 的可执行输入

1. 行业默认工具动词 = `add/search/get/update/delete`；MCP 官方 memory reference server
   （@modelcontextprotocol/server-memory）为知识图谱三件套
   （create_entities/create_relations/search_nodes + read_graph/open_nodes/delete_*，
   JSONL 落盘）。WP02 工具命名（memory_write/search/verify）取「写入/检索/验证」三动词，
   与 roadmap 拍板一致；对标面缺口（update/delete/get）在 SPEC 非目标/风险节显式处置。
2. 数据模型字段命名向 mem0 对齐：`content`（事实文本）、`tags`、`metadata`（自由 object）、
   `created_ns`（=created_at 纳秒口径）、`origin_device`（partisync 特有溯源）、检索响应带
   `score`（一期非语义分数，见 SPEC §2.2）。
3. 两家 MCP 参数 schema 均不在文档页公开（mem0 明示走 `tools/list`）——本报告工具签名
   按 REST 文档还原（※ 项），SPEC 契约以 partisync 自身 `tools/list` 输出为唯一权威。
4. mem0 `infer=true`（LLM 抽取事实）为异步事件流——partisync 一期不做推理式写入
   （写什么存什么），登记为非目标。
