# Task: M10-WP03-T04 WP03 关账盘点（文档关账件）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP03-T04 |
| **类型** | 文档关账（R0——纯 docs，零代码触面；沿 M10-WP04-T06 / M9-WP03-T06 关账判例） |
| **范围** | docs/specs/M10-WP03.md（§3 末两行勾选 + 状态批准落记 + 修订登记 0.6）+ docs/specs/M10-WP00.md（§1 WP03 行收官回填）+ 本卡 |
| **创建日期** | 2026-10-08 |
| **来源** | WP03 实施件 T01–T03 全合入（PR #168 SPEC / #169 T01 / #170 T02 / #171 T03，均 2026-10-06）后的关账动作 |

## 盘点动作

1. SPEC §3 末两行未勾项对照实际交付勾选：
   - GUI 截图行：三任务帧全数在库核实（T01 10 帧 = 三态 + 还原态 +
     全 7 tab 巡检；T02 6 帧；T03 8 帧）——该行 v0.5 时只差勾选位，
     证据注记本已逐任务 ✅；
   - 三件套行：2026-10-08 本机复跑（main=`bfded23`）`cargo fmt --all
     --check` 绿 / `cargo clippy --workspace --all-targets -- -D
     warnings` 0 warning / `cargo test --workspace` exit 0 全套件 ok；
     实施 PR #169/#170/#171 CI 全绿经 `gh pr checks` 实查（fmt/clippy/
     test×2/deny/interop/task-ids 全 pass）。
2. SPEC 状态落记批准（v0.1 = PR #168 合入 2026-10-06，合入 = 批准
   沿 M8/M9-WP00 判例）+ 修订登记 0.6。
3. M10-WP00 §1 WP03 行收官回填；巡检新债 D5（sidecar 持 tantivy
   bm25 写锁 → 桌面检索面 LockBusy，T01 巡检发现）维持
   M10-WP03 §6-D5 处置（另立任务候选，不在本 WP、不顺手修）。

## 验收

- [x] 勾选与交付证据一一对应（本 PR diff 即证）；无未交付勾项
      （§3 六行全部有实名探针/截图帧/PR 号注记，T01–T03 交付齐备）；
- [x] fmt/clippy/test 三门禁本机全绿（2026-10-08 实跑，纯文档 diff
      零 Rust 触面仍跑全量以保「全程」行证据新鲜）；
- [x] 提交挂 Task-ID `M10-WP03-T04`。
