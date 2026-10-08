# M10 里程碑报告

生成：`cargo xtask report M10`（骨架）+ 人工回填 · 版本: 1.0 · 日期:
2026-10-08 · 范围: `67c339f..805a800`（M9 关账 PR #151 之后 → M11 提案
v0.2 #202；45 commit）· 执行: GLM-5.3-Flash (ZCode) ·
**历史版本**: 本文件由 M10-WP99-T01 骨架（`xtask report M10`）正式化
回填，骨架即占位约定（fill-in），非覆盖历史报告。

## 1. 范围与结果（对照 SPEC 汇总；范围变更记录）

| WP | 主题 | 结果 | 关键交付 |
|---|---|---|---|
| WP00 | 总纲 + 台账 | ✅ | 开发蓝图提案 v0.1→v0.2（PR #152/#202，M11 五线候选落档待拍板）+ 总纲 SPEC（PR #155，T01/T02 同卡：主题方向用户拍板「加强前端 GUI 功能加速」——原 α2/β2 后置重排 + M9 尾款承接口径拍板 8 项）；T03 滚动登记：记忆编辑/删除 UI 条件触发承接位（WP04 §4 裁定）+ iroh-blobs 0.103.0→0.103.1 yank 补丁（PR #173，上游事故响应） |
| WP01 | GUI 检索可用性与信息透出（「形同虚设」修复） | ✅ | T01–T05 全合入（PR #156/#159/#160/#161/#162/#163）：SearchHit 增 filename（graph 反查回填，bm25/hybrid 双路径）+ tantivy SnippetGenerator 命中词定位摘要（sentinel→`<mark>` 安全渲染，中英双语 e2e）+ 结果-详情联动 + 扩展名 chips 客户端过滤 + index_stats 空态引导三分支 + 规模徽标；SPEC §3 唯一未勾 = GUI 截图行（见 §5 债表行 5） |
| WP02 | 记忆面板 UX 深化 | ◐ T02 合入，T01/T03 OPEN | T02 详情联动展开 + clipboard 复制（PR #166 合入，四探针 + GUI 五帧归档）；T01（#165 form 防重载 + 表头排序）/ T03（#167 相对时间四档 + tag chips）两 PR **OPEN 挂「待 GUI 验证」label**（桌面端硬性规则：环境不可得保持 OPEN 不合入）——M10 唯一未收官 WP，SPEC §3 T01/T03/GUI 截图/三件套四行未勾 |
| WP03 | 桌面深耕二期 + T04 GUI 承接 | ✅ | T01–T03 + 关账 T04（PR #168–#171 + #187）：M9-WP03-T04 GUI 三态验收承接（篡改安全流 + 全 7 tab 巡检 10 帧 + M9 双 SPEC 回填）+ 详情面板 UX（×/Esc 关闭 + mtime 补全 + content_id/副本路径复制）+ 浏览列表 UX（表头客户端排序 + mtime 相对时间）；SPEC §3 全勾、批准日期 2026-10-06 落记；巡检新债 D5（sidecar 持 tantivy 写锁）登记 §6 |
| WP04 | 记忆层二期（α2 后置） | ✅ | T01–T06（PR #172/#174/#176–#181）：tombstone 软删不动根（墓碑留承诺集，ADR-0029 修订登记 2026-10-06）+ update 换身份/复活 + ("memory","delete") 同步臂 + memory GC（过保留期硬清除 + 根重算 + CLI memory-gc）+ MCP memory_update/memory_delete + schema v18 content_chunk 清单落表 + 重组读面 + reindex 重组回退（**NB5 清偿**，P22 登记）+ 语义向量检索条件评估件（拍板暂不实施，触发条件落锤）+ T06 关账盘点；SPEC §3 全勾（头部批准日期未落记，见 §5 债表行 5 注） |
| WP05 | MCP 2.0 正式远程化（β2 后置） | ✅ | T01–T06（PR #182–#185/#188–#195/#196）：接线评估件六问必答 + ADR-0031 五项落锤（mock AS/StreamableHttp/jsonwebtoken RS/rustls/scope 步进）+ gateway 远程面（PRM 真实化/Origin·协议头校验/fail-closed 401 墙/TLS 启动守卫）+ P23 授权矩阵 13 例 + 11 工具透传（list 逐字段一致/handle 绑 T-R5 身份）+ mock AS 全链 e2e；**NB3/G8 主体清偿**；真实外接 AS 端到端 = NB-WP05-1 条件触发维持；SPEC §3 全勾、批准日期 2026-10-08 落记 |
| WP06 | 发布节奏（压轴，全 docs 零代码） | ✅ 准备面 | T01–T05（PR #197–#201）：rc/0.2.0 口径拍板文档三轴推荐（tag `v0.2.0-rc.1` + workspace 0.1.0→0.2.0 **bump 49 处实测清单** + REPRODUCIBLE=no 处置建议）——**推荐待用户确认，确认前不 tag 不 publish**；双窗口登记落档（drop-caches 复测挂 Linux 台架 + 示例扩展生产钥签名挂持有人）+ Mimosa deep 重扫登记；SPEC §3 全勾、批准日期 2026-10-08 落记；发布执行未开工（待用户确认口径后另立） |

