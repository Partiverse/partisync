# M4-WP03 KPI 底稿（T05 基准与验收报告）

日期: 2026-09-22 · 环境: Apple Silicon (arm64, macOS 25.6.0, 24 GB) ·
release profile · 关联: SPEC M4-WP03 验收标准、ADR-0019（rmcp = 3.4.0）

## 0. 开放项闭合（2026-09-22 续接会话回填）

T05 时登记的开放项处理结果：

| 优先级 | 开放项 | 结果 |
|---|---|---|
| P0 | cargo deny check rmcp 进场复核 | ✅ **四段全绿**（licenses/bans/sources 真实退出码 0；advisories `--offline` 本地缓存 DB 通过——GitHub 不可达无法 fetch 最新，缓存截至上次成功同步；bans 曾 FAIL：gateway path 依赖缺 version 补齐后通过） |
| P0 | dataset_export record_count 实测 | ✅ 内存库测试覆盖（`dataset_export_writes_jsonl_shards`：分片写入 + 内容断言 + record_count 断言） |
| P1 | job_status checkpoint 格式对齐 | ✅ 已对齐——graph/jobs.rs `set_status(checkpoint=COALESCE)`：**sidecar 作业不写 checkpoint**（checkpoint 仅 indexer 作业使用，值为路径字符串非 JSON）；job_status 做宽松解析（JSON stage/next 字段，非 JSON 原样返回），格式演进不破坏 |
| P1 | set_tag 原子性 | ✅ 单事务重写——asset_organize 全部操作在单 `BEGIN..COMMIT` 内执行，任一失败整体回滚（测试 `asset_organize_bad_content_rolls_back_tx` 验证回滚） |
| P2 | 分片导出（>10,000 条） | ✅ `shard_size` 参数实装（每片记录数，缺省单文件），LIMIT 10000 上限保留为防御边界 |
| **P0** | **MCP e2e 集成测试（真实 AI Agent 调用）** | ⏸ **用户指令后置为独立待办**——本会话完成工具级离线测试（10 例）替代覆盖 handler 行为 |

### 附带修复（WP02 遗留债务，阻塞 WP03 编译，一并披露）

T05 后首次真正编译 partisync-index（WP02-T06 时 numkong 阻塞从未编译过），
暴露 6 类错误 + 2 个行为 bug，全部修复：

| 问题 | 修复 |
|---|---|
| `usearch::Options` 不存在（真实 API = `IndexOptions` plain struct + `MetricKind::Cos` + `ScalarKind`） | vector.rs 全量重写，API 口径逐一从 usearch 2.26.2 源码核实 |
| usearch key 为 `u64`（WP02 代码传 String） | content_id → blake3 前 8 字节 u64 映射（`content_key`），碰撞概率披露于模块文档 |
| usearch 要求 reserve 先行（"Reserve capacity ahead of insertions!"） | upsert 前容量检查 2× 扩容 |
| `sqlx` 未在 index/Cargo.toml 声明（writer.rs 使用） | 补依赖声明（WP02-T02 遗漏） |
| `hybrid::` 模块路径 + 私有类型跨模块导入（engine.rs） | imports 重整：Bm25Hit 等直接从 bm25/vector 源头导入 |
| `Box<dyn Future>` await 不稳定（E0277） | `Bm25Source` trait 签名改 `Pin<Box<dyn Future>>` |
| tantivy 0.26：`QueryParser::for_index` 需 `Vec<Field>`；`TopDocs` 需 `.order_by_score()`；`commit()` 返回 `Opstamp` 且需 `&mut` | bm25.rs 逐一对源码核实修正 |
| writer.rs `tag_entry` 表名/关联列错（schema 实为 `entry_tag`，经 `entry_path` 关联） | 修正 SQL（WP02 笔误） |
| **行为 bug**：`open_or_create` 以「目录存在」判 tantivy 打开（预建空目录即炸） | 判据改 `meta.json` 存在 |
| **行为 bug**：bm25 `err()` 吞掉底层错误信息 | source 保留 `{what}: {e}` |
| byteorder `read_f32_into` 误用 | 改 `f32::from_le_bytes` 手写（byteorder 依赖此后无引用） |

### numkong SIMD 编译障碍（环境问题，本轮找到可行绕过）

Apple Clang 16 编译 numkong SME 探针 TU 时 clang 前端直接崩溃（Abort trap 6），
`NUMKONG_DISABLE_SIMD=1` 单独无效（探针仍测 SME）。**可行绕过**：
`NK_TARGET_<ISA>=0` 环境变量强制关闭全部 SVE/SME 探针（numkong 7.8.2 build.rs:438
支持 env override），NEON 路径正常编译。完整环境变量清单见 numkong-env 惯例
（16 个 NK_TARGET_* 变量）。此为构建环境解法，代码零改动。

## 1. 口径声明（诚实标注）

- **工具级测试 10 例全绿**（gateway 内存 SQLite + index 单元测试 6 例），
  真实 MCP Client（stdio JSON-RPC 往返）与 AI Agent 端到端场景待独立待办；
