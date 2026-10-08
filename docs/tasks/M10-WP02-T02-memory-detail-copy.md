# Task: M10-WP02-T02 记忆详情联动展开 + 复制

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP02-T02 |
| **类型** | 功能补全（M10-WP00 §1-WP02「详情联动」「复制」项） |
| **范围** | desktop ui/app-core-v3.js + ui/index.html + ui/styles-v3.css + tests/ui_hardening.rs + 本卡（零 IPC / 零引擎 / 零 gateway） |
| **创建日期** | 2026-10-05 |
| **SPEC** | docs/specs/M10-WP02.md §2.2 / §3 |

## 根因

内容列一律 90 字截断（app-core-v3.js:625），metadata 字段返回不展示，
memory_id 无任何可见位置（只在 DOM data-* 属性）——内容全貌不可见、
id 不可复制（MCP 工具联调只能开 DevTools 抄 data 属性）。

## 修复

1. 点击记忆行（「验证」按钮 stopPropagation 隔离）展开
   `tr.mem-detail` 行（沿 .mem-proof 展开行判例，styles-v3.css:367）：
   完整 content、tags 完整、metadata pretty JSON（esc 后）、完整
   memory_id、created_ns 完整本地时间、score + 口径注记（FTS 匹配秩 /
   LIKE 恒 1.0，store.rs:2279-2282 语义诚实透出）；再点收起。
2. 详情行内「复制内容」「复制 ID」按钮 → `navigator.clipboard`
   `.writeText`（webview 内建 API，零新增依赖）；成功反馈按钮文案
   瞬变「已复制」≈1.5s 回落；剪贴板不可用回落方案实现期定（SPEC
   §6-R1），GUI 实操必测复制成功。
3. 硬约束：展开/收起不关闭既有证明行、不清空列表；证明行单开语义
   维持；详情模板动态段 esc() 全覆盖（无未转义插值）。

## 验收

- [x] 静态探针：详情模板字段齐全（含 score 口径注记）+ esc 全覆盖 +
      展开不关证明行/不清列表 + 行点击与验证按钮事件隔离 + 复制经
      clipboard.writeText + 反馈回落——`t02_memory_detail_row_fields_and_esc_coverage`
      + `t02_detail_expand_preserves_proof_rows_and_list` +
      `t02_row_click_and_verify_button_isolated` +
      `t02_copy_via_clipboard_write_text_with_fallback_and_feedback`
      全绿（2026-10-06 本地实测，先红后绿：实现前 4 探针全红，实现后
      ui_hardening 21/21）；
- [x] GUI 实操截图 `docs/screenshots/M10-WP02-T02-*.png`（详情展开 +
      复制粘贴到别处验证内容/ID 一致）——**✅ GUI 实操验证通过
      （2026-10-07 解锁窗口补验）**：2026-10-06 首验因主机锁屏实操不可得
      （PR 曾打 label「待 GUI 验证」），2026-10-07 解锁后以 AX（AXPress）
      操控本分支重建实例五步全过：① 记忆 tab 5/5 条、>90 字行列表「…」
      截断；② 详情行完整透出（content 无截断/metadata pretty JSON/完整
      64-hex id/score 口径注记）；③ 复制内容→「已复制」反馈 +
      `pbpaste` 与库内 content 逐字节一致（191 字/401 字节）；④ 复制
      ID→`pbpaste` = 完整 64 hex；⑤ 验证→证明行（root e167fe58…=
      banner 承诺根）与详情共存，再点行详情收起、证明行与列表原样。
      五帧归档 docs/screenshots/（list-truncated / detail-expand /
      copy-feedback / verify-proof-coexist / collapsed-proof-intact）；
- [x] fmt/clippy/test 绿（2026-10-06：`cargo fmt --all --check` ✅ /
      `cargo clippy --workspace --all-targets -- -D warnings` ✅ /
      `cargo test --workspace` ✅ exit 0）；改动仅限本卡范围
      （app-core-v3.js / styles-v3.css / ui_hardening.rs / 本卡 / SPEC §3
      回填；index.html 未触碰——T02 无需 HTML 结构变更）；零新增顶层
      依赖（剪贴板走 webview 内建 navigator.clipboard + execCommand 回落）。
