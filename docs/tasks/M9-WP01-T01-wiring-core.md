# Task: M9-WP01-T01 装配层实现——挂载写进 sync 管线（含 P19 登记）

> **范围外**：e2e 探针与 SEMANTICS/ADR 修订随 T02；F1 sync_stats 派生
> 随 T03（desktop 面 + GUI 实操验证）。本卡不改既有探针断言。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP01-T01 |
| **类型** | 实装（装配层核心；R1 全审 + 对抗审查） |
| **优先级** | P0（M9 主线 β 首件） |
| **范围** | SPEC M9-WP01 §5 清单的 fuse 事件面 / gateway 装配层 / bin / graph store 视图 / P19 登记 / 本卡；**零外部新依赖**（workspace tokio 增 `time` feature 沿 M6-WP03-T05 判例；gateway 增 sync/graph path-only 沿 ADR-0024 修订 5 判例） |
| **创建日期** | 2026-10-03 |
| **来源** | SPEC M9-WP01 批准（PR #115，main=`e5cc26e`）+ 摸底实测（wiring_e2e 落锤 Q3；session::push/bisync 零生产调用方） |

## 交付物

1. **fuse 事件面**：`crates/partisync-fuse/src/events.rs`（`FuseWriteEvent`
   Upsert/Remove/Rename + `EventSink`）+ fs.rs 五个 apply-成功钩子位
   （`apply_overlay_replace`/setattr truncate/create/unlink/rmdir/rename；
   mkdir 本为 EPERM 拒绝面无事件）+ `PartiFuse::with_wiring` 装配构造
   （缺省构造行为与 M8 全等——无 `--graph` 零回归）。
2. **gateway 装配层**：`crates/partisync-gateway/src/wiring.rs`——
   `WiringSession::init`（同根校验：graph 卷指纹 ≠ backing 规范路径 →
   拒绝启动，SPEC §2.1；空库播种设备行 + 卷指纹）+ `serve` 事件循环
   （折叠 → GraphApplier → capture；Rename 折叠 Removed+Created 带
   size——T05 判例）+ bisync tick（默认 5s，MissedTickBehavior::Skip）。
3. **bin**：gateway `partisync-fuse` 增 `--graph/--peer/--tick-ms`；
   init 先于挂载（block_on 校验），失败 fail-fast。
4. **graph store**：`volume_fingerprints()` 视图（同根校验用）。
5. **P19 登记**：`docs/tests/properties.md`（登记先于 T02 测试代码）。

## 验收

- [ ] fmt/clippy/test 三件套全绿；无 `--graph` 时 M8 探针面零改动
      （probe_mount/wiring_e2e 现有段原样，P15）；
- [ ] P19 行落 properties.md；
- [ ] 提交挂 Task-ID `M9-WP01-T01`；
- [ ] 对抗审查（R1：触碰 sync 装配核心面）随 PR 评审执行。