**范围变更**（WP00 §3-T03 滚动登记）：①记忆编辑/删除 GUI 登记
条件触发——tombstone 引擎能力随 WP04 就绪后，GUI 立项以 T03 滚动行为
承接位（WP04 §4 裁定，WP02/WP03 §4 对读）；②WP05 实装沿铁律 3 拆
堆叠 PR（#185/#186→#190/#188/#189→#191 等，契约面零变更）；无降级
项；各 SPEC §4 非目标维持。

**M10 新增 PR 数**：#152–#202 窗口内 **merged 44 个**（全经 `gh pr
merge --squash`；其中 #166/#174/#176/#183/#184 五笔 squash 主题经
编辑未带 `(#N)` 后缀——实证：main 上 commit 均为 squash 新建（≠ PR
head SHA），与 PR 一一对应经 gh 记录核验；#166 的 head c67f9b0 即
T02 trailer 所指「合并提交」= **分支侧**解 ui_hardening.rs 冲突的
merge，非 main 直推；#188/#189/#191 三堆叠聚合入 #191 squash 单
commit 1445482）；**OPEN 2 个**（#165/#167，均挂「待 GUI 验证」）；**CLOSED
2 个**（#175/#186，堆叠 PR 被后继取代重开，工作未废弃）。窗口内 45
commit = 42 个 M10 任务提交 + 3 个 M9 尾款（#153 G3 三签 / #154
release.yml / #157 v0.1.0-beta 发布报告——M9 发布执行同窗口落地，沿
M9-report「同窗口放行」注记）。**CI 现场核验：47/47 merged PR head
commit run conclusion=success**（`gh run list --commit <sha>` 逐个
过一遍，含 3 笔 M9 尾款，无红灯合入）；main HEAD（805a800）run
37790713351 success。

## 2. KPI 达标表（基准报告链接）

| 指标 | 记值 | 证据 |
|---|---|---|
| 远程 MCP mock AS 全链 e2e | PRM → 401 挑战 → token → initialize → tools/list → tools/call 全栈 PASS（11 工具 list 与 stdio 逐字段一致；P23 矩阵 13 例全绿） | docs/specs/M10-WP05.md §3（T05 PR #195） |
| 记忆 tombstone 语义探针 | 删除三路径不可见 / update 换身份旧 id 墓碑 / 复活 deleted=0 / 双端 bisync 收敛墓碑一致，全 PASS | docs/specs/M10-WP04.md §3（T01/T02 PR #174–#178） |
| CAS 内容重组（NB5） | 大文件 content_chunk 清单 ↔ chunk_root 对账 + blake3(拼接)==content_id roundtrip + 缺块显式 Err 两类注入 | docs/specs/M10-WP04.md §3（T04 PR #179，P22） |
| GUI 检索透出 | filename 卡 + 命中词 `<mark>` 高亮中英双语 e2e（`t01_search_hit_carries_graph_filename` / `t02_highlight_centers_on_hit_terms`） | docs/specs/M10-WP01.md §3（T01/T02 PR #156/#159/#160） |
| rc/0.2.0 bump 改动面 | 49 处 = 14 文件（13 成员 Cargo.toml + 根）实测清单（**推荐待用户确认**，未执行） | docs/reviews/M10-WP06-release-cadence.md §1.1/§2 |

注：M10 无新基准任务（`git log 67c339f..805a800 -- docs/reports/bench/`
为空）；上表全部为 SPEC/任务卡工件在案记值，本报告未重跑基准。

## 3. 测试证据（覆盖率/属性测试/变异分数/模糊时长/混沌/互操作）

- **CI**：全程 8 jobs（fmt/clippy/deny/interop/task-ids/test×2/gate）；
  M10 全部 44 个 merged PR head commit run conclusion=success（现场
  逐个核验），无红灯合入；main HEAD（805a800）run 37790713351
  success（task-ids 按 PR-only 设计 skipped）。
- **新增属性测试/不变量**（docs/tests/properties.md:29-30）：P22
  CAS 内容重组承诺（落表完整/可重组/缺块显式失败三臂，M10-WP04-T04
  登记）+ P23 远程 MCP 授权 fail-closed（401 挑战形状/scope 403
  step-up/无绕过通道，含他 resource 受众 token 注入，M10-WP05-T04
  登记先于测试代码）。
- **探针面**：WP05 `wp05_mcp_remote`（PRM 形状/TLS roundtrip/Origin/
  fail-closed/token 矩阵 13 例/G-3·G-6 协议头与未知方法/tools list
  一致性/mock AS 全链 e2e/handle 绑身份）；WP04 wp04 探针三路 + GC
  探针（行数减/根值变/verify ok/FTS 收缩）+ P22 三臂；WP01
  filename/高亮/联动/过滤/index_stats 五组 e2e+探针；WP02-T02 四探针
  （详情展开/复制反馈/列表截断/验证徽章共存）；WP03 全 7 tab 巡检
  10 帧 + 篡改安全流留痕。
- **互操作**：musl/双平台由 CI test(ubuntu+macos) 兜底；M10 无容器
  e2e 任务（M9-WP01 容器挂载判例本期无对应需求）。
- **本机全量复跑**（2026-10-08，`cargo test --workspace
  --no-fail-fast`）：111 个测试二进制（98 单测/集成 + 13 doctest），
  **683 passed / 0 failed / 13 ignored**，exit 0 零失败。13 ignored
  均有设计理由：8 例基准/显式跑测试（M5/M8 基准与 100k 仿真、需下载
  嵌入模型权重、M2 KPI 分钟级搭建，按 `--ignored` 显式运行口径）+
  5 例 doctest（iroh_blobs 网络示例两例等）。M9 期唯一失败样例
  `mcp_call_real_sidecar_ext_list` 本期 **ok**——其失败前提（本机
  装有断签 demo 扩展）已不在（`~/.partisync/extensions/` 现不存在，
  测试走无预置文件路径）；**示例扩展断签窗口本身未闭合**（§5 债表
  行维持，闭合动作 = 持有人生产钥签名，不因本机环境变化而勾销）。

## 4. 安全（cargo audit / deny / unsafe 增量 / 外部审计）

- **cargo deny**：`cargo deny check` → advisories/bans/licenses/
  sources **4 项 ok**（2026-10-08 本机复核）。唯一 warning =
  RUSTSEC-2026-0253 `advisory-not-detected`（deny.toml:61 既有登记
  项未被当前依赖树命中，信息性，非失败）。
- **cargo audit**：`cargo audit --no-fetch` → **10 vulnerabilities +
  8 allowed warnings**。10 项**全部有 ADR 登记**：7 项沿 M9 判例
  （rsa-0071→ADR-0020；h2-0258 + webpki-0098/0099/0104→ADR-0021；
  wasmtime-0315/0316→ADR-0025 修订 6）；**3 项为 M9 报告后新公告**
  （wasmtime-0325/0326/0327，同一修复波次，版本仍 47.0.4 线内无补
  丁）→ ADR-0028 承载（状态**接受**，2026-10-03 用户批准落记；漏洞
  路径均不可达：async/gc/exception-handling feature 未启用 + WIT 面
  无 resource/record + 零 async-lift 用法，撤销条件双触发登记）。
  **deny.toml M10 期间零改动**（末次变更 c93f2f3 = M8-WP07-T02 PR
  #102，早于本窗口；ignore 增量零）。
- **新依赖**：rustls 0.23 + tokio-rustls 0.26 + rcgen 0.14（仅 dev
  测试证书）+ jsonwebtoken `=11.1.0`（aws_lc_rs feature，MSRV/许可
  查证留痕）——全部由 **ADR-0031**（M10-WP05-T02 落锤，PR #184）
  承载，deny 四项 ok 随 T03/T04 批次实证；iroh-blobs 0.103.0→
  0.103.1 为上游 yank 补丁替代（PR #173，非新增依赖）。
- **unsafe**：workspace `forbid(unsafe)` 维持；M10 新增代码零 unsafe。
- **Mimosa**：起草期 **deep 重扫登记落档**（M10-WP06-T04，PR #200，
  docs/reviews/M10-WP06-mimosa-rescan.md）：五要素齐备（scanId
  `scan-2026-10-08T06-53-40…`、seal `f9a368e3…`、227 文件 0 解析
  失败、全程无 enobufs、run status=inconclusive 原文登记）；唯一
  HIGH = xtask `git()` 污点链，与历史四扫描 anchor 逐字同源，误报
  处置维持 M6-report §5.1 签收；零新增 advisory anchor。**登记不
  构成安全放行结论**；完整扫描结论随外部审计窗口（资金回笼触发）
  一并出具。

## 5. ADR 清单与债务登记

**M10 期间新增/修订 ADR**：

| ADR | 主题 | 状态 |
|---|---|---|
| ADR-0031 | MCP 2.0 正式远程化接线五项决策（mock AS/StreamableHttp 传输件/jsonwebtoken RS 验签/rustls TLS/scope 步进） | 接受（PR #184） |
| ADR-0029（修订登记） | 墓碑叶口径：墓碑行留承诺集、软删不动根；GC 硬清除动根对照；memory_count = 含墓碑承诺集规模 | 已登记（2026-10-06，WP04-T01 触发「重新评估条件」第 3 条） |

**债务台账**（M10 清偿 3 笔 + 延续/新增 11 笔）：

| 债 | 来源 | 状态/去向 |
|---|---|---|
| NB2 记忆层生命周期缺口 | M9-report §8-5 | ✅ **主体已清偿**（WP04，PR #174/#176–#178：软删墓碑不动根/update 换身份/复活/GC 硬清除/同步 delete 臂/MCP 工具面）；语义向量检索沿 T05 评估件拍板「条件不成熟暂不实施」（PR #180）转条件触发；编辑/删除 GUI 维持条件触发（WP00 §3-T03 承接位） |
| NB5 CAS 内容重组缺口 | M9-report §5 | ✅ **已清偿**（WP04-T04，PR #179：schema v18 content_chunk + 重组读面 + reindex 回退，P22）；存量无清单数据不可回填、read_errors 边界诚实维持（WP04 §4） |
| NB3 MCP 2.0 正式远程化（G8 后半） | M9-report §5 | ✅ **主体清偿**（WP05，PR #182–#196：ADR-0031 + 真实 OAuth 2.1 RS 验签 + rustls TLS + 11 工具透传 + mock AS 全链 e2e）；**NB-WP05-1** 真实外接 IdP 端到端 = 条件触发维持（WP05 §4，mock AS 本地交付为界，诚实登记不伪造端到端） |
| **WP02 T01/T03 收官** | WP02 SPEC §3 | ⏳ 两 PR **OPEN**（#165 form 防重载+表头排序 / #167 相对时间四档+tag chips），均挂「待 GUI 验证」label（AGENTS.md 桌面端硬性规则：GUI 实操验证后方可合入）；SPEC §3 T01/T03/GUI 截图/三件套四行未勾；WP00 §1-WP02 行未收官——M10 唯一实施面遗留 |
| **WP01 SPEC §3 GUI 截图行未勾 + WP04 SPEC 头部批准日期未落记** | 本报告对账发现 | ⏳ 小尾巴回填：截图行 T02/T03/T04 已 ✅ 注记归档、T05 注记「部分」，但 docs/screenshots/ 实存 `M10-WP01-T05-*.png` 5 帧（M10 截图共 45 帧在库）——注记偏保守未回勾；WP04 §3 全勾 + WP00 §1 已收官但头部「批准日期: 待合入」（WP05/WP06 判例后才出现落记动作）。均为口径差异非交付缺口，随 WP02 收官 PR 一并回填 |
| **drop-caches 冷缓存复测** | M8-WP07-bench §2（M9-report §5 改挂） | ⏳ **窗口登记落档**（M10-WP06-T03，PR #199）：RELEASE-CHECKLIST-beta.md B1 注记（挂 M10-WP06 复测窗口 + 非阻塞依据：读路径自 M8 未改、冷基线 1567 MiB/s 维持）；触发 = Linux 台架可得 |
| **示例扩展生产钥签名** | WP04-SPEC §6-R5（M9-report §5） | ⏳ **窗口登记落档**（M10-WP06-T03，PR #199）：EXT-SIGNING.md「示例扩展签名窗口」节（对象 `examples/extensions/demo_ext.*` 实测无 `.minisig`、闭合动作 = 持有人生产钥签名、key-custody AI 不可代办）；未闭合行为 = 强制验签下装载拒绝维持（P21 不变，非缺陷回归）；本机实证影响 = desktop e2e `mcp_call_real_sidecar_ext_list` 失败（环境耦合，CI 无预置文件自跳过） |
| **scanner_enobufs 覆盖边界** | M7-report D-S2 | ⏳ 披露性挂账维持：M10-WP06-T04 deep 重扫全程无 enobufs（227 文件）；工具侧动态派发不可达的能力边界非项目可修，结论仍随外部审计窗口 |
| **REPRODUCIBLE=no（v0.1.0-beta B7）** | M9-WP06-T03-release-report.md:24 | ⏳ 处置建议随 rc 口径落档（M10-WP06-T02，PR #198）：rc 期优先试 `CARGO_PROFILE_RELEASE_DEBUG=false`（去 debuginfo=路径相关性主源）评估实验；不达标则 rc 维持 REPRODUCIBLE=no + reproducibility.txt 如实登记（**不假绿**），GA 前清偿 |
| **D5 sidecar 持 tantivy bm25 写锁** | M10-WP03-T01 巡检新债（WP03 §6-D5） | ⏳ 登记不在本 WP 处置（铁律 9）：sidecar 拉起后桌面自身 IndexEngine LockBusy、检索面 error 常驻；另立任务候选：sidecar 懒开索引或改读句柄/共享 writer |
| **rc/0.2.0 发布执行** | M10-WP06 §3 | ⏳ 待用户确认三轴口径（T02 推荐案，确认前不 tag 不 publish 红线沿 M9-WP06 §2.1）；确认后另立执行任务（bump 49 处清单已备） |
| **外部审计双义务（M2-D1/OSCP + Mimosa 完整结论）** | M7/M8 延续 | ⏳ 资金回笼触发（条件行维持）；M10 期重扫登记见 §4 |
| M11 提案拍板 | M10-WP00-T03（PR #202） | ⏳ 五线候选推荐排序 v0.2 落档，待用户拍板后立项 M11-WP00 |
| O_DIRECT/direct_io / SSO-OIDC F4 / 移动端 / reranker-SMB / WASI 0.3 | 既有登记 | 维持（不承诺 / 条件触发 / M10+ / 永久否决 / toolchain 触发） |

## 6. AI 使用披露（自动统计）

- 挂接 M10-\* 任务的提交数：**70**（`xtask report` 自动统计，
  `git log --all` 含 squash 前分支提交的重复计数）；范围
  `67c339f..805a800` 实测 **45 commit = 42 个 M10 任务提交（44 PR，
  三堆叠 #188/#189/#191 聚合 1 commit）+ 3 个 M9 尾款**（#153/#154/
  #157，M9 发布执行落地，不入 M10 统计）
- 任务数：**31**（xtask 按 Task-ID trailer 去重；WP00-T02 与 T01
  同卡合入 PR #155，trailer 仅挂 T01——SPEC §3 两行均已执行）；工作
  包分布：M10-WP00–WP06 全线（WP00×2、WP01×5、WP02×3、WP03×4、
  WP04×6、WP05×6、WP06×5）
- AI 辅助提交（AI-Assist）：main 范围 45/45 commit 逐个校验零缺失
  （xtask 口径 70/70 含分支重复），全部挂 GLM-5.3-Flash (ZCode) 披
  露；单会话单任务 + 极简指令自主推进模式
- AI-Review trailer：**7 个 commit 共 10 处**（探针自查/对抗审查签字）
- 人工终审提交（Reviewed-By）：**0/45**——与运行授权一致：用户已授
  权本运行内 PR 自动批准（CI 绿 + 对抗评审通过即合入），人工终审以
  PR 批准 + 关键拍板批示形式执行（#152 蓝图提案、#155 主题方向、
  #198 rc 口径等拍板位均为用户显式指令）；无 AI 审查报告缺失的
  merged PR
- 本报告本体由 GLM 起草（M10-WP99-T01），G3 三签不在其代签范围

## 7. 抽查审计记录（xtask trace ×5 抽样任务）

| 任务 | trace 结果 | 判定 |
|---|---|---|
| M10-WP01-T02 | 2 commits（#159 引擎面 + #160 前端面堆叠）· spec M10-WP01 §3 ✓ · AI-Assist ✓ · 双语截图 4 帧在库 | PASS |
| M10-WP02-T02 | 1 commit（bfded23，PR #166；分支侧合并 c67f9b0 解冲突后 squash 产出，主题无 (#N) 后缀，合入路径 AI trailer 自证 + gh merged 记录核验）· spec ✓ · 四探针 + GUI 五帧 | PASS |
| M10-WP04-T04 | 1 commit（#179）· spec ✓ · P22 三臂探针 + 对抗评审 3 findings 修复留痕 | PASS |
| M10-WP05-T04 | 2 commits（#192/#193 两堆叠）· spec ✓ · ADR-0031 决策 3 查证回填 + P23 登记 | PASS |
| M10-WP06-T03 | 1 commit（#199）· spec ✓ · 双窗口登记四文件落点全对账 | PASS |

5/5 追溯链完整（Task-ID → SPEC → commit → PR）；无越卡文件清单、
无 Task-ID 缺失；堆叠判例（两堆叠/三堆叠/聚合合入）经 trace 与 gh
双通道核验无漏。

### §7.1 审计清单（必要项）

- [x] fmt/clippy 本地对 main HEAD 绿（2026-10-08 实跑）；CI main
      HEAD（805a800）success（run 37790713351）
- [x] `cargo test --workspace --no-fail-fast` 本机全量：111 二进制
      683 passed / 0 failed / 13 ignored（ignored=8 基准/显式跑 +
      5 doctest，理由在案 §3；M9 期环境耦合失败样例本期 ok）
- [x] cargo deny 绿（4 项 ok；M10 期间 deny.toml 零改动，末次变更
      c93f2f3 = M8-WP07-T02 #102）
- [x] cargo audit 10 项 vulnerabilities 全部 ADR 登记（0020/0021/
      0025r6/0028），无静默新增；8 allowed warnings 沿既有名单
- [x] bench：M10 无新基准任务（KPI 表均为工件在案记值）；无 SPEC
      性能红线回退
- [x] 抽查 5 任务追溯链完整
- [x] AI 披露 100%（45/45 任务提交 + 本报告本体披露）

## 8. 下一阶段建议（M10 尾款 + M11 方向）

1. **WP02 收官**：#165/#167 GUI 实操验证（桌面环境可得时）→ 合入 →
   SPEC §3 四行回填 + WP00 §1-WP02 收官行 + WP01 截图行/WP04 批准
   日期两处小尾巴一并回勾；
2. **rc/0.2.0 发布执行**：用户确认 T02 三轴口径 → bump 49 处 → tag
   `v0.2.0-rc.1` + draft Release → 用户终审 publish（红线沿
   M9-WP06 §2.1）；REPRODUCIBLE 评估实验随发布报告落档（不假绿）；
3. **D5 sidecar 索引锁任务候选**：sidecar 懒开索引或读句柄/共享
   writer（涉 gateway/index，需新任务卡）；
4. **M11 提案拍板**（PR #202 v0.2 五线候选）→ 立项 M11-WP00；
5. **条件触发行维持**：外部审计双义务（资金回笼）/ NB-WP05-1 真实
   IdP 端到端（可得 IdP/部署环境）/ drop-caches 复测（Linux 台架）/
   示例扩展生产钥签名（密钥持有人）/ 语义向量检索（WP04-T05 触发
   条件）/ 记忆编辑删除 GUI（WP00 §3-T03 承接位）。

## 放行签字（G3）

<!-- 三签由关账任务 M10-WP99-T02 落档（判例 M6-WP99-T01 PR #24 /
     M8-WP99-T01 PR #113 / M9-WP99-T02 PR #153——用户指令后由关账任务
     回填，报告不代签）。 -->

- [x] 架构负责人：@lead（2026-10-08 运行指令「M10 G3 关账放行
      （M10-WP99-T02）：三签落档（@lead，沿判例的补认机制）」，沿
      M6-WP99-T01 / M8-WP99-T01 / M9-WP99-T02 判例落档）
- [x] 评审人：@lead（同上）
- [x] 安全负责人（M2/M4 必需）：@lead（同上；延续义务如实登记：
      ① 外部审计双义务（M2-D1/OSCP + Mimosa 完整扫描结论）随资金
      回笼窗口，非放行条件——本期起草期 deep 重扫已登记
      docs/reviews/M10-WP06-mimosa-rescan.md，登记不构成安全放行
      结论；② scanner_enobufs 覆盖边界披露性挂账维持（本期重扫
      227 文件全程无 enobufs，工具侧能力边界非项目可修）；③ WP02
      T01/T03 两 PR（#165/#167）OPEN 挂「待 GUI 验证」——M10 唯一
      实施面遗留，桌面环境可得后验收合入；④ drop-caches 冷缓存复测
      挂 Linux 台架可得（RELEASE-CHECKLIST-beta B1 注记，非阻塞）；
      ⑤ 示例扩展生产钥签名挂密钥持有人（EXT-SIGNING「示例扩展签名
      窗口」节，key-custody）；⑥ REPRODUCIBLE=no 维持如实登记
      （rc 期评估实验建议已落档 M10-WP06-T02，不假绿，GA 前清偿）；
      ⑦ D5 sidecar 索引锁（另立任务候选）+ NB-WP05-1 真实 IdP
      端到端（条件触发）随 M11 承接；⑧ rc/0.2.0 发布执行待用户
      确认三轴口径，确认前不 tag 不 publish（M10-WP06 §3 红线））
- Reviewed-By 补认：M10 里程碑 46 提交（`67c339f..2f39b78`，含 3 笔
  M9 发布执行尾款 #154/#157 一并补认（#153 为 M9 三签本体）+ 报告
  本体 #203）随本三签一并补认生效，报告签字表为凭、不改历史（先例
  M6-WP99-T01 / M7 / M8-WP99-T01 / M9-WP99-T02；报告 §6 协作模式
  披露不变）
- 同窗口放行：无——M10 无发布执行动作；rc/0.2.0 发布执行待用户确认
  T02 三轴口径后另立任务（M10-WP06 §3，确认前不 tag 不 publish）
