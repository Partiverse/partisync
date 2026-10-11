# M11 里程碑报告

生成：`cargo xtask report M11`（骨架）+ 人工回填 · 版本: 1.0 · 日期:
2026-10-11 · 范围: `247566d..b82d090`（M10 G3 放行 #204 之后 → WP05-T05
勾闭 #245；42 commit）· 执行: GLM-5.3-Flash (ZCode) ·
骨架即占位约定（fill-in），非覆盖历史报告。

## 1. 范围与结果（对照 SPEC 汇总；范围变更记录）

| WP | 主题 | 结果 | 关键交付 |
|---|---|---|---|
| WP00 | 里程碑换轨 + 总纲 | ✅ | M11 提案 v0.3（partiverse D19 定位对齐，#207，挂 M10-WP00-T03）；T01 拍板落纲——**D19 桌面冻结令生效**（partisync-desktop 维护冻结、验证面转 CLI/partisd/MCP）+ M10 收官回填 + AGENTS.md 验收规则修订（#208）；T02 CI 卫生 upload_ack 基准去抖 best-of-3 最小 p99（#212） |
| WP01 | v0.2.0 发布线 + 许可终局 | ✅ | **rc.1 已发布**（tag `v0.2.0-rc.1`，2026-10-10，首个公开 GitHub Release）：workspace bump 0.2.0 + CHANGELOG/release.yml rc 口径（#217）+ macOS strip BSD 修复（#220）+ 发布报告 strip -81%/REPRODUCIBLE=yes/B1–B8 复跑（#222）+ A3 信任资产 README 对外化（#221）+ B1 by-hash 断言处置拍板 A（#223）；许可两次终局：ADR-0033 Apache-2.0 转正（#219，@lead 核签）→ **ADR-0034 AGPL-3.0-only 取代**（#231 @lead 核签，deny.toml license 同步 #233，ADR-0000 双修订注记）；GA(v0.2.0) 口径拍板文档 J1–J4 + 反馈窗口三空白推荐值（#224，**待用户三轴确认**）；T08 GA 前 CLI 全量实测 8 域矩阵 **PASS38/DEGRADED-EXPLICIT 1/FAIL0**，J1-③ 内部全链验收闭合（#242） |
| WP02 | partisd 常驻形态 + CLI 消费面 | ✅ | T01 SPEC + ADR-0032（partisd 前台+pidfile/loopback MCP 复用/索引单写多读，#209）；T02 索引只读打开 + P24 登记（#210）；T03 消费者只读化迁移（D5 锁冲突根因消除，#211，审查补档 #244）；T04/T05 partisd 实体化 + 三消费者并发 e2e + pidfile 跨平台（#213）；T06 关账回填（#214）。SPEC §3.1 勾 10/11——末项 T06 自指框漏勾（§5 债表行 2） |
| WP04 | 检索深化（stored 原文域） | ✅ | SPEC 起草（#225）→ 实施（bm25.rs 单面 399 行：ocr_text_orig/transcript_text_orig stored-only + 反 fan-out 摘要四级回落链 + 中文高亮修复，#229，审查补档 #243）→ T04 真库实测：存储 +9.5%/+10.3% ≤ +35% 达线、reindex +7.8% 超 v0.1 预算 <5% → **用户裁决 SPEC v0.2 ≤+10%**（#232/#234）→ search 探针无回归。**GA 硬判据 J2 闭合** |
| WP05 | 向量检索触发链（G10/T2/候选 4） | ✅ | T01 G10 AI workflow 编排四问评估 + T2 查询实证（#215）；T02/T03 + WP04-T01 三份评估件（#216）；T04 T1 语料积累——LCSTS 批量直插 **118076 叶** + MCP 面 98% 零命中实证 + 用户裁决「T1 语料满足」登记（#226/#227/#228）；T05 候选 4 重触发立项评估「有条件放行」+ WP06 SPEC 草稿同车（#235）+ 验收勾闭（#245） |
| WP06 | 记忆域语义向量检索 | ✅ | T02 ADR-0035 + P25 登记（graph 独立通道/fail-open 降级/异步伴生+崩溃回填，#236）；T03 schema v19 + usearch 包装 + blake3 key 域（#237）；T04 store 三腿接线（查询 RRF 融合/写入伴生嵌入/生命周期摘除）+ fail-open（#238）；T05 memory_search hybrid + MANIFEST blake3 填实 + 回填器（#239）+ 审查处置修复（#240）；T06 真模型实测 e2e——dim=512 全 512/预算达线/语义收益（#241）。SPEC §3 十一项验收未回填（§5 债表行 1） |

