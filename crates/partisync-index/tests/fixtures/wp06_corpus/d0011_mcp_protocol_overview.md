title: MCP 协议概览
filename: mcp_protocol_overview.md
tags: [mcp, protocol, ai-agent, json-rpc, model-context-protocol]
updated_ns: 1704412800000000000

# MCP 协议概览

## MCP 是什么

Model Context Protocol（模型上下文协议）—— 让 AI 模型/LLM 与外部工具和数据源对话的开放协议。

- 2024-11 由 Anthropic 发布
- 2025-2026 生态快速扩张（OpenAI / Google 接入）
- 2026-07-28 当前 spec 版本

## 三大原语

| 原语 | 方向 | 用途 |
|---|---|---|
| tools | client → server | 模型调用工具 |
| resources | server → client | server 推送结构化数据 |
| prompts | server → client | server 提供 prompt 模板 |

## 传输

- **stdio**：本地进程，CLI/编辑器常用
- **HTTP + SSE**：远程 server
- **Streamable HTTP**：HTTP/2 streaming（2026 新）

## PartiSync MCP server

- 5 工具：asset_search / asset_read / asset_organize / dataset_export / job_status
- rmcp 3.4.0 SDK
- stdio only（无网络端口）
- stateless：每次工具调用自包含

## JSON-RPC 2.0 兼容

```json
{"jsonrpc": "2.0", "id": 1, "method": "tools/call",
 "params": {"name": "asset_search", "arguments": {"query": "..."}}}
```

## 与 function calling 关系

MCP 是 function calling 的标准化、跨厂商版本。
OpenAI / Anthropic / Google 均已支持。