# Task: M10-WP04-T01 记忆 tombstone 引擎面（delete/update/复活 + 同步臂）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP04-T01 |
| **类型** | 实装（graph+sync 引擎；R1 全审——公共承诺语义 + 同步新面） |
| **范围** | SPEC M10-WP04 §2.1 + §3 T01 探针 + store.rs/memory.rs/capture.rs/session.rs + ADR-0029 修订 + properties.md P20 注记 + 本卡 |
| **创建日期** | 2026-10-06 |
| **来源** | M10-WP00 §1-WP04（tombstone 先行拍板）+ ADR-0029 重新评估条件触发 |

## 交付物

1. `Store::memory_delete`：deleted=1 + hlc 推进 + 同事务 oplog
   `("memory","delete")` + 根快照刷新（根值不变——叶不含 deleted，
   memory.rs:113）；`Store::memory_update`：同事务墓碑旧 id + 写新行
   （canonical 等价 no-op；墓碑/不存在 id 显式 Err）。
2. `memory_write` 复活语义：命中墓碑行 → deleted=0 + hlc 推进 + 根
   刷新，返回 deduplicated=false（store.rs:2100 幂等路径仅活行）。
3. session.rs `("memory","delete")` 应用臂（LWW：delete hlc > 行 hlc
   才落位；沿 `("tag","remove")` 判例 session.rs:215-221）+ capture.rs
   `record_memory_delete`。
4. `memory_verify(memory_id)` 响应增 `deleted`；全检报告增
   `tombstones` 计数。ADR-0029 修订登记（墓碑留承诺集/软删不动根/GC
   动根）+ properties.md P20-a 措辞注记（承诺集=含墓碑全集）。

## 探针

wp04_memory.rs（wp02_memory.rs 判例）+ sync 既有收敛判例文件：删除
三路径不见/proof 仍过/根值不变；update 新旧 id + no-op + 墓碑报错；
复活 + 活行幂等不回退；双端 delete bisync 不动点（rows 全等 + 根相
等）+ 晚到 upsert 被拒。

## 验收

- [ ] §3 T01 四组探针绿（先红后绿）；
- [ ] fmt/clippy/test 三件套绿；零新增顶层依赖；
- [ ] 提交挂 Task-ID `M10-WP04-T01`，Spec trailer 指本 SPEC。