**范围变更**：①**D19 冻结令**（WP00-T01）——桌面端停新功能、验证一律
走 CLI/partisd/MCP 面（AGENTS.md 2026-10-09 修订落纲）；②许可终局两
次转向：ADR-0033（Apache 转正）被 ADR-0034（AGPL-3.0-only，核心下
游强制开源）取代，ADR-0000 治理总纲两条修订注记落档；③M11 无 WP03
（工作包跳号，提案 v0.3 六线编排如此）；④向量二期（候选 4）实施立
项由 WP05-T05「有条件放行」孵化出 WP06（原 WP04-T05 触发纪律首次实
际运转）。各 SPEC §4 非目标维持；无降级项。

**M11 新增 PR 数**：窗口 `247566d..b82d090` 共 **42 commit = 41 个
PR 合入 commit + 1 次无 PR 直推**（973b858，CHANGELOG ADR-0033 注记
补行，[M11-WP01-T03]）。41 PR 含：M10 尾款 2 笔（#165/#167，桌面
GUI 实操验证通道 10-09 闭合合入——M10 唯一实施面遗留清账）、Kubuntu
迁移交接 + drop-caches 复测回填 2 笔（#205/#206）、M11 任务 PR 37 笔
（#207–#245 内，#218/#230 为 CLOSED 未合入的取代重开）。全部
squash 合入，无 OPEN 遗留。

**CI 现场核验**（2026-10-11，41/41 PR 头 commit 逐个 `gh run list
--commit` 过一遍）：**41/41 各有 ≥1 个 success run**；非 success run
共 7 个，逐笔归因：deny 红 2（#231/#232——AGPL 许可变更先于
deny.toml 同步，#233 闭环后绿）；test 红 3（#235 macos / #236 ubuntu /
#242 ubuntu，均为单 job 瞬态，同 commit 配对 gate 绿、main 后续 run
绿；豁免登记见 §5 债表行 7）；in_progress 2（#244/#245 docs-only，
合入时 CI 未跑完，非红灯）。main HEAD（b82d090 前驱 cec709f）run
38145282095 success。

## 2. KPI 达标表（基准报告链接）

| 指标 | 记值 | 证据 |
|---|---|---|
| rc.1 发布质量 | strip -81% / REPRODUCIBLE=yes / B1–B8 复跑全绿（B1 按断言口径 v2） | docs/reviews/M11-WP01-release-report-rc1.md（PR #222） |
| GA 前 CLI 全量实测 | 8 域矩阵 PASS38 / DEGRADED-EXPLICIT 1（hybrid 无模型诚实回退）/ FAIL0；J1-③ 闭合 | docs/reviews/M11-WP01-t8-cli-fulltest-evidence.txt（PR #242） |
| stored 原文域代价 | 存储 +9.5%/+10.3% ≤ +35% 达线；reindex +7.8%（v0.1 预算 <5% 超线 → 用户裁决 SPEC v0.2 ≤+10%）；search 探针无回归 | docs/reviews/M11-WP04-t4-eval-evidence.txt（PR #232/#234） |
| 候选 4 T1 语料 | memory 域 **118076 叶**（LCSTS 实测入库）+ MCP 检索面 98% 零命中 → 用户裁决「T1 语料满足」 | docs/reviews/M11-WP05-t1-corpus-evidence.txt + #228 判定登记 |
| 记忆域向量（真模型） | BGESmallZHV15 dim=512 十句全 512；逐句延迟真中位 3.52ms；预算达线 + 语义收益 e2e | docs/reviews/M11-WP05-t05-vector-reinitiation-eval.md + M11-WP06-t6-eval-evidence.txt（PR #241） |
| upload_ack 基准去抖 | best-of-3 最小 p99 口径（WP00-T02 CI 卫生） | docs/tasks/M11-WP00-T02-ci-hygiene.md（PR #212） |

