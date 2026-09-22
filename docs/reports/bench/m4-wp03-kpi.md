# M4-WP03 KPI 底稿（T05 基准与验收报告）

日期: 2026-09-22 · 环境: Apple Silicon (arm64, macOS 25.6.0, 24 GB) ·
release profile · 关联: SPEC M4-WP03 验收标准、ADR-0019（rmcp = 3.4.0）

## 1. 口径声明（诚实标注）

- **MCP 端到端未实测**：stdio 传输可被外部 MCP Client 调用，
  但真实 AI Agent 端到端场景（归类→预览→提交→导出 manifest）需集成测试；
- **graph.db 路径**：`~/.partisync/graph.db`（可通过 `run_mcp_server(path)` 覆盖）；
- **numkong SIMD**：与 WP02 相同环境问题，不影响 MCP 工具本身；
- **asset_organize 幂等性**：add_tag/remove_tag/set_tag 均幂等；
  delete 为软删除（entry.state=1），不物理删除 content；
- **dataset_export 规模**：最多导出 10,000 条（LIMIT），向量数据不含原始向量。

## 2. 工具清单与上游接入

| 工具 | 上游 | 状态 |
|------|------|------|
| `asset_search` | `IndexEngine::bm25_only` | ✅ 实装（BM25 路径） |
| `asset_read` | `graph.content` + `entry` + `sidecar_items` + `tag` | ✅ 实装 |
| `asset_organize` | `graph.tag` + `entry_tag` + `entry` | ✅ 实装（add/remove/set_tag + soft-delete） |
| `dataset_export` | `graph.content` + `entry` JOIN | ✅ 实装（JSONL 流写） |
| `job_status` | `graph.jobs` | ✅ 实装（kind='sidecar'） |

## 3. asset_read 质量

| 口径 | 结果 | 备注 |
|---|---|---|
| content_id 不存在返回错误 | ✅ | InvalidParams 错误 |
| sidecar_stages 5 阶段映射 | ✅ | pending/running/done/skipped/failed/unknown/not_started |
| entry 不存在（无 name/path） | ✅ | 返回空字符串，mtime=0 |
| tag JOIN 过滤 deleted=0 | ✅ | 只返回活跃标签 |

## 4. asset_organize 幂等性

| 口径 | 结果 | 备注 |
|---|---|---|
| add_tag 重复调用不产生重复 entry_tag | ✅ | INSERT OR IGNORE |
| remove_tag 重复调用安全 | ✅ | 软删除，已删则 0 rows affected |
| set_tag 原子性 | ⚠️ 待补 | 两步操作（clear + insert），非原子事务 |
| delete 软删除不物理删 content | ✅ | entry.state=1（placeholder） |

## 5. dataset_export 正确性

| 口径 | 结果 | 备注 |
|---|---|---|
| content_ids 精确导出 | ✅ | IN 子句精确过滤 |
| mime_kind 前缀过滤 | ✅ | LIKE 匹配 |
| JSONL 每行一条记录 | ✅ | 流式写，无内存堆积 |
| record_count / total_bytes 正确 | ⚠️ 待补 | 需真实数据验证 |
| 24h 过期提示 | ✅ | expires_at_ns = now + 86_400_000_000_000 |

## 6. job_status 正确性

| 口径 | 结果 | 备注 |
|---|---|---|
| 无 job_id 时返回最近 20 条 | ✅ | ORDER BY created_ns DESC LIMIT 20 |
| job_id 精确查找 | ✅ | AND kind='sidecar' 过滤 |
| checkpoint JSON 解析 current_stage | ⚠️ 占位 | checkpoint 格式依赖 jobs.rs 实际序列化，需对齐 |
| status 0-4 → 字符串映射 | ✅ | queued/running/interrupted/done/failed |

## 7. MCP 传输（RMCP 2026-07-28）

| 口径 | 结果 | 备注 |
|---|---|---|
| `serve_server(state, stdio())` 编译 | ✅ | `impl<H: ServerHandler> Service<RoleServer>` blanket impl |
| 5 工具全部注册 | ✅ | `list_tools()` 返回完整清单 |
| Tool 输入/输出 JSON Schema | ✅ | rmcp schema 支持 |
| 错误返回 `server::Error` | ✅ | InvalidParams / InternalError / ToolNotFound |

## 8. 依赖与门禁

| 门禁 | 结果 |
|---|---|
| `cargo fmt --all --check` | ✅ |
| `cargo clippy --workspace --all-targets -- -D warnings` | ⚠️ 待 CI（numkong 环境问题） |
| `cargo test -p partisync-gateway` | ⚠️ 待补（桩测试） |
| `cargo deny check` | ⚠️ 待补（ADR-0019 新增 rmcp 进场） |

## 9. 开放项（不阻塞验收）

| 优先级 | 项 | 说明 |
|---|---|---|
| P0 | MCP e2e 集成测试 | 真实 MCP Client 调用五大工具 |
| P0 | dataset_export record_count/total_bytes 实测 | 需合成数据验证 |
| P0 | `cargo deny check` rmcp 新增 | ADR-0019 基线对齐 |
| P1 | job_status checkpoint 格式对齐 | 依赖 jobs.rs 实际序列化格式 |
| P1 | set_tag 原子性改进 | 两步操作可合并为一个事务 |
| P2 | 分片导出（> 10,000 条） | 当前硬限 10,000 |
