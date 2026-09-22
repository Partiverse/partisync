# Threat Model: MCP 网关面（M4-WP06）

> 文档编号: TM-M4-WP06-001
> 适用版本: PartiSync M4-WP06（2026-09-23）
> 上游: `docs/specs/M4-WP06.md`
> 方法: STRIDE（Spoofing / Tampering / Repudiation / Information Disclosure / DoS / Elevation of Privilege）
> 范围: MCP 5 工具 + CLI 入口 + 内部进程边界
> 不在范围: iroh 数据面（WP04）、密码学面（M2 SEC-AUDIT-2026-M3-002 已覆盖）

## 1. 系统边界

```
┌────────────────────────────────────────────────────────┐
│  外部                                              │
│    │                                                │
│    │ MCP (stdio JSON-RPC)         CLI (子进程 argv) │
│    ▼                                                ▼
│  ┌─────────────────────────────────────────────────┐  │
│  │  partisync-mcp bin / partisync CLI              │  │
│  │  ├─ 5 工具面（M4-WP03）                        │  │
│  │  ├─ rmcp 3.4.0 服务端（MCP 2026-07-28 stateless）│  │
│  │  └─ sqlx 0.9 → SQLite graph.db                  │  │
│  └─────────────────────────────────────────────────┘  │
│    │                                                │
│    ├──→ tantivy BM25 索引（本地 fs）                 │
│    ├──→ usearch 向量索引（本地 fs）                   │
│    ├──→ Sidecar 产物目录（fs）                       │
│    ├──→ content.mime/size/c2pa TEXT 列（SQLite）    │
│    └──→ dataset_export 输出目录（fs）               │
└────────────────────────────────────────────────────────┘
```

## 2. 信任级别

| 级别 | 主体 | 信任假设 |
|---|---|---|
| T0 | 本机用户 | 全权：可读 DB / 索引 / sidecar 目录 |
| T1 | MCP 工具调用方（AI Agent / CLI） | 受限：受 5 工具契约约束；无文件系统任意访问 |
| T2 | 网络远端 | **不可信**：MCP server 无网络监听（stdio only） |
| T3 | 第三方模型/服务 | 不存在：reranker/embedding 全本地 |

**关键不变量——MCP server 是 stdio-only**：
`partisync-mcp` bin 不绑定任何 TCP/UDP 端口；网络攻击面 = 0。
所有威胁均围绕**stdin JSON-RPC 输入**与**进程文件系统读路径**。

## 3. STRIDE 矩阵

### 3.1 asset_search（`crates/partisync-gateway/src/mcp.rs:488`）

| 维度 | 威胁 | 攻击路径 | 现有缓解 | 残余风险 |
|---|---|---|---|---|
| **S** | 伪造 client 身份 | 无——stdio 进程父子信任 | RMCP 协议无鉴权层（设计） | 低：本地进程父子信任已足够 |
| **T** | 篡改查询结果返回路径 | 注入 `query` 字段触发 tantivy 查询构造异常 | tantivy 自身异常隔离；BM25 走 safe QueryParser | **中**：Unicode 控制字符 / 嵌套正则未实测 |
| **R** | 重放 | 无状态查询，每次独立 | 无 | 低：查询无副作用 |
| **I** | 信息泄露——通过侧信道 | 大 limit 返回触发栈外 OOM；错误消息含 SQL 细节 | `limit.clamp(1, 100)` + `internal_error` 不透 SQL | **中**：错误路径需实测验证 |
| **D** | DoS——limit=100 + 100 万条目 + 多次调用 | BM25 全表扫描 | tantivy TopDocs limit 100 已限 | **中**：并发未限 |
| **E** | 提权 | 无——查询只读 | 无 | 低 |

### 3.2 asset_read（`mcp.rs:546`）

| 维度 | 威胁 | 攻击路径 | 现有缓解 | 残余风险 |
|---|---|---|---|---|
| **S** | 伪造 content_id 跨库访问 | `content_id` 不属于本 DB 时返回 tool_err（设计） | 已实现 | 低 |
| **T** | 篡改 sidecar_items 返回 | SQL 注入 `content_id` 字段 | sqlx 0.9 参数化绑定 | 低：参数化已落地 |
| **R** | 不可追溯 | 工具调用日志未落 | 无 | **中**：需补 MCP 调用审计日志 |
| **I** | 信息泄露——超长 detail 字段（OCR 文本可能 100 KB+） | 工具返回 JSON 体积爆炸 | limit=1 调用固定上限 | 低 |
| **D** | DoS——DB 慢查询 | 故意构造全表扫描型 content_id | sqlx 已参数化 | 低 |
| **E** | 提权 | 无——只读 | 无 | 低 |

