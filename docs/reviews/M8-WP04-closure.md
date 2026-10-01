# M8-WP04 收官与核销报告（滚动）

> SPEC: docs/specs/M8-WP04.md（2026-10-01 批准，PR #61）·
> 本文件随 T01→T03 滚动回填；终版 = RFC §8 核销表 + 基准对照。

## 1. 单节点回归基线盘点表（T01，2026-10-01）

> 盘点口径：crates/partisync-hub 两个门面（直连 `Hub` = wp01 镜像
> `HubService` = wp03）的 pub API × 现有测试覆盖 × 语义断言证据。
> 盘点方法：测试函数签名/断言抽样核对（非整读）。

### 1.1 API × 覆盖矩阵

| API（lib.rs） | 覆盖测试 | 语义断言 |
|---|---|---|
| `open` / `open_with_threshold` | wp01 t04/t05 全部；wp03 几乎全部（threshold=300/50）；m5_wp02 t06 / m5_wp06 t02（registry 重开） | ✅ |
| `put_entry` | wp01 t04/t05 全部；wp03 t02 全部 | ✅ |
| `get_entry`（读时修复+墓碑） | wp01 `t04_projection_consistency_and_tombstone` / `t05_read_repair_on_get` / 随机对拍；wp03 `t02_put_get_remove_tombstone…` / `t02_read_repair_on_get…` | ✅ 最充分 |
| `remove_entry`（投影清除） | wp01 t04 L219-227 / 随机对拍 L758；wp03 t02 L95-99 / L337 | ✅ |
| `rename_entry`（跨目录/O1） | wp01 `t05_rename_dir_o1_no_descendant_rewrites` / `t05_rename_move_between_dirs_root_and_errors`；wp03 `t02_rename_o1_and_errors…` / `t02_subtree…` | ✅（占用名覆盖面见 1.2-1） |
| `list_children`（分页/幽灵清理/CAP） | wp01 `t04_list_pagination_completeness_10k`（10k 行 + 序界）/ `t05_read_repair_on_list_ghosts` / `t05_read_repair_cap_256`；wp03 镜像 + t06 e2e | ✅（极端 limit 见 1.2-4） |
| `subtree`（BFS/环终止） | wp01 `t05_subtree_traversal_and_cycle_guard`（精确 BFS 序）；wp03 镜像 | ✅ |
| `persist` | wp03 L102 裸调用一次（**从未绑定重开断言**） | ⚠️ → 本轮补缺 |
| `split`（小阈值多轮） | wp01 `t04_split_property_small_threshold_multi_round`（300）+ t05 crash×4；wp03 `t07_split_in_raft_apply_multi_round_m1m2`（50）+ `t07_m3_kill_during_split_meta_commit_recover` | ✅ |

未触 entry 数据面的测试文件：m5_wp01（upload-ack）、iroh_e2e（通道）、
m5_wp02/m5_wp06（仅 `HubService::open` 路由/registry 用途）。

### 1.2 无直接断言缺口（T01 补缺）

| # | 缺口 | 现状行为（盘点核实） | 补缺测试 |
|---|---|---|---|
| 1 | rename 到已占用名 | `rename_entry_impl` `put_child` 静默改写目标槽位，不报错；原持有者权威行完好但隐身（读时修复不抢异主槽） | `wp04_baseline::baseline_rename_to_occupied_name_overwrites_slot_silently` |
| 2 | 同目录命名唯一性违约（同槽异主） | 不报错；投影以最后写者为准（lib.rs Hub doc 判例）；违约状态稳定不抖动 | `wp04_baseline::baseline_same_name_violation_projection_is_last_writer` |
| 3 | `Hub::persist()` 显式语义 | 显式刷盘 + 重开恢复（含墓碑跨重开存活）从未绑定断言；重开测试全依赖引擎默认耐久性 | `wp04_baseline::baseline_persist_reopen_roundtrip` |
| 4 | `list_children` 极端 limit | limit=1 逐页走完 = 有序全量（keyset 分页下界） | `wp04_baseline::baseline_list_children_limit_one_walk_complete` |

四项均为**现状钉扎**（characterization）：不改产品代码语义，把未定义
面固化为红灯基准。T02/T03 若改变其中行为，须在本文件留修订注记后
再改断言（不静默放宽）。

### 1.3 基线冻结声明（T02/T03 红灯基准）

- 冻结面：直连 `Hub` 九项 API 行为 + `wp03` 镜像（raft 单节点组）全量
  断言 + `wp04_baseline` 四项钉扎；
- 冻结命令：`cargo test -p partisync-hub`（2026-10-01 全绿，T01 PR CI）；
- 冻结承诺：T02（NodeConfig 构造）与 T03（apply 业务化/幂等/ReadIndex）
  合入时本基线**零红灯**；`Hub::open` 单节点降级路径行为契约不变。

## 2. RFC §8 移交项核销表（T03 回填，待实施）

（占位——五项逐项核销 + 证据链接随 T03 交付回填。）

## 3. 基准对照（T03 回填，待实施）

（占位——RouteQuery P99 / apply 吞吐对 M5-WP02/04 口径。）
