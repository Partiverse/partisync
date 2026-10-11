# AI Review Report — M11-WP02-T03（补档）

- **Task-ID**: M11-WP02-T03「sidecar/桌面/CLI search 改只读打开，消除 D5 IndexWriter 锁冲突根因」
- **审查对象**: squash 合入 commit `838d0266`（PR #211，+141/−34，8 文件）；已确认 `838d0266` 为 main 祖先
- **风险分级**: **R1** —— 任务卡声明 R1（gateway/desktop/cli 三 crate 横切），SPEC §2.0 同口径（gateway/index 横切面 + 消费新公共 API 面）。按 R1 执行 **100% diff 全审**（8 文件逐行核毕），并以全仓 `open_or_create`/`open_read_only` 调用点清点做完备性旁证。
- **审查性质**: 补档（合入后审查，M11 里程碑 §7 抽查审计发现审查工件缺口，现补齐）。合入时无独立 AI 对抗审查工件，本报告即为该缺口的第一审查者记录。
- **输入**: SPEC docs/specs/M11-WP02.md（§2.1/§2.2/§3.0-T03/§3.1/§5/§7-0.2）；任务卡 docs/tasks/M11-WP02-T03-consumer-migration.md；D5 登记（M10-WP03 §6-D5 + docs/reviews/M11-WP02-d5-repro-evidence.txt 修复前实证）；PR #211 正文证据；后验 docs/reviews/M11-WP01-t8-cli-fulltest-evidence.txt（PASS38）

---

## 1. SPEC 合规（验收条 | 覆盖证据 | 缺口）

| 验收条（SPEC §2.2 / §3.1 T03 行） | 覆盖证据 | 缺口 |
|---|---|---|
| sidecar（build_server_state）只读打开，启动不持写锁（fuser 复检为空）——D5 症状消除直证 | mcp.rs:1807 `open_read_only`（gateway 全仓唯一索引打开点）；PR #211 正文：sidecar 启动后 fuser/lsof 无写锁句柄、`.tantivy-writer.lock` 不存在、CLI search 同库成功（修复前同路径 LockBusy fatal，对照 d5-repro-evidence.txt）；P24-a 探针背书只读打开不建锁 | 无。证据随 PR 正文归档（合规）；仓内无独立 after-state 证据文件，规则允许 |
| sidecar「索引不存在 → 降级为空（口径不变）」 | 引擎未注入时 asset_search 返回空 hits（mcp.rs:296 docstring + asset_search handler `None → ok_json(hits:[])`），与修复前失败降级口径一致 | 无 |
| §7-0.2 修订（懒成分撤回，eager read_only） | 实现为 eager `open_read_only`（build_server_state 内联打开），0.2 修订行随本 commit 落 SPEC | 一致 ✓（修订与实现同 PR，追溯闭环） |
| 桌面（state.rs OnceCell）改只读打开 | state.rs:98 `open_read_only`；OnceCell 保持 | bootstrap 偏差见 §5-建2 |
| CLI search 只读打开 + 缺索引 hint；reindex/watch 写面维持 | main.rs:912 `open_read_only` + `hint: 先运行 partisync watch 或 reindex`；reindex.rs:208 生产写面 `open_or_create` 未动（:352/:437 均在 `#[cfg(test)]` 内）；后验 t08 PASS38 覆盖 CLI search/reindex 双面 | 无 |
| 锁冲突错误含指引文本且分类不劣化 | bm25.rs:378 写者获取失败错误含三段指引（另一写者持锁/只读查询可走 open_read_only/批量重建先停写面）；错误分类（err() 同一构造路径）未变 | 指引缺 SPEC §2.2 引文中的 `<pid/file>` 成分（§5-建3） |
| D3 清偿：search/search_hybrid 打开失败落 `Index` | ipc.rs:101-106（search）、:171-176（search_hybrid）`Internal→Index` 拨回，与 index_stats（ipc.rs:220-223）同款；映射仅作用于 `state.index()` 错误路径（该路径错误均为索引打开域），消息原文保留 | 无 |
| 三件套绿；改动仅限清单；零新增顶层依赖 | PR 正文：桌面 commands 25/25、gateway/cli/index 全绿、clippy -D warnings 四 crate 零告警；零新增依赖 | 文件清单差 1：tests/commands.rs 未列卡（§5-建1） |

