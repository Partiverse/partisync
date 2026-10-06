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

- [x] 静态探针：× + Esc 接线 / 关闭后列表节点不清空 / 零重查询站点
      / 可见性门控 / mtime 行 + 「—」回落 / 两处 clipboard.writeText
      + 反馈回落 / 无未转义插值；✅ ui_hardening +6 探针先红后绿
      （23/23 全绿）：`t02_detail_close_dual_channel_and_zero_requery` /
      `t02_esc_close_gated_on_visibility_and_input_focus` /
      `t02_detail_mtime_row_first_copy_with_dash_fallback` /
      `t02_copy_full_cid_and_copies_with_feedback_and_fallback` /
      `t02_detail_dom_parity` /
      `t02_detail_no_raw_interpolation_of_detail_fields`。实现形态：两复制
      站点（content_id 全量 / 副本路径）经单点助手 `detailCopy` 走同一
      `navigator.clipboard.writeText` 通道 + `detailCopyFallback`
      （execCommand）回落；反馈防重入沿 PR #166 F3 判例。
- [x] GUI 实操截图 `docs/screenshots/M10-WP03-T02-*.png`（面板打开 →
      关闭前后 + 复制到粘贴板验证）；✅ 2026-10-06 补验完成（首轮
      13:31–13:39 主机锁屏八轮 ×55s，13:49 解锁窗口按移交清单执行）。
      六帧归档：`M10-WP03-T02-detail-open.png`（修改时间行 + 两类复制钮
      + × 头在镜，列表完好）/ `…-copy-cid-copied.png`（「已复制」反馈
      在镜；`pbpaste` = 64 hex 全量
      `1ffdc5397cea41d0…b061eddf`，与行指纹徽章 `1ffdc539` 对账）/
      `…-copy-path-copied.png`（路径钮「已复制」在镜；`pbpaste` =
      `/d0001_vacation_photos_2024.md` 逐字）/ `…-x-closed-list-intact.png`
      （× 关闭后浏览列表原样在镜）/ `…-reopen.png`（再点行重开）/
      `…-esc-closed-list-intact.png`（Esc 关闭后列表原样在镜）。
      工具链注记：× 为 27×20 pt 小热区，cliclick 合成点击差 3 pt 未
      派发 onclick——经 macOS Accessibility AXPress 派发成功
      （M8-WP05 §6 判例原样再证；CID/路径复制钮 cliclick 均正常派发）。
      「已复制」1.5s 回落顺带在镜（x-closed 帧中 CID 钮已回落
      「复制」）。
- [x] fmt/clippy/test 绿；改动仅限本卡范围；零新增顶层依赖；footer
      IPC 计数 12 不变。✅ fmt --check / clippy -D warnings /
      test --workspace 全绿（workspace exit 0；desktop：lib 7 /
      commands e2e 24（1 ignored 判例沿用）/ ui_hardening 23）；diff
      触及 ui/app-core-v3.js、ui/index.html、ui/styles-v3.css、
      tests/ui_hardening.rs + 本卡 + SPEC 回填，`ipc.rs`/`commands.rs`
      /引擎零触碰（footer 计数 12 由既有 t05 探针锁）。
