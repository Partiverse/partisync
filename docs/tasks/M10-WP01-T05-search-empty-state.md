# Task: M10-WP01-T05 检索空态引导（index_stats IPC + 分支文案）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP01-T05 |
| **类型** | 功能补全（M10-WP00 §1-WP01「UX 深化」之三） |
| **范围** | desktop src/ipc.rs + src/lib.rs + ui/app-core-v3.js + ui/index.html + tests/commands.rs + tests/ui_hardening.rs + 本卡 |
| **创建日期** | 2026-10-05 |
| **SPEC** | docs/specs/M10-WP01.md §2.5 / §3 |

## 根因

空态不区分「索引还没建」与「真无命中」（app-core-v3.js:226-228 统一
渲染「换个更短的关键词」）——索引目录为空骨架时该文案误导，正确动作
是跑 `partisync reindex`（M9-WP03-T06）。UI 现无任何索引规模可见面：
`Bm25Index::approx_count()`（bm25.rs:442）零调用方。

## 修复

1. 新 IPC `index_stats` → `IndexStats { docs: u64 }`：`state.index()`
   懒加载后取 approx_count；打开失败 → `DesktopError::Index`；
   `lib.rs:97` generate_handler 注册；
2. 检索 tab 三分支：初始态显示「全文索引 N docs」徽标（一次拉取缓存
   不轮询）；索引空（docs==0 零命中）→ 「全文索引还没有建立——运行
   `partisync reindex`」引导文案；有索引无命中 → 维持既有建议文案。
   approx_count 为近似值，仅做 0/>0 分支 + 规模徽标（SPEC §6-R5）。

## 验收

- [ ] e2e `index_stats`：空索引 docs=0；种子 IndexedDoc 后 >0；失败
      → `kind:"Index"`；
- [ ] 静态探针：三分支文案/徽标接线断言；
- [ ] GUI 实操截图 `docs/screenshots/M10-WP01-T05-*.png`（索引空引导
      态；有索引态入镜更佳）；
- [ ] fmt/clippy/test 绿；零新增顶层依赖。
