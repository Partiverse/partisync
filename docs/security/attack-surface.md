# Attack Surface: MCP 网关面（M4-WP06）

> 文档编号: AS-M4-WP06-001
> 适用版本: PartiSync M4-WP06（2026-09-23）
> 上游: `docs/security/threat-model-mcp-gateway.md`
> 范围: 进程边界 + 文件系统 + DB + 索引 + 网络（无）

## 1. 进程边界

| 进程 | 入口 | 网络监听 | 受信父进程 |
|---|---|---|---|
| `partisync-mcp` | stdin/stdout JSON-RPC | 无 | ZCode/AI Agent |
| `partisync` CLI | argv 子进程 | 无 | shell |

**结论**：MCP server **零网络端口**——所有攻击面 = stdin/stdout JSON-RPC 解析 +
本地 fs/DB 读路径。

## 2. 文件系统读路径

| 路径 | 来源 | 读取者 | 风险 |
|---|---|---|---|
| `~/.partisync/graph.db`（SQLite） | CLI/MCP `--db` 默认 | sqlx 0.9 | 低（本地） |
| `~/.partisync/index/`（tantivy BM25 + usearch） | CLI/MCP `--index-root` | tantivy/usearch | 低（本地） |
| `./partisync.sidecar/` | `partisync sidecar-run --sidecar-dir` | FsBlobSink | 低（本地） |
| 模型目录 `~/.partisync/models/`（reranker 可选） | CLI `--reranker-model-dir` | fastembed | 低（本地） |
| `output_dir`（dataset_export） | **MCP 工具参数** | tokio fs write | **高**：用户控制路径，需 canonicalize |

## 3. 数据库写入面（asset_organize）

| 操作 | 表 | 风险 |
|---|---|---|
| add_tag | entry_tag（JOIN entry_path） | 标签字符串长度未限 |
| remove_tag | entry_tag | 同 |
| delete（软删除） | entry（state 字段） | 无审计日志 |

## 4. 索引写入面

| 操作 | 路径 | 风险 |
|---|---|---|
| tantivy writer.add_document | tantivy IndexWriter | tantivy 自身异常隔离 |
| usearch Index::add | usearch | usearch 自身异常隔离 |
| reranker 模型加载 | fastembed | 路径未 canonicalize（路径穿越低——本地用） |

## 5. 网络面

| 协议 | 端口 | 状态 |
|---|---|---|
| TCP | 0（无监听） | N/A |
| UDP | 0 | N/A |
| iroh（数据面，WP04） | 随机端口 | WP04 范围，不在本 WP |
| Hub 同步 | iroh 0-RTT | WP04 范围 |

## 6. 敏感数据流

| 数据 | 流向 | 处置 |
|---|---|---|
| 用户资产文件 | 仅 fs read | 不外发 |
| BM25 索引 + 向量 | 仅 fs read/write | 不外发 |
| MCP 工具返回 JSON | stdout → Agent | Agent 侧隔离（不在 PartiSync 范围） |
| dataset_export 输出 | 用户指定 fs 路径 | 用户控制——已在 §2 高风险 |

## 7. 第三方依赖（cargo deny 四段通过）

| 依赖 | 攻击面 |
|---|---|
| `rmcp 3.4.0`（MCP server） | JSON-RPC 解析 |
| `sqlx 0.9`（SQLite） | 参数化绑定 |
| `tantivy 0.26`（BM25） | 全文查询解析 |
| `usearch 2.26`（向量） | HNSW |
| `fastembed`（reranker 可选） | ONNX runtime |
| `c2pa =0.90.22`（C2PA 校验） | JUMBF / 密码学 |

**已知 supply chain 风险（已 ADR）**：
- `RUSTSEC-2023-0071`（rsa Marvin）——c2pa 公钥验证路径不可达（ADR-0020）
- deny ignore 共 5 条（ADR-0014×1 + ADR-0016×3 + ADR-0020×1）

## 8. 测试覆盖

| 攻击面 | 探针位置 |
|---|---|
| MCP 注入 | `crates/partisync-gateway/tests/pen_test.rs` |
| MCP 鉴权 | 同上 |
| MCP DoS | 同上 |
| dataset_export 路径 | 同上 |
| CLI sidecar-run | `crates/partisync-cli/tests/`（本 WP 不覆盖，登记开放项） |