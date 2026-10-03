# Task: M9-WP02-T03 session memory 应用臂 + 双端 bisync 收敛

> **范围外**：MCP 三工具随 T04；CAS exists() 精化随 T05；防篡改端到端
> （直改 SQL content / 快照根 → verify 必败）已由 T02 探针
> `p20c_tamper_detected_and_snapshot_rebuildable` 全覆盖，本卡不重复。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP02-T03 |
| **类型** | 实装（sync 应用臂 + 收敛探针；R1 全审 + 对抗审查） |
| **优先级** | P0（M9 主线 α 第二实施件） |
| **范围** | SPEC M9-WP02 §2.5 + §3 收敛/性能两条验收 + §5 清单中 sync 侧文件（capture/session）+ graph store 应用臂 + 本卡 |
| **创建日期** | 2026-10-04 |
| **来源** | SPEC M9-WP02 批准（PR #122）+ T02 收官（main=`f6e93a4`） |

## 交付物

1. **store.rs `apply_remote_memory`**：远端 memory upsert 应用臂——HLC LWW
   （行 `hlc` ≥ 来键即落选，沿 `apply_remote_tag` 判例）；应用成功即
   `refresh_memory_root`（叶含 created_ns/origin_device，LWW 胜者行双端
   一致 ⇒ 根收敛，leaf 不含 hlc/deleted 不受应用侧簿记影响）。
2. **capture.rs `record_memory_upsert`**：共享域捕获（domain=1，
   entity="memory"，payload 读自落笔后的行）——oplog 键即 LWW 水位，
   随即经 `apply_remote_memory` 回填行上（沿 `record_tag_upsert` 判例）。
   `memory_write` 单入口已自带 oplog，本函数为通用捕获臂（重捕获 /
   workflow 显式路径复用）。
3. **session.rs**：`("memory", "upsert")` 应用臂（payload 解包 →
   `apply_remote_memory` → `count_and_relay`，LWW 落选不转发）；
   bisync 零新增同步代码路径（SPEC §2.5 设计目标）。
4. **收敛探针**（`crates/partisync-sync/tests/wp02_memory.rs`，P6/P19
   判例延伸；bisync 在 sync crate，SPEC §5 的 graph tests 文件清单由
   本卡补记此偏差）：双端各自 `memory_write` → bisync 不动点 → 双端
   `memory_rows` 逐行一致且 root 相等；同 id 双端独立写（created_ns /
   origin 簿记异）收敛一行、根一致。
5. **根重算性能记录**：10⁴ 叶 `refresh_memory_root` 全量重算记值入本卡
   （本机实测，非 CI 门——沿 WP01 bench 判例）。

## 验收

- [ ] 收敛探针全绿（不动点 + 根相等 + 簿记异收敛一行）；
- [ ] 10⁴ 叶根重算记值：**175 ms**（Apple M 系列 debug profile，2026-10-04 实测；≤1s 口径达标）；
- [ ] fmt/clippy/test 三件套全绿；P6 收敛 / WP01 装配探针零回归；
- [ ] 提交挂 Task-ID `M9-WP02-T03`；
- [ ] 对抗审查（R1：同步语义 + LWW）随 PR 评审执行。