注：`docs/reports/bench/` 窗口内零变更（git log 实证）；上表全部为
SPEC/任务卡/证据件工件在案记值，本报告未重跑基准。WP04 reindex 预
算超线经用户裁决修订（SPEC v0.2）后达线，非静默放宽。

## 3. 测试证据（覆盖率/属性测试/变异分数/模糊时长/混沌/互操作）

- **CI**：全程 8 jobs（fmt/clippy/deny/interop/task-ids/test×2/gate）；
  41/41 merged PR 头 commit 各有 ≥1 success run（现场逐个核验，非
  success 7 run 逐笔归因见 §1）；main HEAD run success。
- **新增属性测试/不变量**（docs/tests/properties.md，均登记先于测试）：
  **P24** search::bm25 索引并发三臂（只读打开无写锁新建/与活跃
  IndexWriter 并存 reload 可见/≥3 只读实例并发，M11-WP02-T02 登记，
  探针 crates/partisync-index/tests/m11_wp02_readonly.rs）；**P25**
  记忆域向量通道四臂（维度恒 512/key 仅由 memory_id 派生 blake3 前
  8B 幂等/tombstone-GC 后不可召回/降级显式回退 BM25+标记，M11-WP06-T02
  登记，探针 crates/partisync-graph/tests/wp06_memory_vector.rs）。
- **探针面**：WP02 partisd 守护进程 + 三消费者并发 e2e（#213）+
  wp06_surface partisd 面（#239 批次）；WP06 向量全链
  wp06_memory_vector 771 行 + 真模型 e2e（#241）；WP04 检索等价双钉
  （结构钉 orig 不进 parser + 快照钉 4 文档×5 查询容差 1e-6，#229）
  + 旧 schema 回落链 fixture。
- **互操作**：musl/双平台由 CI test(ubuntu+macos) 兜底；rc.1 产物经
  B1–B8 复跑（#222）；T08 八域实测覆盖 CLI/partisd/MCP/hub/export/
  fuse 拒绝面（fuse 拒绝面 = mkdir Permission denied 直证）。
- **本机全量复跑**（2026-10-11，`cargo test --workspace
  --no-fail-fast`，三轮）：**729 passed / 0 failed / 13 ignored**（终
  验两轮零失败；中间一轮出现 1 例未复现瞬态失败、未捕获名，前后两轮
  同面零失败，非主干回归信号）。13 ignored 与 M10 完全同集（8 基准/
  显式跑 + 5 doctest），零新增。M10 期环境耦合样例
  `mcp_call_real_sidecar_ext_list` 本期根因定位深化：本机装有**无签
  名** demo 扩展时 [P21] verify-mandatory 拒载 → dir:null → 断言红；
  已恢复设计跳过态（无 extensions 目录即跳过）。**残债**：
  scripts/install-demo-ext.sh 仍只装 wasm+json 不带 `.minisig`，装了
  必被拒（M9 安全基线后的脚本脱节，§5 债表行 4，闭合 = 脚本修或生
  产钥签名，key-custody AI 不可代办）。

## 4. 安全（cargo audit / deny / unsafe 增量 / 外部审计）

- **cargo deny**：`cargo deny check` → advisories/bans/licenses/
  sources **4 项 ok**（2026-10-11 本机复核）。唯一 warning =
  RUSTSEC-2026-0253 `advisory-not-detected`（deny.toml:62 既有登记项
  未被当前依赖树命中，信息性非失败；撤销条件 tantivy/reed-solomon
  跟进后撤销，沿 ADR-0024 amend 登记）。
- **cargo audit**：本机无 cargo-audit 二进制（2026-10-11 如实登记，
  不假造数字）；advisory 面由 **cargo-deny advisories（同一 RustSec
  源，fetch 走代理后绿）+ CI deny job** 覆盖。deny.toml 窗口内变更 4
  笔逐笔核验（#217 bump 0.2.0 / #219 ADR-0033 / #231 ADR-0034 /
  #233 AGPL license 同步）——**ignore 名单零增量**，无静默新增。
