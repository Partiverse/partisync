# Task: M10-WP03-T01 M9-WP03-T04 GUI 三态验收承接 + 全 tab 巡检

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP03-T01 |
| **类型** | GUI 人工验收（kind=gui；M9 尾款承接，M10-WP00 §2 拍板 #5） |
| **范围** | docs/screenshots/M10-WP03-T01-*.png + docs/specs/M9-WP03.md（§3 T04 勾选）+ docs/specs/M9-WP00.md（§1-WP03 行）+ 本卡。**零代码改动** |
| **创建日期** | 2026-10-06 |
| **SPEC** | docs/specs/M10-WP03.md §2.1 / §3 |
| **判例** | M8-WP05-T04 + M8-WP05-ui-report §6（AXPress 可靠 / 截图归档硬性 / 智能引号坑） |

## 交付物

1. **记忆 tab 三态截图**：全空态 / 有数据态（GUI memory_write ≥2 条
   含 tags，列表 + 绿徽章 + root hex 在镜）/ 验证失败态（篡改红徽章）；
2. **全 7 tab 巡检截图**（浏览/检索/记忆/同步/重复内容/作业/扩展，
   每 tab ≥1 帧）；检索帧含 M10-WP01 已交付件在镜（filename 卡 /
   chips / 索引徽标任一，对账其 §2.1 遗留注记）；
3. **回填**：M9-WP03 §3 T04 勾选 + M9-WP00 §1-WP03 行更新。

## 篡改演示安全流（硬约束）

memory 库 = 生产库 `~/.partisync/graph.db`（gateway mcp.rs:313 默认）。
只允许动本演示自写的行：

1. GUI memory_write 写演示行（内容含标记词）；
2. 退出桌面 app（sidecar 释放 DB）；
3. `cp ~/.partisync/graph.db /tmp/graph.db.wp03t01.bak` 备份；
4. `sqlite3 ~/.partisync/graph.db "UPDATE …"` 篡改**仅演示行** content；
5. 重启 app → 记忆 tab：root 徽章红 + 演示行验证红，截图；
6. 删除演示行（sqlite3 DELETE）收尾，演示行不留库。

严禁改演示行以外任何行。发现新缺陷不顺手修——登记 SPEC 修订或
另立任务（铁律 9）。

## GUI 工具链判例（硬性）

- cliclick 对静态按钮不派发 onclick → 走 macOS Accessibility
  **AXPress**；
- setValue 遭智能引号替换（尾引号变 ”）→ 空参调用或含数字值 JSON；
- 环境不可得 → PR 保持 OPEN 打 label「待 GUI 验证」，不得合入。

## 验收

- [x] 截图归档 `docs/screenshots/M10-WP03-T01-*.png`（三态 + 巡检
      ≥7 帧）——10 帧：memory-empty / memory-green / memory-tamper-red /
      memory-restored + tab-browse/search/sync/dups/jobs/ext；
- [x] 篡改安全流留痕（PR 正文：备份/单行/收尾三步说明）——db+wal+shm
      三件备份 /tmp/graph.db.wp03t01.bak* → `UPDATE … WHERE
      memory_id=<演示行A>`（changes()=1）→ DELETE 演示行 + memory_root
      还原为演示前原值（5 行、root e167fe58… 一致，残留 0）；
- [x] M9-WP03 §3 T04 勾选 + M9-WP00 §1-WP03 行回填随 PR；
- [x] 零代码 diff（docs/ + screenshots/ 之外无改动）。

## 实施留痕（2026-10-06）

- 空态帧经临时 `--data-dir` 空库实例采集（生产库已有 5 条真实记忆，
  不清不动）；绿态/红态/还原态对生产库 `~/Library/Application
  Support/.partisync/partisync.db`（桌面侧车 `--db` 实际指向，即
  `state.rs:52` 同源库）执行。
- GUI 工具链新判例：`setValue` 经 System Events 对 WKWebView 文本域
  静默失效、`<input>` 文本框呈幽灵别名（写它实写 textarea，bounds
  越窗）——可靠通路 = Swift AX 直设 `kAXValueAttribute`（textarea）
  + 剪贴板粘贴（input）+ AXPress（按钮）；拼音输入法会劫持 keystroke
  英文（「demo」混入中文），粘贴通路不受影响。
- 巡检新缺陷登记 M10-WP03 §6-D5：sidecar 持 tantivy 写锁致桌面自身
  检索 LockBusy（记忆 tab 先用后）；检索帧以先检索后 sidecar 顺序
  绕行采集。
