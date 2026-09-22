title: AI Agent 框架对比
filename: ai_agent_frameworks_comparison.md
tags: [ai-agent, langchain, langgraph, autogen, frameworks]
updated_ns: 1726348800000000000

# AI Agent 框架对比

## 主流框架

| 框架 | 作者 | 类型 | 特点 |
|---|---|---|---|
| LangChain | LangChain | 库 + Chain | 生态最广 |
| LangGraph | LangChain | 图编排 | 复杂流程 |
| AutoGen | Microsoft | 多 Agent | 对话驱动 |
| CrewAI | CrewAI | 角色编排 | 团队隐喻 |
| Semantic Kernel | Microsoft | .NET 友好 | 企业集成 |
| LlamaIndex | LlamaIndex | RAG 优先 | 检索聚焦 |

## LangChain vs LangGraph

| 维度 | LangChain | LangGraph |
|---|---|---|
| 抽象 | Chain / Runnable | Graph |
| 复杂度 | 简单流程 | 复杂循环 |
| 状态管理 | 手动 | 内置 |
| 可观测性 | 中 | LangSmith |

## AutoGen 特点

- 多 Agent 对话
- UserProxyAgent + AssistantAgent
- 适合：研究/探索类任务

## CrewAI 特点

- 角色 + 任务 + 流程
- 隐喻友好（PM/工程师/QA）
- 适合：工作流自动化

## LlamaIndex 特点

- RAG 优先
- 数据连接器丰富
- 适合：知识库检索

## PartiSync 选择

**不绑定框架**——MCP 是中立协议。
Agent 用 LangChain / AutoGen / CrewAI 都可以，只要支持 MCP 即可。

## MCP 接入

```python
from mcp import Client

# ZCode 等支持 MCP 的 client 直接连接
client = Client("partisync-mcp")
result = await client.call_tool(
    "asset_search",
    {"query": "yesterday's vacation photos"}
)
```

## 实战建议

- 简单 RAG：LlamaIndex
- 复杂多步：LangGraph
- 团队自动化：CrewAI
- 研究：AutoGen

## 评测

- 任务完成率
- 步骤数
- Token 消耗
- 错误率

详见 `llm_evaluation_methodology.md`。