- **新依赖**：**workspace 顶层 pin 零新增**（usearch 2.26 为既有 pin
  Cargo.toml:53，fastembed =7.0.1 沿 ADR-0017/0018）；WP06 新增 crate
  依赖边 1 条（partisync-graph → usearch/fastembed 既有 pin），由
  **ADR-0035** 承载登记（#236），crate Cargo.toml 内登记注记在位。
  Cargo.lock 变更与依赖边解析一致。
- **unsafe**：workspace `forbid(unsafe)` 维持；M11 新增代码零 unsafe。
- **许可面（里程碑级合规事件）**：基座许可经 ADR-0033（Apache-2.0
  转正，#219）→ **ADR-0034 AGPL-3.0-only**（#231，@lead 核签，核心
  下游强制开源）两次终局，deny licenses 检查同步绿（#233）；仓库
  LICENSE 与 Cargo 面一致。外部审计双义务（M2-D1/OSCP + Mimosa 完整
  结论）延续：资金回笼触发，条件行维持，本期无新增扫描义务。

## 5. ADR 清单与债务登记

**M11 期间新增/修订 ADR**：

| ADR | 主题 | 状态 |
|---|---|---|
| ADR-0032 | partisd 常驻形态（前台+pidfile）/loopback MCP 服务面复用/索引单写多读 | 接受（合入=批准，#209） |
| ADR-0033 | 许可终局与商业分层——Apache-2.0 转正 + hub 商业扩展区 | 接受（#219，@lead 核签）；**终局条款被 ADR-0034 取代** |
| ADR-0034 | 基座许可 AGPL-3.0-only（核心下游强制开源） | 接受（#231，@lead 核签；deny 同步 #233） |
| ADR-0035 | 记忆域语义向量检索面——graph 独立通道/fail-open/异步伴生+崩溃回填 | 接受（合入=批准，#236） |
| ADR-0000（修订） | 治理总纲追加 0033 复议「临时决策」条款 + 0034 再议取代注记 | 已登记 |

**债务台账**（M11 新增/滚入；既有条件行维持见 §8-6）：

| 债 | 来源 | 状态/去向 |
|---|---|---|
| 1. **WP06 SPEC §3 十一项验收未回填** | 本报告对账发现 | ⏳ 实施/实测 PR（#237–#241）已合入、T06 证据件在库，勾闭为纯回填——另立小任务逐项对证据勾闭 |
| 2. **WP02 SPEC §3.1 末项「T06 关账回填」未勾** | §7 抽查发现 | ⏳ #214 即 T06 关账 PR，自指框漏勾——一行回填，随行 1 一并 |
| 3. **WP05 无正式 SPEC 文件** | §7 抽查 | ⏳ 形态债：以 roadmap §3-WP05 拍板行 + M10-WP04-memory-vector-eval §6 触发纪律替代，链路可追溯（trace PASS）但不符合「无已批准 SPEC 不写实现」字面——M12 起触发纪律类任务补 SPEC 或修订规程明确豁免形态 |
| 4. **install-demo-ext.sh 与 [P21] 验签基线脱节** | 本报告 §3 定位 | ⏳ 脚本只装 wasm+json 无 `.minisig`，装了必被 verify-mandatory 拒收——另立任务候选：脚本内签名腿（需生产钥，key-custody）或 skip 守卫校验 minisig 存在性 |
| 5. **GA 三轴确认 + rc-feedback 渠道登记** | M11-WP01-T07 §0 | ⏳ 三个确认位全空（判据轴/窗口轴/序列轴），确认前不 tag v0.2.0 不 publish（红线）；确认后起草窗口开启公告 + `rc-feedback` 标签渠道登记——待用户指令 |
| 6. **J1 窗口计满 + J4 B1–B8 复跑** | cadence J1/J4 | ⏳ rc.1 发布 2026-10-10，≥14 自然日 → 最早 **2026-10-24** 关窗；J4 复跑在 GA tag 前执行（B1 按 v2 口径，无口径障碍） |
| 7. **PR-run 瞬态红灯豁免未登记**（#235 macos / #236 ubuntu / #242 ubuntu test） | §1 CI 核验 | ⏳ 三笔均单 job 瞬态、main 后续绿；豁免理由随本报告 §1 归因登记，不逐笔补档案 |
| 8. **M11-WP01-T08 Spec trailer 指针不精确** | §7 抽查 | ⏳ 指向谱系文档而非判据所在文档（ga-cadence J1 行）——下次触碰该校准时修正 |
| 9. **WP04-T03 审查改期项**（混合字段回落测试 + SPEC O(窗口) 口径更正 + 三条风格） | #243 审查回执 | ⏳ 已逐条回执登记（#243 PR 正文）；改期项随后续触碰处置 |
| 10. **向量二期 T4 窗口 + 候选 4 复裁** | WP05-T05 评估件 | ⏳ 实施位随 partiverse S4（≈2027-03 前）窗口对齐；复裁属用户决策 |
| 11. 既有条件行维持 | M10-report §5 | NB-WP05-1 真实外接 AS 端到端（资源到位）/ 外部审计双义务（资金回笼）/ drop-caches 复测（已回填 #206 清账）/ 示例扩展生产钥签名（持有人，与行 4 合流）/ M12 立项（roadmap 滚动项） |

