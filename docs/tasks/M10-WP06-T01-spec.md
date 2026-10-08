# Task: M10-WP06-T01 SPEC 起草（发布节奏收口）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP06-T01 |
| **类型** | 规格（G0 门槛件，docs-only） |
| **范围** | docs/specs/M10-WP06.md（§1–§7 全节）+ 本卡 |
| **创建日期** | 2026-10-08 |
| **来源** | M10-WP00 §1-WP06（rc/0.2.0 口径拍板 + drop-caches 复测窗口 + 示例扩展生产钥签名窗口）+ §2 拍板#7（联邦/10⁹ 维持 WP06 条件位）+ M9-WP00 §4 债表 drop-caches 行 + M9-report §5 债表两行 + 用户指令（重跑 Mimosa 完整安全扫描，工具不可得/超长则如实登记债务） |

## 摸底输入（起草期实证 2026-10-08）

- 上游：M9-WP06 SPEC §2.1/§2.2-6（beta 口径与 drop-caches 原案）+
  docs/reviews/M9-WP06-T03-release-report.md（B7 REPRODUCIBLE=no /
  strip / macOS fuse / hub demo 四项 rc 改进项来源）+ M8-WP07-bench §2
  + M9-WP04 §6-R5（断签窗口）+ M9-report §5（Mimosa 无新扫描 + 两债行）；
- 实物：`examples/extensions/` = demo_ext.wasm + demo_ext.json，无
  `.minisig`（断签状态实测）；
- 版本面：path-dep `version = "0.1.0"` 约束 49 处（13 成员 Cargo.toml +
  根，grep 实测）+ tauri.conf.json:4 `"version": "0.1.0"`；
- **Mimosa deep 扫描已随起草发起并完成**：scanId
  `scan-2026-10-08T06-53-40.527Z-63a9f1cb4a97`、seal
  `sha256:f9a368e3…7923`、run status inconclusive（动态派发覆盖缺口）、
  227 文件 0 解析失败、唯一 HIGH = xtask/src/main.rs:343 git() 污点链
  （anchor 与 2026-10-02 复扫同源 = 已签收误报行号漂移）——五要素登记
  入 SPEC §2.3，落档挂 T04。

## 交付物

`docs/specs/M10-WP06.md`：任务切分 T02 拍板文档 → T03 双窗口登记 →
T04 Mimosa 登记 → T05 关账；范围裁定：rc 发布执行 / 复测执行 / 持有
人签名三项全部条件触发不占线（SPEC §4）；Mimosa 边界纪律（不构成安
全放行结论）入 §2.3/§6-R3。

## 验收

- [x] SPEC 七节齐备、任务粒度对齐单 PR ≤400 行（全 docs 预期无需堆叠）；
- [x] 零代码 diff（本 PR 仅 docs/specs/M10-WP06.md + 本卡）；
- [x] 提交挂 Task-ID `M10-WP06-T01`。
