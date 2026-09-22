# ADR-0019: M4-WP03 MCP Server 依赖批次：rmcp

版本: 1.0 · 状态: **起草中** ·
关联: M4-WP03（MCP Server 工具面）、ADR-0017（推理栈批次先例）、
RMCP 规范 2026-07-28（stateless）
负责人: @lead · 起草日期: 2026-09-22

## 背景

M4-WP03 需要将 `IndexEngine::hybrid_search` 等能力以 MCP（Model Context Protocol）工具形式暴露。
MCP crate 选型：

| crate | 版本 | spec | 评估 |
|-------|------|------|------|
| `rmcp` | 3.4.0 | MCP 2026-07-28（stateless） | ✅ 规范版本匹配 |
| `brontes` | 0.7.3 | MCP 2026-07-28 | ✅ 规范版本匹配 |
| `rust-mcp-sdk` | 2.0.0 | MCP 2025 | ❌ 规范版本旧 |

选 `rmcp = "3.4.0"`，理由：spec 版本精确匹配 2026-07-28、crates.io 下载量高、生态成熟。

## 决策

### 1. 精确锁定版本

```toml
rmcp = "=3.4.0"   # exact，锁定 2026-07-28 spec 实现
```

理由：MCP 协议仍在活跃演进，版本漂移会导致兼容性问题。

### 2. feature 门控：默认开启（gateway 专用）

MCP server 是 gateway crate 的主要功能，无 feature 门控需求。

### 3. 依赖方向

```
partisync-gateway
  → partisync-index   (hybrid_search API，只读)
  → partisync-graph  (content/sidecar_items/jobs 表，只读)
  → partisync-ai     (SidecarStore，只读)
```

符合架构：gateway 为叶节点（不反向依赖能力层）。

### 4. 工具清单

| 工具 | 上游 |
|------|------|
| `asset_search` | `IndexEngine::hybrid_search` |
| `asset_read` | `graph.content` + `sidecar_items` |
| `asset_organize` | `graph.asset_txn` 事务 |
| `dataset_export` | manifest writer |
| `job_status` | `jobs.jobs` 表 |

## 风险

- **MCP 协议演进**：`rmcp` 3.x 后续升级可能打破 2026-07-28 兼容性——监视 `rmcp` release notes，
  重大变更走 ADR 更新
- **传输层**：stdio 传输（RMCP 推荐），无连接状态，适合 CLI 场景；Hub 多客户端场景需未来扩展