## 6. AI 使用披露（自动统计）

- 挂接 M11-\* 任务的提交数：**73**（`xtask report` 自动统计，
  `git log --all` 含 squash 前分支提交的重复计数）；范围
  `247566d..b82d090` 实测 **42 commit = 41 个 PR 合入 commit + 1 直推**
- 任务数：**29**（xtask 按 Task-ID trailer 去重，M11-\* 前缀）；工作
  包分布：M11-WP00, WP01, WP02, WP04, WP05, WP06（无 WP03，范围变更
  ③）；范围全量唯一 Task-ID 33 个（含 M10 尾款 #165/#167 与跨界件）
- AI 辅助提交（AI-Assist）：范围 42/42 commit 逐个校验零缺失
  （xtask 口径 72/73 的 1 笔差异为直推补行 973b858 无 trailer，正文
  挂 [M11-WP01-T03]），全部挂 GLM-5.3-Flash (ZCode) 披露；单会话单
  任务 + 极简指令自主推进模式
- AI-Review trailer：**6 个 commit**（#223/#224/#225/#226/#227/#242）
  + docs/reviews/ 独立对抗审查工件 2 件补档（#243 WP04-T03 / #244
  WP02-T03——后者为 §7 抽查发现的缺口补齐）；M11-\* 审查/证据工件
  共 15 件在库（`ls docs/reviews/M11-*`）
- 人工终审提交（Reviewed-By）：**3/42**（#219/#221/#231，WP01 许可
  线两次终局的 @lead 核签）——与运行授权一致：用户已授权 CI 绿 +
  对抗评审通过即合入，人工终审以 PR 批准 + 关键拍板批示执行
  （D19 冻结令 #208、T1 语料满足 #228、B1 拍板 A #223、reindex 预算
  裁决 #234、许可两次终局 #219/#231 等均为用户显式指令）
- 本报告本体由 GLM 起草（M11-WP99-T01），G3 三签不在其代签范围

## 7. 抽查审计记录（xtask trace ×5 抽样任务）

| 任务 | trace 结果 | 判定 |
|---|---|---|
| M11-WP02-T03 | 1 commit（838d0266，#211）· SPEC §3-T03 ✓ · 卡 4/4 勾 · P24 三探针 + D5 直证 | PASS（审查工件缺口 → 补档 #244 收口） |
| M11-WP04-T04 | 6 commits（#232/#233/#234）· SPEC v0.2 预算裁决登记 ✓ · 真库实测证据件 ✓ | PASS（#232 首跑 deny 红系 #233 同因，闭环） |
| M11-WP05-T05 | 1 commit（be43e8c2，#235）· 无正式 SPEC（形态债行 3）· 卡 6/6 未勾 → 勾闭 #245 · #235 macos test 瞬态红 | PASS（两断链均于关账窗口收口） |
| M11-WP06-T04 | 1 commit（303842d6，#238）· SPEC 三腿/§2.4 fail-open/P25(c)(d) ✓ · G1 登记先于测试 ✓ | PASS（trace 带入的 ccdbb5d 系 T05 PR 正文引用字符串误匹配，工具特性非断链） |
| M11-WP01-T08 | 1 commit（77b54e1，#242）· 唯一带 AI-Review trailer · 8 域证据件 ✓ · J1-③ 闭合登记 ✓ | PASS（Spec trailer 指针不精确 → 债表行 8；ubuntu test 瞬态红 → 行 7） |

