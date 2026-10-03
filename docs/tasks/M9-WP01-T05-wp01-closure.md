# Task: M9-WP01-T05 WP01 关账盘点（SPEC §3 全勾 + 状态回填）

> **范围外**：WP02（记忆层）起各 WP 独立会话；本卡不改代码（p11 探针
> 为 §3 冲突条目的验收缺口补齐，随本卡入仓）。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP01-T05 |
| **类型** | 关账盘点工件（P11 探针补齐 + SPEC 0.3 + WP00 状态回填） |
| **优先级** | P0（WP01 关账件） |
| **范围** | `crates/partisync-gateway/tests/wp01_wiring.rs`（p11 探针）+ `docs/specs/M9-WP01.md`（0.2→0.3）+ `docs/specs/M9-WP00.md`（§1 WP01 状态）+ 本卡 |
| **创建日期** | 2026-10-03 |
| **来源** | SPEC M9-WP01 §3 冲突条目验收缺口（T01–T04 未覆盖双端并发写） |

## 交付物

1. **P11 探针**（`p11_conflict_both_addressable_via_wiring`）：装配层写
   路径被远端异属主直写并发创建 → push 后 base + conflict 后缀两行皆可
   寻址 + 血缘落档——装配层走同一 `apply_remote_entry` 路径，零新增冲突
   代码。**判例追加**：peer 库须自有 device（owner 盖章 ⇒ 同 device 播种
   会吞掉冲突分支）。
2. **SPEC M9-WP01 0.3**：§3 八条全勾（逐条标注承载任务/PR + 交付口径
   修订注记——tick 驱动交付由对照测试覆盖、真挂载探针以手动 push 驱动）。
3. **M9-WP00 §1**：WP01 状态 ✅ 收官回填（T01–T05，PR #116–#120）。

## 验收

- [x] p11 探针绿（wp01_wiring 6/6）；
- [x] SPEC §3 无未勾条目；
- [ ] CI 绿合入 → **M9-WP01 关账**，M9 进度 WP00+WP01 ✅，下一 WP02。
