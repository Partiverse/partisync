# Task: xtask trace squash merge 兼容修复（M7-WP00-T02）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M7-WP00-T02 |
| **类型** | 工具修复（R0：纯内部、有测试） |
| **优先级** | P1（M6-report §5.3 债务 #2，偿还窗口 = M7 开局） |
| **范围** | `xtask/src/main.rs` + 本任务卡 |
| **创建日期** | 2026-09-29 |
| **来源** | M6-report §7 抽查审计发现（§5.3-2 债务登记）；SPEC M7-WP00 §3 T02 |

## 根因

`trailer()`（xtask/src/main.rs）对 body 的 `Task-ID:` trailer 用
`find_map` **单值提取（取最后一条）**。GitHub squash merge 在 body 逐条
保留全部原始 commit 的 trailer，squash commit 挂接的多个任务中除末位外
全部失明——`cargo xtask trace M6-WP04-T03` / `trace M6-WP02-T01` 返回空
（M6-report §7 两处 ⚠️ 的直接成因）。追溯链本身未断（body trailer +
PR 工件完整），属工具兼容债。

注：M6-report §5.3-2 处置方向表述为「仅匹配 subject `[Task-ID]` 括号」，
与实现不符——实现解析的是 body trailer 但只取末条；subject 括号形态
（早期提交）反而完全未解析。本修复一并覆盖两种形态。

## 修复

1. `CommitRecord.task_id: Option<String>` → `task_ids: Vec<String>`，
   全量提取 body 每行 `Task-ID:` trailer（`trim` 后匹配，容忍缩进），
   去重保序；body 无 trailer 时回退 subject 的 `[M…-…-T…]` 括号
2. `trace` / `report` 全部使用点改多值语义（contains / any）；
   `Spec` / `AI-Assist` / `AI-Review` / `Reviewed-By` 维持单值 trailer
   语义不变
3. 单测 ×2 新增：squash 多 trailer（fd71169 真实形态）、subject-only
   fallback + 缩进 trailer 去重；既有 5 测试随签名迁移

## 验收

- [x] `cargo test -p xtask` 7/7 绿
- [x] `cargo clippy -p xtask --all-targets -- -D warnings` 零警告
- [x] 实测：`trace M6-WP04-T03` 命中 2 commits（`fd71169`/`9aa45bc`
      squash）、`trace M6-WP02-T01` 命中 1 commit（`01dcd542` squash）——
      修复前均为空
- [x] `report M6` 口径变化如实登记：commit 去重计数 47/21（工具窗口
      含 `f4f58e0` 关账 commit，故任务数 20+WP99=21；M6-report §6
      手工口径 54/20 按「提交×任务」对计且窗口止于 `9aa45bc`——两者
      语义不同，历史工件不改，M7-report 起以工具口径为准）

## 范围外

- `report` 统计口径与 M6-report §6 手工数字的对齐说明（本卡 §验收
  第 4 条已登记，不做代码层调和）
- `scanner_enobufs` hook advisory（M7-WP00 §4 披露性挂账）
