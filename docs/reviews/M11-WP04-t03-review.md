# AI Review Report — M11-WP04-T03

风险分级: R1（任务卡声明；diff 内未发现 R2 面任意一项——无 unsafe、无 graph schema 迁移、无密码学/鉴权、无公共 API 签名变更，**不升级**）

审查对象: commit 776bfef（PR #229，squash，main 祖先已核实）· SPEC 对照版本 **v0.2**（T03 合入时为 v0.1；两版差异仅在 §2.3 reindex 预算 <5%→≤+10%，系 T04 用户裁决 #234 所改，与 T03 验收条无交集，下文逐条注明）

- 100% diff 逐行审（bm25.rs 372 行 + 任务卡 27 行）
- tantivy API 断言全部在 pinned registry 源码核实（rsproxy.cn-e3de039b2554c837/tantivy-0.26.2，与 Cargo.lock `tantivy 0.26.2` 一致），无凭记忆引用
- 三门禁在本机实跑（776bfef..main 之间 bm25.rs/hybrid.rs 零变更，实跑结果对合入面有效）

---

## 1. SPEC 合规

| 验收条（SPEC §3） | 覆盖证据 | 缺口 |
| --- | --- | --- |
| 三门禁绿 | `cargo fmt --all --check` 绿；`cargo clippy --workspace --all-targets -- -D warnings` 绿；`cargo test -p partisync-index` 19/19 绿（含改写 t02 与两个新 t03）。workspace test 现红于 1 例 `partisync-desktop::commands::mcp_call_real_sidecar_ext_list`——**与本 diff 零代码交集**：根因为本机 `~/.partisync/extensions/demo_ext.wasm` 缺 `.minisig`（gateway ext loader verify-mandatory → ext disabled → `dir:null`），该 e2e 依赖本机 demo 扩展安装态；T03 未触碰 gateway/desktop/test 任何文件 | workspace 红灯为环境态非本任务引入；建议以独立事项登记（补 demo 扩展签名或放宽该 e2e 的 skip 守卫至校验 minisig），**不阻断 T03** |
| t02 中文摘要改写（可读原文、bigram `[[..]]`、无碎片流；仅此一项改写） | `t02_snippet_cjk_hit_and_no_term_fallback`（bm25.rs:889-967）按新契约改写且断言实质**收紧**：`contains("[[夸克]]")` → `contains("发现了新的[[夸克]]")`（要求可读原文上下文）+ 新增负断言 `!contains("了 新 的")`（碎片流禁现）；回落例（fb1 头截无 sentinel）与全空例（mt1 → None）原样保留。改写理由见测试 doc 注释 + PR 正文（commit message），符合「改写理由随 PR 正文」 | none |
| 旧 schema fixture 回落链测试（②③与现状一致、全链不 panic） | `t03_legacy_schema_six_fields_fallback`（bm25.rs:974-1073）：旧 6 字段 schema 内嵌索引 fixture，读侧验证 ②扇出串定位（`[[夸克]]`）与 ③头截（无 sentinel），写侧验证旧目录 `open_or_create` + `upsert` 自动降级不写 orig、不 panic、检索不受影响（total=2） | 单字段层级覆盖完整；**跨字段优先级变化未覆盖**（见 §5-建议 1） |
| 检索等价测试（orig 引入前后命中集逐项一致） | `t03_search_equivalence_with_orig_fields`（bm25.rs:1076-1180）：结构钉（`parse_query("ocr_text_orig:hello")` / `"transcript_text_orig:hello"` `is_err()`、`"ocr_text:hello"` `is_ok()`——真 assert，依据 tantivy query_parser.rs:541-543 `!is_indexed→Err` 已源码核实）+ 快照钉（4 文档×5 查询，total/content_id/score 逐项 `assert_eq!`/`assert!`，容差 1e-6——**真断言非打印**，本机实跑通过） | 快照 provenance（「main 版临时 harness 采集」）在仓库内无 harness/输出工件可复现（见 §5-建议 2）；有 T04 证据件独立佐证（top-20 命中集零差异、top-3 分数逐位相同） |
| Latin/混合查询现有用例零断言放宽（t02 改写除外） | diff 对 7 个存量用例**零改动**（parent 7 → merged 9，仅新增两个 t03 用例，diff 实证无其他 hunk 触及存量断言）；`t02_snippet_window_centers_on_hit_terms_beyond_head` / `search_handles_cjk_punctuation` 等实跑通过 | none |
| T04 实测报告 | 独立任务 M11-WP04-T04，不在本卡（任务卡明示）；证据件 docs/reviews/M11-WP04-t4-eval-evidence.txt 已存在（后验事实，不作 T03 验收依据） | 不适用于本卡 |
| P/L 登记（本 WP 不新增不变量，Q1 留评审） | diff 零 `proptest!`、零新 P/L 引用；「orig 与扇出域同源」由同调用点双写保证、单测可达——Q1「不登记」判断在本次评审中**维持** | none |

