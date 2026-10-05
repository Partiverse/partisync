# Task: M10-WP01-T01 检索命中透出真实文件名（「GUI 搜索形同虚设」修复）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP01-T01 |
| **类型** | 缺陷修复（用户指认 P0；M10-WP00 §1-WP01 首件） |
| **范围** | crates/partisync-desktop/src/ipc.rs + ui/app-core-v3.js + tests/commands.rs + tests/ui_hardening.rs + 本卡 |
| **创建日期** | 2026-10-05 |
| **来源** | 用户拍板「加强前端 GUI 功能实现」+「GUI 搜索形同虚设」指认（2026-10-05，M10-WP00 §1） |

## 根因

检索命中卡 name 行渲染 `content_id.slice(0,8)` 哈希切片——数据在索引与
graph 里都有可读形态，但 UI 未透出；用户看到一屏哈希 = 「搜索形同虚设」。

## 修复

1. ipc.rs：`SearchHit` 增 `filename: Option<String>`；bm25/hybrid 双路径
   收集段改经 `enrich_hits`（graph `entries_by_content` 反查首个 entry
   name 回填；孤儿索引行 None——前端回落哈希切片）。
2. app-core-v3.js：命中卡 name 行优先 `h.filename`（缺失回落哈希切片）。
3. 测试：e2e（graph 关联行 filename=「2026Q3-验收报告.md」透出 + 孤儿行
   None）+ 静态探针（命中卡模板优先 filename）。

## 验收

- [x] 单测/e2e 绿（desktop 22 passed；1 failed 为既有环境耦合 e2e
      `mcp_call_real_sidecar_ext_list`——M7 旧未签名 demo 扩展 × WP04
      强制验签，M9-report §5 债表在案，与本改动无关）；
- [x] fmt/clippy -D warnings 绿；零新增依赖；
- [x] 提交挂 Task-ID `M10-WP01-T01`；
- [ ] GUI 实操截图随 T04 巡检窗口（人工件）。
