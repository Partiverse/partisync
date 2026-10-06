# Task: M10-WP04-T02 MCP 工具面——memory_update / memory_delete

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP04-T02 |
| **类型** | 实装（gateway MCP 公共工具面扩充；R1 全审——公共 API change 走 pre-merge-safety 核对） |
| **范围** | SPEC M10-WP04 §2.1 工具面契约 + crates/partisync-gateway/src/mcp.rs + tests/wp02_memory.rs + 本卡（桌面零改动） |
| **创建日期** | 2026-10-06 |
| **来源** | T01 能力面 + M9-WP02 §2.4 四改动点判例（all_tools/call_tool/instructions/schema） |

## 交付物

1. `memory_update(memory_id, content?, tags?, metadata?) →
   {old_memory_id, memory_id, root, tombstoned}`——限界沿 memory_write
   （content 非空 ≤64KiB / tags ≤32 / metadata object ≤16KiB；全缺省
   = 等价 no-op 仍返回原 id）。
2. `memory_delete(memory_id) → {memory_id, root, tombstoned: true}`；
   不存在/已墓碑 id → 工具级错误。
3. 两工具入 `all_tools()`/`call_tool`/instructions 四改动点（mcp.rs
   :466-493/581-583 判例）；schema 为对外唯一权威契约；
   `memory_verify(memory_id)` 响应透出 `deleted` 旗标（T01 报告面）。

## 测试（gateway/tests/wp02_memory.rs 延伸 + e2e）

tools/list schema 逐字段断言；三工具全功能 e2e（stdio 真实 gateway）；
限界拒绝矩阵；desktop `mcp_call` 透传零改动确认（既有 stub 不回归）。
已核：gateway tests 无工具总数锁（PR 内 grep 留痕）。

## 验收

- [ ] §3 T02 探针绿；
- [ ] fmt/clippy/test 三件套绿；零新增顶层依赖；桌面源码零 diff；
- [ ] 提交挂 Task-ID `M10-WP04-T02`。