## 2. 文件清单合规

- 触碰文件（`git show 776bfef --stat`）:
  - `crates/partisync-index/src/search/bm25.rs`（+357/-15）
  - `docs/tasks/M11-WP04-T03-impl.md`（+27，任务卡）
- 是否在任务卡文件清单内: **是**（任务卡 §涉及文件清单恰列此两件；SPEC §5 预列的 hybrid.rs 零 diff 实证 ✓）
- Iron Rule 3: 384 insertions + 15 deletions = **399 行 ≤ 400** ✓（贴线，无拆分必要）
- Iron Rule 2: commit trailer `Task-ID: M11-WP04-T03` + `Spec:` ✓，任务卡存在于 approved SPEC 关联路径 ✓
- 零新依赖: Cargo.toml / Cargo.lock / deny.toml / rust-toolchain.toml 未动 ✓

## 3. 测试证据

- 单测新增: **2**（`t03_legacy_schema_six_fields_fallback`、`t03_search_equivalence_with_orig_fields`），存量 7 个零删改；partisync-index 覆盖面内无覆盖率回退信号（三门禁绿）
- 属性测试: 无新增（与 SPEC §3/Q1「不新增 P/L」一致；零 proptest）
- 检索等价断言形态: 结构钉（parser 注册面负向 assert，源自 tantivy `FieldNotIndexed` 错误路径，query_parser.rs:541-543 已源码核实）+ 快照钉（4 文档×5 查询 × (total, content_id, score≤1e-6) 全 assert）。快照值非打印、非 `unwrap_or_default` 掩盖；632 行 `.unwrap_or_default()` 为预存 content_id 提取路径，非本 diff 引入
- tampering 扫描: 零 `#[ignore]` 新增（partisync-index 存量 ignore 均为显式跑登记件：元数据仿真/doctest-私有函数，预存）；零 `dbg!`；零断言放宽（t02 为 SPEC §3 唯一授权改写项且实质收紧）；零测试删除
- 基准: 不适用（性能承诺归 T04 实测；SPEC §2.3 search 延迟项 T04 已登记探针无回归信号）

## 4. 安全 / 钉子清单扫描

| 钉子类别 | 是否触及 | 处置 |
| --- | --- | --- |
| schema 迁移 | 否 | tantivy schema 是索引目录内嵌物（meta.json），非 graph schema；存量索引按内嵌旧 schema 打开 + orig 域 `Option<Field>` 降级（bm25.rs:396-398/:446-447），无迁移语义，无需 ADR |
| unsafe | 否 | 全 diff 无 unsafe（workspace forbid 维持） |
| 密码学 / AEAD | 否 | 无 |
| Provider / MCP 公共 API | 否 | `Bm25Index`/`IndexEngine`/`IndexedDoc` 签名不变（仅 Bm25Index 增两个私有字段）；highlight 载荷仍为「纯文本 + `[[`/`]]`」透传契约，hybrid.rs/gateway/mcp/IPC 零 diff |
| 鉴权 / 授权 | 否 | 无 |

→ 维持 R1，无升级依据。

## 5. 实现细节发现

### 阻断（0 条）

无。

### 建议（3 条）

1. **bm25.rs:648-653 → 回落链跨字段优先级对存量索引有可观测变化，未测试未登记**
   现象: 父提交链序为「②ocr→②tx→③ocr→③tx」（跨层级优先），本实现改为逐字段「①ocr→②ocr→③ocr→①tx→②tx→③tx」。对旧 schema 存量索引的混合场景（ocr 有文本但无词面命中、tx 有词面命中），highlight 从 tx② 定位摘要变为 ocr③ 头部截断——信息量降级且行为异于改造前。SPEC §2.2「逐字段回落」文义支持此实现（实现合规），但 §3 验收条「回落行为与现状一致」仅在单字段层级被 fixture 验证，跨字段维度既无测试钉也无登记。
   建议: 补一条混合字段回落测试（旧 schema fixture + ocr 无词面/tx 有词面用例）钉死新序；或 SPEC 修订记录补一行「跨字段优先级由跨层级改为逐字段，存量索引 highlight 混合场景行为变化」。
   期望回执: 接受（补测试，另一小 PR）或改期（SPEC 补记）；驳回需说明为何该场景不可达。

