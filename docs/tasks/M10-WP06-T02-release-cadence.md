# Task: M10-WP06-T02 rc/0.2.0 口径拍板文档

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP06-T02 |
| **类型** | 发布工程拍板件（docs-only，零代码 diff） |
| **范围** | docs/reviews/M10-WP06-release-cadence.md + 本卡 + SPEC §3 T02 行勾选 |
| **创建日期** | 2026-10-08 |
| **来源** | SPEC docs/specs/M10-WP06.md §2.1 + M9-WP06 §2.1（轴 + 推荐案 + 依据判例）+ docs/reviews/M9-WP06-T03-release-report.md（§2-B7/§3-1/§3-3/§3-4 改进项来源）+ M10-roadmap-proposal §5 M10-WP05 行（「v0.1.0-rc / 0.2.0 增面版」二选一原案） |

## 摸底输入（实测 2026-10-08，本会话实证）

- `grep -rn 'version = "0.1.0"' crates/*/Cargo.toml Cargo.toml` = **49 处 /
  14 文件**（13 成员 Cargo.toml + 根）：path-dep 约束 46 处（12 成员）
  + 自身版本字段 3 处（根 workspace.package:23 / fuse:3 /
  fuse-spike:7——后者独立评估 workspace 不入产品依赖图）；
- `crates/partisync-desktop/tauri.conf.json:4` = `"version": "0.1.0",`；
- `grep -n '0.1.0' .github/workflows/release.yml` = 8 行：产物名模板 6 处
  （:40/:45/:66/:71/:91/:96）+ 口径注记注释 2 处（:5–:6）；
- Cargo.lock 已入库（bump 附随 diff）；desktop 无 package.json 版本面；
- gateway `src/bin/partisync-mcp-http.rs` 在位（M10-WP05 交付，入包
  候选 bin）。

## 交付物

`docs/reviews/M10-WP06-release-cadence.md`：三轴推荐案（版本号 /
产物面 / 发布触发节奏）各有着落——推荐 + 依据 + **待用户确认位显
式**；rc 改进项 a)–d) 逐项处置建议；bump 改动面实测清单（49 处计数
细分 + tauri.conf + release.yml 逐项 + Cargo.lock 附随 + hub 约束语义
核查）。**只拍板不实施**：bump / tag / Release 全部留 rc 发布窗口
（SPEC §4 非目标）。

## 验收

- [ ] 三轴各有着落，待用户确认位显式（确认前不进发布执行）；
- [ ] rc 改进项 a)–d) 逐项处置建议非空；
- [ ] bump 改动面为实测清单非估计（命令 + 计数 + 逐文件）；
- [ ] 零代码 diff；提交挂 Task-ID `M10-WP06-T02`；三门禁绿。
