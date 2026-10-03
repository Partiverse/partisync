# M8-WP07 开放问题落锤记录（SPEC §7）

> Task-ID: M8-WP07-T02（拆卡）· 日期: 2026-10-03 · SPEC §7 授权
> 「实施前须落锤，实施任务卡拍板」· 执行: GLM-5.3-Flash (ZCode)

## Q1（R3）并发写冲突语义

**拍板：后写者 EBUSY**（SPEC §6-R3 候选采纳）。

- 同一路径已有未 release 的写回暂存（overlay 暂存持有中）→ 第二个
  写打开返回 `EBUSY`，不排队不覆盖不分叉；
- 理由：整文件替换语义下「后写覆盖」会静默丢先写者数据（违背
  「拒绝先于破坏性效果」P15 延续）；版本分叉引入合并语义（二期不做，
  非目标表未列）；EBUSY 是 mountpoint-s3 同判例、可预期可探测；
- 实现落点：overlay 暂存以路径为键登记 holder（`Mutex<HashSet<PathBuf>>`
  足够——单挂载点单进程内），open(write) 时查重，release 时释放。

## Q2 写回日志持久化位置

**拍板：backing 旁 `.partisync-writeback/` 目录**（backing root 下）。

- 形态：单文件 append-only JSONL（`wal.jsonl`）+ 重放游标——crash 后
  无需打开 CAS 即可重放（SPEC §2.3 幂等重放的最短依赖路径）；
- 与 GC 交互：日志项 payload 只含路径与操作（整文件替换的临时 blob
  落在同目录 `staging/`，不进 CAS chunk 库）→ **claim_due 协议零感知**
  （不引用 CAS 对象，无 GC 交互面）；staging 清理由应用成功后同步删除；
- 索引排除：`.partisync-writeback/` 目录在 readdir 返回中隐藏 +
  索引接线入口按前缀过滤（T05 接线时实现，探针断言）；
- 放大控制：append-only 无 fsync 放大设计（R4）——应用成功后 truncate
  已应用前缀（日志保持小）。

## Q3 sync/graph 接线入口收敛（§5 二选一）

**拍板：partisync-sync 侧**（SPEC §5 清单收敛到 `crates/partisync-sync/**`）。

- 复用 M5-WP08 既有 `EventApplier` trait + `GraphApplier`（Store 后端，
  Created/Modified = add_entry 幂等 upsert + 父目录链自动建，Removed =
  remove_entry）——挂载面写构造 `EventRecord` 走同一应用链，Merkle 重算
  随 graph apply 链自动发生；
- 依赖方向合规：partisync-fuse 已在能力层（gateway 组装），fuse → sync
  为同层引用（沿 partisync-sync 内部 event.rs 判例）；gateway 装配时把
  `GraphApplier` 注入 fuse 写回器（EventOpts.applier 同款注入模式）；
- e2e 断言（SPEC §3）：挂载写 → `partisync find` 命中 + Merkle 根与
  CLI 直写一致。

## 任务卡拆分（T02–T05）

| 卡 | 内容 | 关键验收 |
|---|---|---|
| T02 | P16 转正（先注册后写测试）+ 写回日志模块（JSONL append + 重放幂等 proptest）+ unlink/rmdir/rename 日志路径 | 重放幂等 ≥1000 例；mkdir/mknod/symlink 维持 EPERM |
| T03 | 整文件替换 overlay 暂存 + truncate 特例 + EBUSY 并发 + crash 矩阵（kill -9 两相位） | backing 无半提交；EBUSY 探针 |
| T04 | `/by-hash` 命名空间（lookup/getattr/read + EROFS + readdir 受限）+ 探针 | 与目录视图并存；读性能不劣于一期 |
| T05 | 索引接线 e2e（EventRecord → GraphApplier）+ SEMANTICS.md 修订 + ADR-0026 修订登记 + bench 报告 + 收尾 | 挂载写 = CLI 写同根校验；Merkle 根一致 |

依赖链：T02 → T03 → T05；T04 独立可与 T03 并行。
