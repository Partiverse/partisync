# Task: M9-WP01-T02 P19 装配探针 + SEMANTICS/ADR 修订

> **范围外**：容器真挂载 `--graph` 端到端探针（SPEC §3 首条，Linux
> 容器门控）随 T03；F1 sync_stats watermark 派生（desktop 面 + GUI
> 实操）随 T04。本卡不触碰 fuse 钩子位与装配层实现（T01 已合入）。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP01-T02 |
| **类型** | 测试 + 语义文档（P19 探针落地 + SEMANTICS 修订 + ADR-0026 修订登记 + SPEC 0.2） |
| **优先级** | P0（WP01 闸门件：P19 探针先于实施合入后的行为锁定） |
| **范围** | `crates/partisync-gateway/tests/wp01_wiring.rs` + `crates/partisync-fuse/SEMANTICS.md` + `docs/adr/0026-fuse-posix-gateway.md`（修订 0.5）+ `docs/specs/M9-WP01.md`（0.1→0.2 措辞修正）+ 本卡 |
| **创建日期** | 2026-10-03 |
| **来源** | SPEC M9-WP01 §2.4/§3（P19 登记随 T01 PR #116）；判例 wiring_e2e（M8-WP07-T05） |

## 交付物

1. **P19 探针 4 例**（`tests/wp01_wiring.rs`，channel 注入、不依赖真实
   挂载）：[P19-a] 真实 `WiringSession::serve` 链折叠 vs CLI 直写同库
   同根 + push 收敛不动点；[P19-b] origin 剪枝（回推
   `skipped_self_origin`）；[P19-c] 重复投递幂等（entry 数稳定）；
   同根校验拒绝异卷装配。
   **新判例**：`add_entry` 对播种了设备的库把 owner=None 盖成本机
   device（owner 进叶哈希）——直写对照侧必须播种同款 device。
2. **SEMANTICS §同步接线节**：事件源=apply 成功点（R5）、折叠契约、
   crash 丢失窗口（对账兜底、风险接受）、事件过期降级、同根校验、
   装配层故障不阻塞挂载面。
3. **ADR-0026 修订 0.5**（前置条件 3 延伸：写路径接线）。
4. **SPEC M9-WP01 0.1→0.2**：P19-a 判据措辞随 P19 登记同步修正。

## 验收

- [x] P19 探针 4/4 绿（本地）；gateway 全量回归绿；
- [x] fmt/clippy 零警告；
- [ ] 提交挂 Task-ID `M9-WP01-T02`，CI 绿合入。
