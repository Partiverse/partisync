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

- [x] e2e `index_stats`：空索引 docs=0；种子 IndexedDoc 后 >0；失败
      → `kind:"Index"`——`t05_index_stats_empty_zero_then_seeded_positive`
      / `t05_index_stats_open_failure_maps_to_index_error` 全绿。实测
      注记：`state.index()` 打开失败原生落 `Internal`（`From<
      PartisyError>`），`index_stats` 命令面按 SPEC §2.5 拨回 `Index`；
      search 打开失败落 `Internal` 属既有文档漂移（state.rs/ipc.rs 注释
      vs 行为），文件不在本卡清单，登记遗留不顺手修；
- [x] 静态探针：三分支文案/徽标接线断言——
      `t05_empty_state_reindex_branch_and_no_hit_copy` /
      `t05_index_stats_badge_cached_and_no_polling` 全绿（ui_hardening
      17 探针全过）；
- [x] GUI 实操截图 `docs/screenshots/M10-WP01-T05-*.png`（索引空引导
      态；有索引态入镜更佳）——真实实例（partisd-desktop）实操五态
      归档：empty-idle-badge（索引空初始态「全文索引 0 docs」徽标）/
      empty-reindex-guide（索引空零命中 → `partisync reindex` 引导）/
      indexed-idle-badge（有索引初始态「全文索引 2 docs」徽标）/
      filename-hit-card（量子退火命中 · 验收报告.txt 卡入镜）/
      indexed-nohit-suggest（有索引无命中 → 既有建议文案，与索引空
      分支正确区分）；
- [x] fmt/clippy/test 绿；零新增顶层依赖（fmt ✓ / clippy -D warnings ✓
      / workspace test 107 目标 ok——唯一失败
      `mcp_call_real_sidecar_ext_list` 为 main 既有环境性失败，stash
      验证 clean main 同红，见遗留登记）。

## 遗留登记

- `mcp_call_real_sidecar_ext_list` 在本机为既有失败（demo 扩展 sidecar
  返回空 tools；stash 验证 clean main 同样红）——环境依赖测试，与本卡
  无关，未修。
- search/search_hybrid 打开失败实际 `kind:"Internal"`，与 ipc.rs/state.rs
  注释宣称的 `DesktopError::Index` 漂移——注释/行为归一需动 state.rs
  （不在本卡清单），待后续任务承接。
