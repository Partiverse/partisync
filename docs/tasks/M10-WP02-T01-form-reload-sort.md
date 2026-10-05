# Task: M10-WP02-T01 记忆 form 防重载 + 表头客户端排序

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP02-T01 |
| **类型** | 债务清偿 + 功能补全（M10-WP00 §1-WP02「排序」项） |
| **范围** | desktop ui/app-core-v3.js + ui/index.html + ui/styles-v3.css + tests/ui_hardening.rs + 本卡（零 IPC / 零引擎 / 零 gateway） |
| **创建日期** | 2026-10-05 |
| **SPEC** | docs/specs/M10-WP02.md §2.1 / §3 |

## 根因

1. **form 整页重载（登记债，本卡清偿）**：`index.html:88` inline
   `onsubmit="return false"` 被 CSP `script-src 'self'`
   （tauri.conf.json:25）拦成死代码；`btn-mem-search` `type=submit`
   （index.html:91）+ `mem-q` Enter 无 preventDefault
   （app-core-v3.js:742）→ 点 [检索] / Enter 都触发 form 默认提交 =
   整页刷回 browse tab。与 M10-WP01-T04 检索 tab 实测 bug 同型；
   `ui_hardening.rs:405-406`（PR #162）明文登记「记忆面板同名 form
   模式为既有遗留」。
2. **列表顺序纯服务端固定**：`memory_search` 无排序参数
   （store.rs:2286，FTS score DESC → created_ns DESC；LIKE
   created_ns DESC），前端无次级整理手段。

## 修复

1. 摘除死 inline onsubmit；按钮 `type=button`；JS 侧 Enter keydown
   preventDefault + form submit 兜底 preventDefault（沿
   app-core-v3.js:731-732 检索判例）；回归锁 =
   `t04_search_form_never_reloads_page` 同款断言扩到记忆 form。
2. 表头点击客户端排序：创建时间 / score / tags / 来源设备四列（内容
   列除外）；再点反序、第三点回默认；箭头指示；新检索 / 换 query /
   换 tag 重置为默认序；tags 排序键 = memTags() 解析后列表字典序。
   诚实边界：只排序当前渲染窗口（≤50 条）。

## 验收

- [x] 静态探针：记忆 form 零 `onsubmit=` / 零 `type="submit"` /
      Enter preventDefault / submit 兜底断言 + 排序接线（点击排序 /
      方向切换 / 回默认 / 新检索重置 / 箭头指示）——
      `t01_memory_form_never_reloads_page` + `t01_memory_table_header_sort_wired`
      全绿（2026-10-06 本地实测，ui_hardening 19/19）。
- [ ] GUI 实操截图 `docs/screenshots/M10-WP02-T01-*.png`（排序前后 +
      Enter 检索不重载、仍停记忆 tab）——**待 GUI 验证**：验证时段
      （2026-10-06 00:4x）主机屏保→锁屏（HID 空闲 51 min+，Touch ID
      无人在场），真实实例窗口退化（210×141 负坐标帧 / AX 零窗口），
      实操不可得；PR 保持 OPEN 打「待 GUI 验证」，解锁后补验归档。
- [x] fmt/clippy/test 绿；改动仅限本卡范围；零新增顶层依赖
      （2026-10-06 本地实测：fmt ✓ / clippy -D warnings ✓ /
      cargo test --workspace exit 0）。
