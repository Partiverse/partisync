# Task: M9 总纲 + WP01 SPEC 草稿（M9-WP00-T02）

> **范围外**：本任务卡只落档总纲与 SPEC 草稿（纯 docs），不实施任何
> 代码。WP01 实施待本 PR 批准后按 §5 文件清单另立任务卡开工。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP00-T02 |
| **类型** | 里程碑总纲 + 首实施 WP SPEC 草案 |
| **优先级** | P0（M9 开局第二件） |
| **范围** | `docs/specs/M9-WP00.md` + `docs/specs/M9-WP01.md` + 本任务卡 |
| **创建日期** | 2026-10-03 |
| **来源** | M9-roadmap-proposal 批准（PR #114，用户指令「同意 M9 路线图提案，批准 pr 并入」）；先例：M8-WP00-T01（PR #59 判例：总纲 + 条件 WP SPEC 草稿同 PR） |

## 执行方案依据

- 铁律 1「规格先行」：总纲正式化 WP 图（提案 §5 → §1）；WP01 SPEC 按
  partisync-spec-draft 骨架起草，批准前不动代码。
- 铁律 4「测试即规格」：M9-WP01 §3 验收全部可执行判定 + P19 新不变量
  候选显式登记（登记先于测试代码，随实现走 partisync-property-registry）。
- 铁律 7「地基人工」：WP01 触碰 sync 装配核心面 → SPEC 预登记 R1 全审
  + 对抗审查。

## 交付物

1. `docs/specs/M9-WP00.md`：M9 总纲（WP 图 / 拍板回填 / 债务承接 /
   风险 / 非目标）。
2. `docs/specs/M9-WP01.md`：写路径接线 SPEC 草稿（装配层契约 / F1
   清偿 / SEMANTICS 修订 / P19 候选 / e2e 验收 / 文件清单 / 六项风险），
   基于写路径摸底实测锚点（wiring_e2e 落锤 Q3、session/capture 零生产
   调用方、sync_watermark 派生面）。
3. 本任务卡落档 `docs/tasks/`。

## 验收

- 总纲 §2 拍板表覆盖提案 §7 全部 7 项（含 M8-WP00 §2 两项复核义务履行）；
- WP01 SPEC §5 文件清单封闭且与摸底锚点一致；
- PR 保持 OPEN 待批准（合入 = 批准，不自行合并）；
- 提交挂 Task-ID `M9-WP00-T02`；纯 docs 提交。
