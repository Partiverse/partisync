# Task: M9-WP02-T04 MCP 三工具（memory_write / memory_search / memory_verify）

> **范围外**：CAS exists() 精化随 T05；语义向量检索不做（§4）；桌面源码
> 零改动（mcp_call 透传既有，仅补 stub 单测）。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP02-T04 |
| **类型** | 实装（公开 MCP 工具面；R1 全审 + 对抗审查；schema additive + 新公开 API 须人工终审重点） |
| **优先级** | P0（M9 主线 α 第三实施件） |
| **范围** | SPEC M9-WP02 §2.4 + §3 工具面验收 + §5 清单（store/memory/mcp/两测试文件）+ 本卡 |
| **创建日期** | 2026-10-04 |
| **来源** | SPEC M9-WP02 批准（PR #122）+ T03 收官（PR #128，main=`bf219bc`） |

## R1 落锤（SPEC §6-R1，本卡记录）

- **FTS5 可用性**：bundled sqlite 3.51.3（libsqlite3-sys 0.37.0，build.rs
  `-DSQLITE_ENABLE_FTS5` 编译期实证）。tokenizer 选 **trigram**（≥3.34）：
  unicode61 对 CJK 连续串不成词，trigram 子串语义正确；查询 <3 字符
  trigram 无法命中 → 该情形走 LIKE。
- **FTS 表不在 schema.sql**：防御性建表（migrate 内 `let _ =` 吞错）——
  非 bundled/裁剪 sqlite 构建缺失 FTS5 模块时，CREATE VIRTUAL TABLE 失败
  静默降级，SPEC「否则 LIKE 兜底」路径真实可达；检索时查 sqlite_master
  判可用性。

## 交付物

1. **store.rs**：`memory_search`（FTS5 trigram / LIKE 双路径 + tag 精确
   过滤（json_each）+ memory_id 精确查 + limit/offset；score = 匹配秩，
   FTS 路径 -bm25，LIKE 路径常量秩；ORDER BY score DESC, created_ns
   DESC）+ `Store::from_pool`（gateway 同池共享构造，迁移幂等重跑）。
2. **memory.rs**：`MemorySearchHit/Report` + `inclusion_proof`（§2.3/§2.4
   带 id 分支：leaf_hash + audit_path + root + ok 自包含验证）。
3. **mcp.rs 四改动点**：`all_tools()` 三 Tool schema（tools/list 唯一
   契约）+ `call_tool` 三臂 + 三 handler（写入限界：content 非空
   ≤64KiB / tags ≤32 / metadata object ≤16KiB，超限 tool_err 不截断）+
   instructions 更新。桌面 `mcp_call` 透传零改动。
4. **探针（随 tests commit）**：gateway `tests/wp02_memory.rs`（tools/list
   三工具 schema 断言 + 三工具调用冒烟 + 限界拒绝 + FTS/LIKE 双路径）+
   桌面 `tests/commands.rs` mcp_call("memory_*") 透传 stub 用例。

## 验收

- [x] tools/list 含三工具且 schema 与 §2.4 一致（gateway e2e
      `wp02_memory_tools_list_and_call_contract` 真实 stdio 握手断言）；
- [x] 桌面 mcp_call("memory_*") 透传 stub 单测绿，桌面源码零改动；
- [x] fmt/clippy/test 三件套全绿；零新增顶层依赖；
- [x] 提交挂 Task-ID `M9-WP02-T04`；
- [ ] 对抗审查 + 人工终审（新公开 API `Store::from_pool` /
      `memory_search` / `inclusion_proof` + FTS 触发器面）随 PR 执行。