---

## 2. 文件清单合规（Iron Rule 9）

diff 实触 8 文件 vs 任务卡清单 7 文件：

| 文件 | 卡内 | 判定 |
|---|---|---|
| crates/partisync-gateway/src/mcp.rs | ✓ | 合规 |
| crates/partisync-desktop/src/state.rs | ✓ | 合规 |
| crates/partisync-desktop/src/ipc.rs | ✓ | 合规 |
| crates/partisync-cli/src/main.rs | ✓ | 合规 |
| crates/partisync-index/src/search/bm25.rs | ✓ | 合规（锁冲突指引文案，属卡交付#4） |
| docs/specs/M11-WP02.md（§7 0.2 行） | ✓ | 合规 |
| docs/tasks/M11-WP02-T03-consumer-migration.md | ✓ | 合规（本 commit 新建） |
| **crates/partisync-desktop/tests/commands.rs** | **✗ 未列** | **Iron Rule 9 形式缺口**（§5-建1） |

判读：tests/commands.rs 的改动是只读迁移的**必然后果**（旧种子路径 `state.index().upsert(...)` 在只读实例上会得结构化错误而 panic），非「顺手修」——实质合规、形式缺登记。卡内验收句「改动仅限本卡清单」因此与事实不符，需 docs-only 补记。

commit trailer：`Task-ID` ✓ `Spec` ✓ `AI-Assist` ✓（Iron Rule 2 合规）；`AI-Review` 缺失（本补档即填补该位）；`Reviewed-By` 未见人工终审记录——补档审查不代位，人工终审仍欠。diff 175 行 ≤400（Iron Rule 3 ✓）。

---

## 3. 测试证据

| 项 | 证据 | 判定 |
|---|---|---|
| P24 探针（T02 面，为 T03 提供语义背书） | crates/partisync-index/tests/m11_wp02_readonly.rs：p24_a（只读打开前后锁文件存在性不变 + is_read_only）、p24_b（与活跃写者并存 search/approx_count 正常）、p24_c（≥3 只读实例并发无互斥）；登记行 docs/tests/properties.md:31（2026-10-09 随 T02 先于测试代码） | ✓ |
| D5 消除直证（T03 主验收） | PR #211 正文：sidecar 启动后 fuser/lsof 无写锁句柄 + `.tantivy-writer.lock` 不存在 + CLI search 同库成功；修复前对照全文归档 d5-repro-evidence.txt（pid 183669 `10wW` 持锁、CLI LockBusy fatal） | ✓ |
| 桌面回归 | commands.rs 种子迁移：独立写者引擎（`Bm25Index::open_or_create`）播种 + state 只读引擎显式 `force_reload()`（OnCommitWithDelay 异步刷新的同步测试语义，沿旧判例）；PR 正文 25/25 绿 | ✓ |
| CLI 写面回归 | reindex 判例复跑（卡验收）；后验 t08（2026-10-11）PASS38：CLI search 只读面 + reindex 写面 + partisd loopback 全矩阵 | ✓ |
| Green-by-tampering 扫描 | 无 `#[ignore]`、无断言放宽/删除；命中断言未动（diff 未触及任何 assert 期望值）；种子迁移与 force_reload 是只读语义下的正当代码迁移，非掩盖。p24_a 的「pre==post 存在性等价断言」非放宽——与 SPEC P24-a「无写锁文件**创建**」逐字对齐（陈留锁不被只读打开清除是正确行为，注释坦陈 tantivy 异步锁清理机制） | ✓ 无篡改 |

未在本补档中复跑门禁（合入后审查；以 PR #211 归档证据 + t08 后验为准）。

---

## 4. 安全 / 钉子清单扫描

