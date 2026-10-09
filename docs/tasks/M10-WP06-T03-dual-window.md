# Task: M10-WP06-T03 双窗口登记落档

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP06-T03 |
| **类型** | 发布工程登记件（docs-only，零代码 diff） |
| **范围** | docs/release/RELEASE-CHECKLIST-beta.md（B1 注记）+ docs/release/EXT-SIGNING.md（示例扩展签名窗口节）+ docs/reports/M9-report.md（债表两行注记）+ 本卡 + SPEC §3 T03 行勾选 |
| **创建日期** | 2026-10-08 |
| **来源** | SPEC docs/specs/M10-WP06.md §2.2 + M9-WP06 §2.2-6（drop-caches 窗口原案）+ M8-WP07-bench §2/§3（非阻塞依据）+ M9-WP06-T03 报告 §3-2（改挂 M10-WP06 判定）+ M9-WP04 §6-R5（示例扩展断签窗口）+ docs/release/key-custody.md（密钥纪律） |

## 摸底输入（实测 2026-10-08，本会话实证）

- `examples/extensions/` 现存 `demo_ext.json`（53B）+ `demo_ext.wasm`
  （111914B），**无 `.minisig`**（`ls -la` 实测；随 M7-WP01-T04 PR
  #36 入仓后未再动，`git log -1` = 40872f0）；
- RELEASE-CHECKLIST-beta.md B1 在位（:31，M9 beta checklist 遗留未
  勾项——beta 发布实际未跑，T03 报告 §3-2 改挂 M10-WP06）；
- M9-report §5 债表两行在位：「示例扩展断签窗口」「drop-caches 冷
  缓存复测窗口」（:126/:127，均 ⏳）。

## 交付物

1. **drop-caches 复测窗口注记**：checklist B1 增 M10-WP06 窗口指针
   + 非阻塞依据一行（读路径自 M8 未改 + 热缓存无算法性劣化
   M8-WP07-bench §2/§3 + M8 冷基线 1567 MiB/s 维持，M9-WP06-T03
   报告 §3-2）；M9-report 债表行同步「⏳ WP06-T03 → 挂 M10-WP06
   窗口」；执行全部条件触发（Linux 台架可得），本任务不执行复测。
2. **示例扩展生产钥签名窗口节**：EXT-SIGNING.md 增「示例扩展签名
   窗口」节——对象 = `examples/extensions/demo_ext.*`、闭合动作 =
   持有人生产钥签名（既有命令链）、key-custody 引用、窗口未闭合 =
   强制验签下装载拒绝维持（P21 不变，非缺陷回归）；M9-report「示
   例扩展断签窗口」行注记；执行条件触发（密钥持有人动作），本任务
   不代办签名。
3. SPEC §3.0 T03 行 + §3.1 两条勾选。

## 验收

- [ ] checklist B1 含 M10-WP06 窗口指针 + 非阻塞依据一行；
- [ ] EXT-SIGNING.md「示例扩展签名窗口」节四要素齐备（对象/持有人
      动作/key-custody 引用/未闭合行为维持）；
- [ ] M9-report 债表两行注记落位；
- [ ] 窗口执行零发生（诚实登记：复测与签名均条件触发未执行）；
- [ ] 零代码 diff；提交挂 Task-ID `M10-WP06-T03`；三门禁绿。

## 收尾补充（2026-10-09，窗口实际执行）

「窗口执行零发生」登记于当日被兑付突破：drop-caches 复测窗口在
Kubuntu 台架（迁移后）触发执行——容器口径 release 重编 partifuse
（宿主 rustup 1.94.0 bind-mount + 离线 registry 缓存）+ 特权容器
drop_caches + tmpfs/overlayfs 双口径读测。结果：**透传侧无劣化实证
成立**（tmpfs 对齐口径四指标 ≥ M8 基线量级）；**by-hash 侧 B1 断言
按字面 FAIL**（open 整载语义，M8-WP07-T04 设计使然非回归），处置
三选待拍板。证据:docs/reviews/M10-WP06-T03-fuse-cold-retest.md
（含脚本归档）+ M9-report:127 行回填 + RELEASE-CHECKLIST-beta.md
B1 执行记录。示例扩展生产钥签名窗口仍未闭合（持有人动作不变）。