2. **bm25.rs:1076-1180 → 检索等价快照 provenance 不可从仓库工件复现**
   现象: 快照值注明「main 分支 harness 输出逐字节转录」，但采集 harness 与原始输出未随 PR/`docs/reviews/` 归档，事后无法独立复核「快照=改造前行为」这一关键前提（现只能确认「快照=合入后行为」的确定性钉）。T04 证据件（top-20 命中集零差异、top-3 分数逐位相同）提供了独立后验佐证，但非 T03 验收依据。
   建议: 将临时 harness 片段与 main 版原始输出补档至 `docs/reviews/`（沿 M11-WP05-t2-query-evidence-v2.txt 判例），或在任务卡附采集方法最小复现。
   期望回执: 接受（补档）或驳回（明示以 T04 证据件为充分佐证并记录该裁决）。

3. **bm25.rs:218-242 → `orig_hit_ranges` 复杂度与 SPEC §2.3「O(窗口)」口径不符**
   现象: 实现对整个 orig 串逐 token 滑窗（O(len(orig)×Σlen(token)) 全文扫描），且 `orig_hit_ranges` 与 `orig_snippet` 各做一次 `Vec<char>` 全串收集；SPEC §2.3 写的是「定位摘要为 O(窗口) 字符串操作」。在 ≥100ms 级查询面上实测不可测量（T04 探针一致），性能承诺实质不破，但 SPEC 表述失真，且超长 OCR 文档 × 多 token 查询存在线性放大长尾。
   建议: 注释/SPEC 更正复杂度口径（如后续 WP 采纳「先首个命中锚点、窗口邻域内定位」可降到真 O(窗口)）；本期接受。
   期望回执: 改期（登记长尾）。

### 风格（3 条）

4. **bm25.rs:90-94 / :214-215 → 源码行号引用漂移**: 「segment_writer.rs:155-165 / :167-169」实际在 `src/indexer/segment_writer.rs`（缺 `indexer/` 路径段；get_field_entry 调用 ：159、`!is_indexed→continue` :167-168）；「remove_long.rs:31」实际边界谓词 `token.text.len() < self.token_length_limit` 在 :35-37（:31 是 `limit` 的文档注释行）。语义断言全部核实为真，仅坐标漂移 1-4 行。建议下次触碰该文件时顺手校准。期望回执: 接受（低优先级）。

5. **bm25.rs:3（模块头，预存）**: 「其余 = text_stemmed（降噪 OCR/转写）」与实际 schema 不符——`TEXT` 常量用 `DEFAULT_TOKENIZER_NAME`（default 链，无 stem；tantivy text_options.rs:276-281 已核实）。非本 diff 引入（首 hunk 仅改 schema 描述行），不追本卡，登记文档债。期望回执: 改期。

6. **bm25.rs:255 → `orig_snippet` 重复收集 `Vec<char>`**（`orig_hit_ranges` 内已收集一次）微冗余，可传切片复用。期望回执: 接受（低优先级，随手修）。

## 6. 与 SPEC 的逐条对照

**§2.0 总则**
- 「检索语义零变更」: **达成**。fan-out 索引域、QueryParser 字段集（两 parser 注册面均仍为 fn/tags/ocr/tx 四字段，diff 实证零改动）、BM25 评分路径零触碰；结构钉 + 快照钉 + T04 后验三重印证。
- 「R1 全审、无新公共 API、无 graph schema 迁移、无 unsafe、零新依赖」: **达成**（§2/§4 实证）。