- **graph.db 路径**：`~/.partisync/graph.db`（可通过 `run_mcp_server(path)` 覆盖）；
- **numkong SIMD**：SVE/SME 探针强制关闭下编译（见 §0），NEON 可用；
- **asset_organize 幂等性**：add_tag/remove_tag/set_tag 均幂等（entry_tag
  ON CONFLICT DO UPDATE 复活墓碑行）；delete 为软删除（entry.state=1）；
- **dataset_export 规模**：防御上限 10,000 条；分片经 `shard_size` 参数。

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
| record_count / total_bytes 正确 | ✅ | 内存库测试断言（§0 P0 闭合） |
| 24h 过期提示 | ✅ | expires_at_ns = now + 86_400_000_000_000 |
| 分片导出（shard_size） | ✅ | 测试覆盖每片记录数与内容断言 |

## 6. job_status 正确性

| 口径 | 结果 | 备注 |
|---|---|---|
| 无 job_id 时返回最近 20 条 | ✅ | ORDER BY created_ns DESC LIMIT 20 |
| job_id 精确查找 | ✅ | AND kind='sidecar' 过滤 |
| checkpoint 解析 current_stage | ✅ | 已对齐：sidecar 作业不写 checkpoint（仅 indexer 用），宽松解析非 JSON 原样返回（§0 P1 闭合） |
| status 0-4 → 字符串映射 | ✅ | queued/running/interrupted/done/failed |

## 7. MCP 传输（RMCP 2026-07-28）

| 口径 | 结果 | 备注 |
|---|---|---|
| `serve_server(state, stdio())` 编译 | ✅ | `impl<H: ServerHandler> Service<RoleServer>` blanket impl |
| 5 工具全部注册 | ✅ | `list_tools()` 返回完整清单 |
| Tool 输入/输出 JSON Schema | ✅ | rmcp schema 支持 |
| 错误返回 `server::Error` | ✅ | InvalidParams / InternalError / ToolNotFound |

## 8. 依赖与门禁（2026-09-22 续接会话实测）

| 门禁 | 结果 |
|---|---|
| `cargo fmt --all --check` | ✅ |
| `cargo clippy -p partisync-index -p partisync-gateway --all-targets -- -D warnings` | ✅ 0 warning（SVE/SME 探针关闭环境） |
| `cargo test -p partisync-index` | ✅ 6/6（bm25/vector/hybrid/engine） |
| `cargo test -p partisync-gateway` | ✅ 10/10（五工具内存库测试） |
| `cargo deny check` 四段 | ✅ licenses/bans/sources exit 0；advisories `--offline`（本地缓存 DB，网络故障披露见 §0） |

## 9. 开放项（状态见 §0 闭合表）

剩余项状态（2026-09-22 L2 验收续接）：

| 项 | 结果 |
|---|---|
| **P0 MCP e2e L1（协议层）** | ✅ **闭合**（`ce515aa`）——`partisync-mcp` bin + rmcp client 真实 spawn 子进程：initialize → tools/list → 五工具全链 → 未知工具协议错误，测试 `mcp_stdio_full_chain` 绿。途中修复：`RunningService` drop 即 shutdown（bin 须 `waiting()`） |
| **P0 MCP e2e L2（DoD 场景）** | ✅ **五步链路全绿**（真实库 INBOX 2914 文件/2797 content）：asset_read（元数据+sidecar 状态）→ asset_organize preview（无事务 ID 不落库）→ commit（`txn_e45b7471`）→ 回读 tags=`["e2e-verified"]` + DB 落库实证（entry_tag deleted=0）→ dataset_export JSONL 落盘（content_id 匹配）。**执行方式披露**：stdio JSON-RPC 直驱 partisync-mcp（协议真实）；ZCode 宿主自动连接未通（见下） |
| P0 ZCode 宿主 MCP 连接 | ✅ **闭合**——两步修复后宿主真实连接：(1) 注册从 workspace `.zcode/config.json` 改至 user scope `~/.zcode/cli/config.json`（Desktop 不读 workspace 文件作 MCP 源）；(2) tools/list 补 SEP-2549 必填字段 `ttlMs`/`cacheScope`（宿主 zod schema 严格校验，rmcp 对 None 省略序列化导致 `invalid_type`——`1a8bb47`）。终态：新会话工具列表出现 `mcp__partisync__*`，Agent 自主调用 `asset_read` 成功返回 `e2e-verified` 标签与 sidecar 状态 |
| 遗留发现 | `LocalContentLoader` 对 DB 有记录但文件已删的 content（INBOX/temp 下 .tmp）整批 fatal 而非跳过——L2 建库时手动清理 1 条幽灵记录绕过；skip 语义修复留待后续任务 |

**L2 DoD 结论**：归类→预览→提交→导出 manifest 完整链路在真实资产库上验收通过（含落盘产物实证）；
**宿主真实验收通过**（GLM Agent 经 ZCode MCP 集成自主调用工具）。WP03 P0 开放项全部闭合。