| 钉子面 | 本 diff 触碰 | 判定 |
|---|---|---|
| schema 迁移 | 无写面 schema 变更；open_read_only 对旧 schema（缺 orig 域）`get_field().ok()` 兼容，与 open_or_create 同构 | ✓ 无 |
| unsafe | 无（workspace forbid 未动） | ✓ 无 |
| 密码学 | 无 | ✓ 无 |
| 公共 API | 无新增/无签名变更（bm25.rs 仅错误文案字符串；消费的 `open_read_only` 系 T02 已登记公共 API） | ✓ 无 |
| 鉴权/授权 | 无 | ✓ 无 |
| 依赖 | 零新增顶层依赖；deny.toml/rust-toolchain/CI 门禁未触 | ✓ 无 |
| 错误面信息泄露 | 锁冲突指引为操作指引，无路径/pid/敏感信息外泄（反而比 SPEC 引文少 pid/file，见建3——就泄露面而言更保守） | ✓ 无 |

---

## 5. 实现细节发现

**阻断：0 条。**

**建议：4 条**

1. **[建议] 任务卡清单遗漏 tests/commands.rs（Iron Rule 9 形式缺口）**
   - docs/tasks/M11-WP02-T03-consumer-migration.md:29-32 → diff 实触 8 文件，卡列 7 文件，`crates/partisync-desktop/tests/commands.rs` 未列；卡验收句「改动仅限本卡清单」与事实不符。
   - 建议：docs-only 修订任务卡，清单补记该测试文件（注记「只读迁移必随后果」）。
   - 期望回执：任务卡清单含 8 文件且与 `git show 838d0266 --stat` 一致。
2. **[建议] 桌面缺索引语义：任务卡交付#2 文本与实现相左，且桌面瞬时写面未入 SPEC 登记**
   - 卡交付#2 写「索引不存在 → `DesktopError::Index` 结构化错误（指引建索引）」；实现为**一次性引导空索引**（state.rs:91-97：meta.json 缺失 → `open_or_create` 建后即弃 → 只读打开）。CHANGELOG（「新装首启自动引导空索引」）与 PR #211 正文（含「初版 bootstrap 持锁缺陷已自纠」）已如实记载，但在仓任务卡与 SPEC §2.2/§7-0.2 均未随改——SPEC §2.0「写者归真写面（CLI reindex/watch 及后续 daemon）」纪律下，桌面首启 bootstrap 是一个未登记的瞬态第三写面。
   - 行为本身可接受：瞬态持锁（建后即弃）；与活跃写者撞锁时得结构化错误且 `get_or_try_init` 失败不缓存、下次调用自动重试；错误文案经 bm25.rs:378 新指引透出。
   - 建议：docs-only 修订——卡交付#2 改述为「缺索引 → 一次性引导空索引（建后即弃写者）」；SPEC §2.2 或 §7 补一行登记桌面瞬态写面（沿 0.2 修订体例）。
   - 期望回执：卡/与 SPEC 文本与 state.rs 实现一致；§2.0 写者归属清单含「桌面首启 bootstrap（瞬态）」。
3. **[建议] 锁冲突指引缺 `<pid/file>` 成分**
   - crates/partisync-index/src/search/bm25.rs:378 → SPEC §2.2 引文为「另一写者持有索引锁：**<pid/file>**；只读查询不受影响；批量重建请停写面或等待」，实现文案无 pid/锁文件路径（且为英文）。三要素中「只读查询不受影响」被更有行动力的「read-only queries can use open_read_only」替代，可接受；pid/file 缺失使用户定位持锁进程仍需自跑 fuser/lsof。
   - 建议：后续小 PR 在错误文案补持锁方线索（ tantivy Lockfile 错误本身不含 pid，需从锁文件内容/进程自报），或 SPEC 0.5 修订将该引文降级为「含三段指引、pid/file 尽力而为」。
   - 期望回执：文案与 SPEC 引文二选一对齐（补成分或修引文）。
4. **[建议] SPEC §5 清单与实际落点不一致**
   - docs/specs/M11-WP02.md:166 → §5 T03 行列 `crates/partisync-cli/src/reindex.rs`（「锁冲突错误结构化(写面维持)」），实际未触；结构化落在 bm25.rs 写者获取点（对所有写者含 reindex/watch 生效，覆盖面更优）。任务卡（权威清单）已正确未列 reindex.rs。
   - 建议：T06 收尾或 SPEC 下次修订时校正 §5 该行注记。
   - 期望回执：§5 与实触文件对齐或加注「结构化统一落 bm25.rs」。