**§2.1 schema（stored-only 原文域）**
- 字段名 `ocr_text_orig`/`transcript_text_orig`: **达成**（bm25.rs:28-30）。
- stored-only 形态: **达成且 API 断言属实**——`add_text_field(name, STORED)` 经 `From<StoredFlag> for TextOptions` = `indexing: None, stored: true`（text_options.rs:307-316，逐行核实）；零 postings/零 fieldnorm 机制由 `!is_indexed → continue`（indexer/segment_writer.rs:167-168）钉实。
- 「不进 QueryParser、不参与评分」: **达成**（parser 注册面 diff 零改动 + 结构钉 assert）。
- 追加在既有 6 字段之后（序号 0-5 存量对齐）: **达成**（schema():82-92 追加序）。
- 同源双写、同调用点一次成文、`None` 两域皆不写: **达成**（upsert bm25.rs:491-505 / upsert_batch :534-548——orig 写入嵌套于同一 `if let Some(ref ocr)` 分支内，无失同步窗口）。
- 存量兼容（旧 schema 打开/检索不受影响；`get_first(orig)` 取不到走回落链）: **达成**。关键守卫真实: orig `Option<Field>` 按**索引内嵌 schema** 解析（open_or_create:396-398 用 `index.schema()`；open_read_only:446-447 用 `index.schema()`），旧 6 字段 schema → None → 不写 orig，规避 `get_field_entry` 裸 Vec 下标（schema.rs:281-283 已核实）越界 panic；写侧守卫有 t03_legacy 测试实证（旧目录 upsert 不 panic）。

**§2.2 摘要四级回落链**
- ①原文定位: **达成**。词项面 = `sanitize_query` + `cjk_fan_out` 后 token 集去重（search:595-601 与 parser 消费的 pre_query 同源）；字面子串匹配、Latin 大小写不敏感（`to_lowercase` 逐 char eq）；RemoveLongFilter(40 字节) 同口径剔除（remove_long.rs:35-37 `len < limit` 保留 ↔ 代码 `len >= 40` 跳过，边界精确一致）。
- 重叠折叠 + sentinel `[[`/`]]`: **达成**——复用 tantivy `collapse_overlapped_ranges`（snippet/mod.rs：`sort_and_deduplicate_ranges` + `merge_overlapping_ranges`，自带排序，token 序 push 输入安全）；SNIPPET_OPEN/CLOSE 常量原样（:151-152），前端 esc 契约不动。
- 窗口 ≤200 字符、按字符边界截断: **达成**（`Vec<char>` 下标操作天然 char 边界安全；窗口内容 `we-ws ≤ 200`；t02 断言 `plain.chars().count() <= 200`）。40 字符前导为非契约量，注释已明示。
- ②扇出串定位原样保留、仅 orig 缺失触达: **达成**（`field_snippet` :174-194 零 diff；链序上仅在 ①None 后达）。
- ③head_truncate 原样保留 / ④None: **达成**（:199-205 零 diff；链尾自然 None）。
- 「①与②不混搭、逐字段回落」: **达成**（单侧输出恒为单一源，or_else 链保证）；跨字段优先级变化的登记缺口见 §5-建议 1。
- 下游透传面零形状变更: **达成**（highlight 仍 `Option<String>` sentinel 载荷；hybrid.rs 零 diff）。

**§2.3 性能承诺**: 归 T04 实测复核（存储 +9.5%/+10.3% ≤ +35% 达线；reindex +7.8% 触发 v0.1→v0.2 预算修订，用户裁决已闭；search 探针无回归）——非本卡验收面，无缺口遗留至 T03。

**§4 非目标**: 全部未越界（无分词器改造、无语义检索、无 by-hash、无前端/IPC/MCP 形状变更、无 filename/tags 改造、无降级兼容测试）✓。

**§5 拆分预案**: 未触发（399 ≤ 400 贴线通过）✓。

**§6-R1（API 查证转化 T03 首步）**: **完成且全部属实**——tantivy 0.26.2（Cargo.lock 实证）pinned registry 源码核实行号：text_options.rs:307-316 ✓ / schema.rs:281-283 ✓ / query_parser.rs:541-543 ✓ / tokenizer_manager.rs:61-72（limit(40) 实在 :62/:72）✓ / get_field 返回 Result（schema.rs:310）✓。漂移仅注记级（§5-风格 4）。无需回滚补 ADR。

---

**总结论: 可勾闭**（阻断 0 / 建议 3 / 风格 3）。建议 3 条交 WP 责任人逐条回执（接受/驳回/改期），workspace test 的 desktop e2e 环境态红灯建议以独立事项登记处置（根因 = 本机 demo 扩展缺 minisig 签名，与 T03 diff 零关联，证据见 §1 第一行）。

签字: AI-Reviewer GLM-5.3-Flash @ 2026-10-11
