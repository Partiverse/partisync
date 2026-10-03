# Task: M9-WP01-T04 F1 清偿——sync_stats 改 sync_watermark 派生（含 GUI 实操验证）

> **范围外**：容器真挂载 `--graph` e2e 探针随 T03（独立分支/PR）；本卡不
> 触碰 sync/graph 域代码（watermarks()/note_applied 为既有 API，仅消费）。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP01-T04 |
| **类型** | 实装（desktop IPC 统计口径）+ GUI 实操验收（2026-10-02 用户指令硬性规则） |
| **优先级** | P1（WP01 债清偿件；SPEC §2.2 F1 口径） |
| **范围** | `crates/partisync-desktop/src/ipc.rs`（sync_stats）+ `crates/partisync-desktop/tests/commands.rs` + `docs/screenshots/M9-WP01-T04-f1-sync-tab.png` + 本卡 |
| **创建日期** | 2026-10-03 |
| **来源** | SPEC M9-WP01 §2.2 + M8-WP05-ui-report D1/F1 登记；M8-WP05-T03 GUI 判例（运行中实例 sqlite 种子 + 截图） |

## 交付物

1. **ipc.rs sync_stats**：`devices` / `last_sync_ns` 改由持久
   `sync_watermark` 表派生（`Store::watermarks`，push 逐行
   `note_applied` 只增不被 ACK trim）；`last_sync_ns` 从水位 HLC 键
   phys 段（定宽 hex 毫秒，`hlc.rs` `to_key` 契约）换算纳秒。
   `applied`/`skipped_self` 维持 pending 面（「待推送」口径不变）。
2. **tests**：fixture 增水位种子；seeded 断言改精确 phys 换算值；
   新增 **F1 回归测** `t04_f1_sync_stats_survives_ack_trim`（真实
   pending 键全量 trim → applied 清零但 devices/last_sync 不归零）。
3. **GUI 实操验证**（docs/screenshots/M9-WP01-T04-f1-sync-tab.png，
   2880×1800）：运行中实例（本次构建二进制）活库 sqlite 种子远端设备
   水位 → 同步 tab AXPress 切入 → 「已与 1 台设备保持一致 / 最近对账」
   渲染（旧口径 pending 空必显 0 台——二进制与派生链同时验证）；验证后
   种子行已清理。

## 验收

- [x] desktop 单测 17/17 绿（含 F1 回归）；
- [x] GUI 截图归档（同步 tab 三态可见、devices/last_sync 非零）；
- [ ] fmt/clippy 零警告 + CI 绿合入。

## 判例（实施期）

- 同步 tab 的 tab 行在自定义标题条内（窗口 y≈91pt），cliclick 裸坐标打
  y=73 落标题栏拖拽区无效——**AXPress（CUA 元素树）为该 tab 切换唯一
  可靠路径**（M8-WP05-T04 判例再证）；`entire contents` 裸枚举时灵时不
  灵，CUA `elements()` 稳定。
- Tauri 窗口内嵌 tokio Runtime（PartiFuse cas_rt）**不可在 async 上下文
  drop**——探针/测试挂载句柄须归还 blocking 线程（T03 判例，先记于此）。
