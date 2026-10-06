# Task: M10-WP03-T02 详情面板 UX（关闭 + 信息补全 + 复制）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP03-T02 |
| **类型** | 实施（R0：前端增强，纯客户端） |
| **范围** | desktop ui/app-core-v3.js + ui/index.html + ui/styles-v3.css + tests/ui_hardening.rs + 截图 + 本卡（零 IPC / 零引擎 / 零 gateway；`ipc.rs` 不动） |
| **创建日期** | 2026-10-06 |
| **SPEC** | docs/specs/M10-WP03.md §2.2 / §3 |

## 根因（代码现状）

`showDetail`（app-core-v3.js:129-157）面板无关闭能力（模板无 ×、无
Esc 接线，aside 常驻）；content_id 只显 16 字符切片（:154），副本
路径不可复制；「修改时间」未渲染——数据已在载荷
`AssetDetail.copies[].mtime_ns`（ipc.rs:223-227，EntryRow 含
mtime_ns，store.rs:36）。

## 修复

1. **关闭**：头部 × 按钮 + Esc 两通道；关闭 = `display:none` + 内容
   清空；硬约束：不清空/重载浏览列表、零重发 IPC；Esc 仅面板可见时
   生效（输入框聚焦不拦截）。
2. **补全**：增「修改时间」行 = `copies[0].mtime_ns`（与 size 取
   first 同口径，ipc.rs:238）；缺失/0 → 「—」。
3. **复制**：完整 content_id + 每条副本路径「复制」→
   `navigator.clipboard.writeText`；「已复制」1.5s 回落（沿
   M10-WP02 §2.2 契约；不可用回落 execCommand，实现期定）。
4. **不变量**：新增模板动态段 esc() 全覆盖，零未转义插值。

## 验收

- [ ] 静态探针：× + Esc 接线 / 关闭后列表节点不清空 / 零重查询站点
      / 可见性门控 / mtime 行 + 「—」回落 / 两处 clipboard.writeText
      + 反馈回落 / 无未转义插值；
- [ ] GUI 实操截图 `docs/screenshots/M10-WP03-T02-*.png`（面板打开 →
      关闭前后 + 复制到粘贴板验证）；
- [ ] fmt/clippy/test 绿；改动仅限本卡范围；零新增顶层依赖；footer
      IPC 计数 12 不变。