5/5 追溯链完整（Task-ID → SPEC/判例 → commit → PR → 证据件 → 回填；
铁律 2 成立）。抽查发现 6 处断链/缺口，**3 处已在本关账窗口收口**
（WP02-T03 审查补档 #244、WP05-T05 卡勾闭 #245、WP04-T03 勾闭先例
#243），**3 处登记 §5 债表**（行 2/3/8）。另两条全仓模式备注：各 PR
均无 GitHub 人工 review 记录（人工终审以用户裁决/审批文本登记于卡片
与提交，沿 M10 披露口径）；`xtask trace` 按 Task-ID 字符串全文匹配，
兄弟 PR 正文引用会误计入（工具特性，非断链）。

### §7.1 审计清单（必要项）

- [x] fmt/clippy 本地对 main HEAD 绿（2026-10-11 实跑）；CI main
      HEAD（cec709f）run 38145282095 success
- [x] `cargo test --workspace --no-fail-fast` 本机全量三轮：
      **729 passed / 0 failed / 13 ignored**（终验两轮零失败；中间
      一轮 1 例瞬态未复现，如实注记 §3；13 ignored 沿 M10 同集零新增）
- [x] cargo deny 绿（4 项 ok；窗口内 deny.toml 变更 4 笔逐笔核验为
      许可/bump，ignore 名单零增量）
- [x] cargo audit：本机无二进制如实登记；advisory 面由 cargo-deny
      advisories（同 RustSec 源）+ CI deny job 覆盖，无静默新增
- [x] bench：M11 无新基准任务（docs/reports/bench/ 零变更）；WP04
      reindex 预算超线经用户裁决 SPEC v0.2 ≤+10% 达线，无未裁决回退
- [x] 抽查 5 任务追溯链完整；发现的断链 3 处收口、3 处登记债表
- [x] AI 披露 100%（42/42 + 本报告本体披露）

## 8. 下一阶段建议（GA 窗口 + M12 方向）

1. **GA 三轴确认**（用户，M11-WP01-T07 §0 三个确认位）：判据轴
   （J1–J4，含候选 4 T1 观察项定位——两源冲突事实上已由 #228 用户
   裁决解消，cadence 文档待回填）/ rc 反馈窗口轴（≥14 天 + 叠加门
   槛三空白落锤）/ GA 发布序列轴；确认后起草窗口开启公告 +
   `rc-feedback` 标签渠道登记（GitHub Issues + Release 反馈区推荐
   案在档）；
2. **GA 序列执行**（cadence §3.2，三轴确认后）：release.yml 产物名
   去 rc 段 + CHANGELOG `[0.2.0]` 节（PR）→ J1 窗口计满（最早
   2026-10-24）+ 零未处置 P1 + issue 全处置 → B1–B8 复跑（B1 v2 口
   径）→ 用户终审 → tag `v0.2.0` → draft Release → 用户终审
   publish → GA 发布报告落档；
3. **SPEC 回填小尾巴**：WP06 §3 十一项逐证据勾闭 + WP02 §3.1 T06 自
   指框（债表行 1/2，纯 docs 小任务）；
4. **任务候选**：install-demo-ext.sh 验签腿（债表行 4）；WP05 SPEC
   形态债的规程化处置（行 3）；
5. **M12 立项**：roadmap 滚动项 + 候选 4 复裁窗口（T4 ≈2027-03 前，
   用户决策）+ AGPL-3.0-only 转轨后的合规跟进（若 hub 商业扩展区启
   动，沿 ADR-0034/commercial-actions-backlog）；
6. **条件行维持**：NB-WP05-1 真实外接 AS（资源到位）/ 外部审计双义
   务（资金回笼）/ 示例扩展生产钥签名（持有人，与债表行 4 合流）/
   向量二期实施窗口（partiverse S4）。

## 放行签字（G3）

<!-- 三签由关账任务回填（判例 M6-WP99-T01 / M8-WP99-T01 /
     M9-WP99-T02 / M10-WP99-T02——用户指令后由关账任务落档，报告不
     代签）。 -->

- [ ] 架构负责人：
- [ ] 评审人：
- [ ] 安全负责人（M2/M4）：