### 3.3 asset_organize（`mcp.rs` ~T04 实现）

| 维度 | 威胁 | 攻击路径 | 现有缓解 | 残余风险 |
|---|---|---|---|---|
| **T** | 注入——operations.value 含恶意 tag 名 | 路径穿越（`../../etc`） | 标签表无文件路径；tag 长度未限 | **中**：tag 长度+字符集需补白名单 |
| **R** | 软删除不可追溯 | `delete` 操作无审计 | 无 | **中**：需补 |
| **I** | 信息泄露——错误信息含 DB schema | `ErrorData::invalid_params` 透 schema | 未实测 | **中** |
| **D** | DoS——operations 数组超长（1 万条） | 单事务批写阻塞 | `set_tag` 单事务重写（WP03 T04 验收） | **中**：批大小未上限 |
| **E** | 提权——跨库操作 | 当前无多租户概念 | 无 | 低（M5 涉及） |

### 3.4 dataset_export（`mcp.rs` ~T04）

| 维度 | 威胁 | 攻击路径 | 现有缓解 | 残余风险 |
|---|---|---|---|---|
| **T** | 路径穿越——`output_dir` 越界 | 用户控制 `output_dir` 参数 | 未实测 | **高**：未做 canonicalize 检查 |
| **I** | 信息泄露——导出包含 `include_vectors` 默认 true | 导出向量 JSONL 进入用户控制目录 | 默认 true 是设计 | 中：默认值需审视 |
| **D** | DoS——`shard_size=0` + 1M records 内存爆炸 | 流写已实现（WP03 T04 验收） | JSONL 流写 | 低：已流写 |
| **E** | 跨库导出 | `content_ids` 不属于本库 | DB JOIN 自然过滤 | 低 |

### 3.5 job_status（`mcp.rs`）

| 维度 | 威胁 | 攻击路径 | 现有缓解 | 残余风险 |
|---|---|---|---|---|
| **I** | 信息泄露——checkpoint 路径含敏感目录 | `job_id` 含 SQL 注入试探 | sqlx 参数化 | 低 |
| **D** | 列出全部 jobs 触发大结果集 | `job_id=""` 列出 recent | 已实现 LIMIT | 低 |

### 3.6 CLI 入口（`partisync-cli`）

| 维度 | 威胁 | 攻击路径 | 现有缓解 | 残余风险 |
|---|---|---|---|---|
| **T** | CLI 参数注入 | argv 解析 | clap 等标准化解析 | 低（未来迁移） |
| **I** | `partisync search` 返回完整 highlight | 无敏感信息 | 低 | 低 |
| **D** | `partisync sidecar-run` 全树递归无大小上限 | 用户控制 root | 无 | **中**：需补深度/文件数上限 |

## 4. 高风险结论

按残余风险**高 > 中 > 低**分桶：

| 等级 | 项 | 路径 |
|---|---|---|
| **高** | dataset_export `output_dir` 路径穿越 | T05 探针覆盖 + 立即修（canonicalize） |
| **中** | asset_search 错误消息含 SQL 细节 | T05 探针覆盖 |
| **中** | asset_organize tag 长度/字符集 | T05 探针覆盖 |
| **中** | MCP 调用无审计日志 | M5+ 范围（本 WP 登记） |
| **中** | 并发未限——DoS 防护缺 | T05 探针覆盖 |
| **中** | CLI sidecar-run 无深度上限 | T05 探针覆盖（受限于既有 API） |

## 5. 不在威胁模型范围（已治理）

- **密码学面**：M2 密码学审计（`SEC-AUDIT-2026-M3-002`）已覆盖 24 子项 + 4 项整改复核；本 WP 不重复
- **iroh 数据面**：M4-WP04 治理范围；M5 设备侧 iroh UploadAck 独立威胁建模
- **依赖供应链**：cargo deny 四段通过（ADR-0014/0016/0019/0020 已豁免）；
  依赖供应链面在 cargo deny 工件里

## 6. 与外部渗透 RFC 的衔接

本威胁模型作为 `docs/security/pentest-rfc.md` 的**输入文档**——外部 vendor
按本模型 §3 矩阵的「残余风险」中-高项作为重点攻击路径。

外部 vendor 须独立产出 `threat-model-findings.md`，与本文件双向校验（独立验证
或反驳每条威胁的真实性与缓解有效性）。