**风格：3 条**

1. crates/partisync-desktop/tests/commands.rs:723-731 → 调试残留：`println!("DEBUG docs={} lock={}", ...)` + `unwrap()` 诊断块遗留在 `t01_search_hit_carries_graph_filename` 内。建议随下个触面 PR 删除。
2. crates/partisync-desktop/src/state.rs:78-80 → docstring 仍列「索引不存在」为 `index()` 错误因；bootstrap 前置后该分支实际不可达（仅 bootstrap 自身失败间接可见）。建议 docstring 随建2 的文档修订一并更正。
3. commit type(scope) = `fix(desktop)` 低估横切面（实触 gateway/cli/index 四 crate，PR 正文自述「四 crate 横切」）。后续跨 crate 任务建议 scope 取主导 crate 或 wp 名。

**移交观察（非本任务责任面）**: SPEC §3.0 T06 行与 §3.1 末项（T06 关账回填）仍为 ☐/未勾，而 §7-0.4 已记 T06 关账完成——勾选状态未随 0.4 翻转，移交 T06 责任面核对。

---

## 6. 与 SPEC 的逐条对照

| SPEC 条 | 实现 | 判定 |
|---|---|---|
| §2.0「多读者只读打开 + 按需写者；读消费者一律只读路径」 | 三面读路径全只读（见下完备性结论）；写面 reindex/watch 未动 | ✓ |
| §2.0「写者归真写面」 | 桌面首启 bootstrap 瞬态写面未登记（建2） | △ 文档缺口 |
| §2.1 只读打开语义（不建写者/写方法结构化错误/缺目录结构化错误/VectorStore 同构包装） | bm25.rs open_read_only：meta.json 判据 + NotFound 结构化错误 + `writer: None` + writer_lock() 守卫 + is_read_only()；engine.rs:150 vector 侧 open_or_create 按 SPEC 明文豁免（无跨进程锁）并在 docstring 注明 | ✓（T02 面，T03 消费合规） |
| §2.1 旧索引/旧 schema 兼容 | open_read_only 用 `index.schema()` + `get_field().ok()`，旧 schema orig 域 → None，与写路径同构 | ✓ |
| §2.2 sidecar 只读（eager，0.2 修订） | mcp.rs:1807 eager read_only；§7-0.2 同 commit 落档 | ✓ |
| §2.2 桌面只读 | state.rs:98 只读 + bootstrap（超出卡面文字，建2） | △ |
| §2.2 CLI search 只读 + reindex/watch 写者维持 | main.rs:912 / reindex.rs:208 | ✓ |
| §2.2 锁冲突错误结构化 | bm25.rs:378 含指引，缺 `<pid/file>`（建3） | △ |
| §2.2 D3 清偿 | ipc.rs search/search_hybrid 拨回，与 index_stats 同款 | ✓ |
| §3.1 T03 三行验收勾（✅） | 证据核毕成立（§1 表） | ✓ |
| §5 文件清单（T03 相关行） | reindex.rs 行与实际落点不符（建4） | △ 文档 |

**只读迁移完备性核查结论：三面全覆盖，无遗漏读写打开点。**
全仓 `open_or_create` 生产调用点清点（HEAD）：① CLI reindex/watch 写面（reindex.rs:208）——真写面，SPEC 指定保留；② 桌面首启 bootstrap（state.rs:95）——瞬态、建后即弃；③ eval/runner、examples/demo_query、benches——离线工具/基准，非常驻消费面。读消费者三面（gateway mcp.rs:1807 / desktop state.rs:98 / CLI main.rs:912）全部走 `open_read_only`，desktop/ipc 与 gateway 内无第二打开点；partisd 经 build_server_state 复用只读组装（T04 面）。无锁倒挂：读面零写锁，写面互斥由 tantivy 原生锁承担，锁冲突错误含指引。已知残债 = SPEC §6-7 跨进程 reader 自动 reload 不生效（0.3 如实登记转底座改进债，测试内 force_reload 为该登记下的正当处置）。

---

签字: AI-Reviewer GLM-5.3-Flash @ 2026-10-11
