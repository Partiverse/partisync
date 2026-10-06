# Task: M10-WP04-T03 memory GC（墓碑硬清除 + CLI 子命令）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP04-T03 |
| **类型** | 实装（graph 维护面 + CLI；R0——本地维护动作、有探针） |
| **范围** | SPEC M10-WP04 §2.1 GC 契约 + store.rs + cli/main.rs + graph 探针 + 本卡 |
| **创建日期** | 2026-10-06 |
| **来源** | M10-WP00 §1-WP04「GC」项 + §6-R2 复活边界口径 |

## 交付物

1. `Store::memory_gc(retention) -> GcReport {purged,
   remaining_tombstones}`：清除墓碑行 hlc（delete op 水位）早于截止
   的行（定宽 hex 字典序=时序，store.rs:1238 口径；如需显式列可
   additive `memory.deleted_ns` 随 v18）→ 物理删行 + 根重算（**动根**
   ）+ memory_count 更新；FTS 随 AFTER DELETE 触发器自动清
   （schema.sql:355-357）。GC 不产 oplog（墓碑 op 已传播；§6-R2 复活
   边界在 --help 诚实登记）。
2. CLI 子命令 `partisync memory-gc [--db <path>] [--retention-days
   N]`（默认 30 天；main.rs:50 手工分派判例 + :55 用法文本）。

## 探针

过保留期墓碑清除（行数减/根变/verify ok/检索不见）；保留期内墓碑与
活行不动；空库/无墓碑幂等；GC 后 P20 全套语义（重算=快照）不回退。

## 验收

- [ ] §3 T03 探针绿；
- [ ] fmt/clippy/test 三件套绿；零新增顶层依赖；
- [ ] 提交挂 Task-ID `M10-WP04-T03`。
