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
      + `t01_sort_rerender_preserves_proof_row_and_empty_text` 全绿
      （2026-10-06 本地实测，ui_hardening 20/20；对抗评审 R3 修复后
      成功/失败两处重置分别按上下文锚定，突变实验单删任一处均转红；
      R2/R4 修复：排序重渲保留 mem-proof 展开行 + 空态文案 memEmptyText 透传）。
- [x] 二轮对抗评审修复：排序 × in-flight 验证竞态——memVerifyRow
      await 前抓行引用，await 期间点表头重写 tbody 后 row.after(tr)
      游离插入证明行静默不可见；修复 = 按 memory_id 延迟寻址（await
      后现查当前 tbody 锚点，行不在窗口落 append 兜底不游离），
      探针 `t01_verify_proof_insert_readdresses_after_await` 锁死
      （突变实验恢复旧代码即转红 0 passed/1 failed，2026-10-07 本地
      实测 ui_hardening 21/21 全绿）。
- [x] GUI 实操截图 `docs/screenshots/M10-WP02-T01-*.png`（排序前后 +
      Enter 检索不重载、仍停记忆 tab）——✅ **2026-10-09 解锁窗口补验通过**：
      merged 分支真实实例 + demo 库 10 条种子（sidecar `memory_write` 真实
      链路写入 + 4 行 created_ns 回拨后真实写入触发根刷新自洽，banner 承诺
      验证 OK）；默认序 / 创建时间↑↓三态（箭头与行序同步）/ score↑ FTS 路径
      排序 / mem-q `partisync` 回车 4 命中仍停记忆 tab（防整页重载）/ 排序态
      点「验证」证明行锚在行正下不飘位（8a24485 延迟寻址）/ 重排后详情行+
      证明行原样重锚（§6-R6 合入链微任务）全过，七帧归档。（前史：
      2026-10-06/07 三轮锁屏不可得登记见 SPEC §3 与 PR 评论。）
- [x] fmt/clippy/test 绿；改动仅限本卡范围；零新增顶层依赖
      （2026-10-06 本地实测：fmt ✓ / clippy -D warnings ✓ /
      cargo test --workspace exit 0）。
