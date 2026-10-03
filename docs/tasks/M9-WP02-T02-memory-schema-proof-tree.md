# Task: M9-WP02-T02 记忆 schema + 独立二叉证明树 + P20 登记

> **范围外**：session 应用臂 + 双端收敛探针随 T03；MCP 三工具随 T04；
> CAS exists() 精化随 T05。本卡不改既有对账分桶树（P7）与 sync/session。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP02-T02 |
| **类型** | 实装（schema 演进 + 承诺树；R1 全审 + 对抗审查） |
| **优先级** | P0（M9 主线 α 第一实施件） |
| **范围** | SPEC M9-WP02 §2.2/§2.3/§2.7 + §5 清单中 graph 侧文件（schema/store/memory/lib/Cargo）+ P20 登记 + 本卡；零外部新依赖（serde_json 转正为 graph 运行时依赖，workspace 既有） |
| **创建日期** | 2026-10-04 |
| **来源** | SPEC M9-WP02 批准（PR #122，main=`a82719d`）+ ADR-0029 已接受 |

## 交付物

1. **schema v16**（additive-only）：`memory` 表（内容寻址主键 + 列级
   content_hash + hlc 可空沿 tag.updated_hlc 判例 + deleted 预留位）+
   `memory_root` 快照表（派生值）。
2. **memory.rs**：canonical JSON（显式键排序，不依赖 serde_json Map
   实现——§6-R2 加固）+ `memory_identity`（内容寻址）+ RFC 6962 式 MTH
   （blake3 0x00/0x01 域分离，叶按 memory_id 升序）+ `audit_path` /
   `verify_inclusion`（O(log n)，自包含验证）。
3. **store.rs**：`memory_write`（幂等 + oplog entity="memory" domain=1 +
   行 hlc 回填 + 根快照刷新，单入口）+ `memory_by_id` / `memory_rows`
   （叶序）+ `refresh_memory_root` / `memory_root_snapshot` /
   `verify_memory`（快照根 vs 重算根 + 列级校验）。
4. **P20 登记**：`docs/tests/properties.md`（登记先于探针代码，随本 PR）。

## 验收

- [ ] P20 探针 + 幂等写 + R2 canonical 探针全绿
      （`crates/partisync-graph/tests/wp02_memory.rs`，随 tests commit）；
- [ ] fmt/clippy/test 三件套全绿；既有 merkle/sync 探针零回归；
- [ ] 提交挂 Task-ID `M9-WP02-T02`；
- [ ] 对抗审查（R1：schema 演进 + 承诺语义）随 PR 评审执行。
