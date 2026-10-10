# Task: M11-WP01-T03 CHANGELOG rc 节起草 + release.yml rc 口径修订

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP01-T03 |
| **类型** | 发布工程(CI workflow 修订 + CHANGELOG 起草;AI 起草人终审判例) |
| **来源** | cadence §3.2-2 + M10-WP06 §4(L178) |
| **创建日期** | 2026-10-10 |

## 交付

1. **CHANGELOG [0.2.0-rc.1] 节**(AI 起草,人终审后随 Release publish):
   Added 六主题(MCP 2.0 远程化/记忆层二期/桌面可用性三期/partisd 常驻
   守护进程/FUSE 写事件/扩展签名工具链)+ Changed(bump 0.2.0/产物名/
   五 bin/**桌面 GUI 维护冻结注记 §4-2b**)+ Known limitations 八条
   (mock AS 边界/编辑删除 UI 归 Partiverse/向量检索准备态/碎片流债/
   reload 缺口/公证/演示面/冻结注记)
2. **release.yml rc 口径修订**:头注释 + 产物名 6 处 `-beta`→`-rc.1`
   + linux/macOS 包新增 `partisync-mcp-http`(**五 bin 口径**)+
   **打包前 strip**(改进项 a,先 strip 后 tar)+ repro 双构建加
   `CARGO_PROFILE_RELEASE_DEBUG=false`(改进项 b 评估,偏差登记不假绿)

## 涉及文件清单(Iron Rule 9)

docs/release/CHANGELOG.md · .github/workflows/release.yml ·
docs/tasks/M11-WP01-T03-changelog-release-yml.md

## 验收

- [x] CHANGELOG rc 节按 beta 判例结构起草(Added/Known limitations;
      桌面冻结注记随发 §4-2b)
- [x] release.yml:五 bin(strip 前置)+ repro DEBUG=false 评估位 +
      产物名 6 处 rc.1;零 0.1.0-beta 残留
- [x] 零产品代码 diff(release.yml/CHANGELOG/任务卡);提交挂 Task-ID;
      三门禁绿(CI 复核)
