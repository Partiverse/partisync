# Task: M11-WP02-T03 消费者迁移——sidecar/桌面/CLI search 只读化 + 锁冲突指引 + D3 清偿

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP02-T03 |
| **类型** | 实施（gateway/desktop/cli 三 crate 横切，R1） |
| **来源** | SPEC M11-WP02 §2.2/§3.0-T03 + ADR-0032 决策 2 + M10-WP03 §6-D3 |
| **创建日期** | 2026-10-09 |

## 交付

1. **sidecar**（gateway `build_server_state`）：`open_or_create` →
   `open_read_only`（eager read_only；0.2 修订：懒成分撤回——扩展注册
   表注入面 `load_registry` 依赖立即持有引擎，且 D5 根因是写锁非
   eager，P24 可观察契约逐字满足）。索引不存在 → 降级为空（口径不变）
2. **桌面**（state.rs `index()`）：`open_read_only`；索引不存在 →
   `DesktopError::Index` 结构化错误（指引建索引）
3. **CLI `search`**：`open_read_only` + 缺索引 hint（`partisync
   watch`/`reindex`）；**`reindex`/`watch` 写面维持不变**
4. **锁冲突错误结构化**：bm25 `open_or_create` 写者获取失败的错误信
   息含指引（另一写者持锁/只读查询不受影响/批量重建先停写面）
5. **D3 清偿**：ipc.rs `search`/`search_hybrid` 补 `Internal`→`Index`
   拨回（与 index_stats 同款；SPEC §2.5 契约对齐）

## 涉及文件清单（Iron Rule 9）

crates/partisync-gateway/src/mcp.rs · crates/partisync-desktop/src/state.rs ·
crates/partisync-desktop/src/ipc.rs · crates/partisync-cli/src/main.rs ·
crates/partisync-index/src/search/bm25.rs · docs/specs/M11-WP02.md（§7
0.2 行）· docs/tasks/M11-WP02-T03-consumer-migration.md

## 验收（SPEC §3.1 T03 行）

- [x] sidecar 启动不持写锁（复现脚本复跑：fuser/lsof 无句柄 + CLI
      search 同库成功——D5 症状消除直证，证据随 PR 评论）
- [x] CLI search 只读打开；reindex 仍可写（写面回归绿）
- [x] 锁冲突错误含指引文本且分类不劣化；D3 修正（`Index` 种类）
- [x] fmt/clippy/test 三件套绿（gateway/desktop/cli/index 四 crate）；
      改动仅限本卡清单；零新增顶层依赖
