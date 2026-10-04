# Task: M9-WP03-T05 默认数据目录对齐（SPEC v0.2 §2.5）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP03-T05 |
| **类型** | 缺陷修复（P0 用户可见；R1 全审） |
| **优先级** | P0（用户实测「桌面壳没有任何功能」的直接根因） |
| **范围** | SPEC M9-WP03 v0.2 §2.5 + §3 T05 验收 + §5 lib.rs + 本卡 |
| **创建日期** | 2026-10-04 |
| **来源** | 用户实测反馈「桌面壳似乎还是没有任何功能」（2026-10-04） |

## 根因（GUI 实操诊断，证据链）

桌面壳默认数据目录 = `~/Library/Application Support/partisync-desktop/`
（独立空库），而生态真数据分散在：gateway graph.db / CLI search index =
`~/Library/Application Support/.partisync/`、用户实际数据集（demo/索引
产物）在仓库工作区。无参启动 → 全 0 统计 + 空浏览 + 空检索 = 用户眼中
「没有任何功能」。UI 本身功能正常（GUI 全功能渲染已实证，见
docs/screenshots/M9-WP03-T05-data-align.png）。

**判例修正**：前会话登记的「窗口退化 210×141@负坐标环境异常」系 CUA
CGWindowList 元数据不可靠（同窗口 screencapture 全幅渲染实证正常）；
「白屏」仅出现在 pkill 非干净重启循环后（WebContent 会话态污染），
干净 `open` 启动恒正常——GUI 阻塞登记解除。

## 交付物

1. `default_data_dir()` → `dirs::data_local_dir()/.partisync`（对齐
   gateway graph.db / CLI index 默认目录）；`--data-dir` 覆盖不变；
   旧目录不迁移不删除（R6，`--data-dir` 可指回）。
2. GUI 验收截图：对齐目录含真实数据集时无参启动，统计非零 + 浏览
   行可见，归档 `docs/screenshots/M9-WP03-T05-data-align.png`。

## 验收

- [x] 无参启动数据目录落点 = `~/Library/Application Support/.partisync/`；
- [x] 真实数据集下 GUI 实测：统计 266 文件可见 + 浏览行渲染（截图）；
- [x] fmt/clippy/test 三件套全绿；零新增依赖；
- [x] 提交挂 Task-ID `M9-WP03-T05`。
