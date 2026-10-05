# Task: M9-WP06-T02 release.yml beta 修订（A5）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP06-T02 |
| **类型** | 发布工程（CI workflow；R1） |
| **范围** | .github/workflows/release.yml + 本卡（SPEC M9-WP06 checklist A5） |
| **创建日期** | 2026-10-05 |
| **来源** | 用户确认 SPEC §2.1 三轴口径（2026-10-05，「WP06 发布执行确认」） |

## 交付物

1. tar 包扩 bin：linux = cli+mcp+fuse+hub-demo-web；macOS = cli+mcp
   （partisync-fuse 登记 linux-only——fuser 需 osxfuse 系统件、runner 构建
   必败；macOS 挂载路径作为已知限制写入 release notes）。
2. 产物名 -beta 后缀（tar + dmg 重命名）。
3. CycloneDX SBOM 独立导出 job（cargo-cyclonedx --all --format json）。
4. reproducibility 双构建对账 job（M8-WP02 R1 判例：偏差登记不假绿，
   结果文件进 Release 产物）。
5. release job 汇总五产物源 + SBOM/reproducibility 签名入列。

## 偏差登记

- macOS 产物面缩窄（无 fuse bin）：SPEC §2.1 产物面轴偏差，已知限制随
  release notes；linux tar 保持四 bin 全量。
- B1 drop-caches 冷缓存复测：交互式手动档台架（M8-WP01 §1 判例）本窗口
  无法安全自动化——沿 SPEC §6-R3 预登记处理，发布照常，B1 挂人工窗口
  （读路径自 M8 未改 + 热缓存无回归 + M8 冷基线维持，非发布阻塞）。

## 验收

- [x] workflow 修订完成，yaml 结构与判例步齐备（CI 由 tag 触发实跑验证）；
- [x] 受影响 crate clippy 零告警 + fmt 绿；
- [x] 提交挂 Task-ID `M9-WP06-T02`。
