# M9 里程碑报告

生成：`cargo xtask report M9`（骨架）+ 人工回填 · 版本: 1.0 · 日期:
2026-10-05 · 范围: `2a3ebb5..4b0f790`（M8 关账后 → WP06-T01 SPEC
草稿，36 commit / 35 PR）· 执行: GLM-5.3-Flash (ZCode) ·
**历史版本**: 本文件由 M9-WP99-T01 骨架（`xtask report M9`）正式化
回填，骨架即占位约定（fill-in），非覆盖历史报告。

## 1. 范围与结果（对照 SPEC 汇总；范围变更记录）

| WP | 主题 | 结果 | 关键交付 |
|---|---|---|---|
| WP00 | 总纲 + 台账 | ✅ | M9 路线图提案（PR #114）+ 总纲 SPEC + WP01 SPEC 草稿（PR #115）；§2 拍板 7 项中 5 项落锤（β 三主线 / stdio 先行 / minisign 选型 / 移动端 M10+ / 资金窗口维持）；T03 滚动登记 4 处范围变更（见下） |
| WP01 | 引擎写路径接线（β） | ✅ | T01–T05（PR #116–#121）：fuse 写事件面 + gateway `partifuse --graph` 装配层（事件折叠→graph/oplog + bisync tick）+ P19 转正 + 容器真挂载 e2e PASS 0.20s + F1 watermark 清偿（GUI 实操截图）；探针抓出 create 绝对路径事件缺陷已修；SPEC §3 全勾 0.3 |
| WP02 | 可验证记忆层一期（α） | ✅ | T01–T05（PR #122–#133；#124/#125 CLOSED 被 #126/#127 取代重开）：memory schema v16 + RFC 6962 式独立证明树 + P20 转正（探针 10 例 + n=1..70 全叶扫描）+ MCP 三工具 memory_write/search/verify + 双端 bisync 收敛 + CAS exists() 错误上浮精化；SPEC §3 关账全勾；10⁴ 叶根重算 175 ms |
| WP03 | 桌面深耕一期（γ 前半） | ✅ 实施面（T04 GUI 待人工） | T01/T02/T03/T05/T06（PR #134–#141）：esc() 统一 + 「记忆」tab（mcp_call 透传三工具，桌面零新增 IPC）+ 旗舰检索记忆通道（分区不混排）+ 默认数据目录对齐 `.partisync`（GUI 实测 266 文件截图归档）+ `partisync reindex`（产品搜索首个索引写入方）；SPEC §3 九项八勾，T04 GUI 人工项待验收 |
| WP04 | 扩展签名分发小步（δ） | ✅ | T01–T03（PR #142–#146）：ADR-0030 minisign 线位落锤 + minisign-verify 0.3.0 入根（零传递依赖）+ 装载期强制验签（拒绝先于编译）+ P21 转正六探针 6/6 + EXT-SIGNING.md + RELEASE-PUB-KEY 扩展节；SPEC §4 全勾；G6 债清偿（registry/更新通道维持不自建） |
| WP05 | MCP 2.0 远程化评估 + 骨架（δ） | ◐ T01 交付，T02 收尾待做 | 评估文档（差距面 G-1..G-6 + 授权 A-1..A-5 + 威胁模型 T-R1..T-R7 + 不承诺生产结论）+ hub `mcp_remote` 骨架（PRM mock / 401 挑战 / initialize 握手 / 202 / 404+-32601 / mock Bearer 边界钉死）+ 探针十路（PR #147–#149 三堆叠）；SPEC §4 关账勾选与 WP00 状态回填（T02）未做 |
| WP06 | v0.1.0-beta 发布（γ 压轴） | ◐ T01 准备件就绪 | T01（PR #150）：SPEC 草稿（beta 口径三轴推荐落锤——tag 携 pre-release / 三产物含 partisync-mcp+partisync-fuse / 含 FUSE 写回）+ CHANGELOG beta 草稿 + RELEASE-CHECKLIST + 本地核验留痕（cargo metadata LOCK_OK / core release 5.45s / auditable 1.67s / cli release 6m00s 38M）；发布动作**待用户确认口径后执行**（T02 workflow 修订、T03 复测窗口+tag+publish 未开工） |

**范围变更**（WP00 §3 T03 滚动登记）：①WP03 v0.2 增补默认数据目录
对齐（T05；实施期发现：桌面独立空目录致用户实测「没有任何功能」）；
②WP03 v0.3 增补 reindex 命令（T06；实施期发现：`upsert_content`/
`rebuild_index` 零生产调用方——产品搜索从未工作过）；③WP05 0.2
实装/探针切分修订（铁律 3 ≤400 行触发，契约面零变更）；④WP06 发布
节奏后置——SPEC 与准备件先行，发布动作待用户确认（沿 M8-WP02
draft→终审→publish 判例）。无降级项；各 SPEC §4 非目标全部维持。

**M9 新增 PR 数**：#114–#123、#126–#150 共 **35 个**（全部
squash-merged；35/35 head commit 的 CI run conclusion=success 现场
核验，`gh run list --commit <sha>` 逐个过一遍，无红灯合入）；
#124/#125 为 CLOSED（堆叠 PR 被 #126/#127 取代重开，工作未废弃）。
另有一笔用户直推提交 445c05c（README 清理，无 Task-ID、不经 PR），
不在 35 个 PR 内。

## 2. KPI 达标表（基准报告链接）

| 指标 | 记值 | 证据 |
|---|---|---|
| 挂载写→同步端到端（容器真挂载 e2e，rust:1.94-alpine） | PASS 0.20s 确定性 | docs/specs/M9-WP01.md §3（T03 PR #120） |
| 记忆证明树根重算（10⁴ 叶，全量） | 175 ms | docs/specs/M9-WP02.md §3（T03 任务卡记值；增量树挂 ADR-0029 重估触发器 10⁶ / P95>100ms） |
| reindex 真实数据集 | indexed=50 / skip_binary=186；`partisync search "argon2 migration"` 命中 2 条 | docs/specs/M9-WP03.md §3（T06 卡实测记录） |
| 桌面冷启动（M6 延续） | P95 < 1500ms 维持 | M6-WP03-T04 基线；WP03 无启动路径回归 |
| WP06 发布准备件本地核验 | core release 5.45s / auditable 干跑 1.67s / cli release 6m00s（产物 38M，<10 分钟线） | docs/specs/M9-WP06.md §3 T01 留痕 |

注：M9 无新基准任务（bench 归 WP06 复测窗口）；上表全部为 SPEC/任务
卡工件在案记值，本报告未重跑基准。

## 3. 测试证据（覆盖率/属性测试/变异分数/模糊时长/混沌/互操作）

- **CI**：全程 8 jobs（fmt/clippy/deny/interop/task-ids/test×2/gate）；
  M9 全部 35 个 merged PR head commit run conclusion=success（现场
  逐个核验），无红灯合入；main HEAD（4b0f790）run 37219704110
  7 jobs success（task-ids 按 PR-only 设计 skipped）。
- **新增属性测试/不变量**（docs/tests/properties.md:26-28）：P19
  挂载写同步收敛（wp01_wiring 探针 + 容器 e2e；origin 剪枝/幂等/同根
  三支）+ P20 记忆层可验证承诺（wp02_memory 探针 10 例：乱序同根/
  全叶包含验证/三类单字节篡改必败/幂等写/canonical/防篡改检出 +
  n=1..70 全叶扫描；探针抓出 verify_inclusion 奇偶判位缺陷，修复为
  RFC 9162 双游标）+ P21 扩展签名装载（wp04_signature 六探针 6/6：
  五路 + scan 孤儿签名 + garbage 反证对照）。
- **探针面**：hub `wp05_mcp_remote` 十路（PRM 形状/401 挑战/握手/
  202/404+-32601/mock Bearer 边界/doc 静态断言）；desktop
  ui_hardening 三探针 + 记忆面板 e2e（stub 侧车 payload 逐键对账）+
  旗舰双通道分区不混排探针；gateway 工具面真实 stdio e2e（schema
  逐字段断言 + 限界拒绝）。
- **互操作/容器**：WP01 容器真挂载 e2e 实测 PASS（`--device` 门控
  SKIP 沿 M8 判例）；musl/双平台由 CI test(ubuntu+macos) 兜底。
- **本机全量复跑**（2026-10-05，`cargo test --workspace
  --no-fail-fast`）：108 个测试二进制，**620 passed / 1 failed /
  13 ignored**。唯一失败 =
  `mcp_call_real_sidecar_ext_list`（desktop tests/commands.rs:241），
  环境耦合 e2e：本机存量 `~/.partisync/extensions/` 示例扩展无
  `.minisig`（M7 期安装，早于 WP04 强制验签），sidecar `ext_list`
  按验签语义正确拒绝（count=0）→ 断言失败。该测试在无预置文件的
  CI 环境自跳过（commands.rs 注释明示 e2e 性质不阻塞离线单测），
  故 CI 不受影响；根因登记 §5 债表「示例扩展断签窗口」。

## 4. 安全（cargo audit / deny / unsafe 增量 / 外部审计）

- **cargo deny**：`cargo deny check` → advisories/bans/licenses/
  sources **4 项 ok**（2026-10-05 本机复核）。唯一 warning =
  RUSTSEC-2026-0253 `advisory-not-detected`（deny.toml:61 既有登记
  项未被当前依赖树命中，信息性，非失败）。
- **cargo audit**：`cargo audit --no-fetch` → 7 vulnerabilities +
  8 allowed warnings；7 项**全部有既有 ADR 登记、零静默新增**——
  rsa RUSTSEC-2023-0071→ADR-0020、h2 0258 + webpki 0098/0099/0104
  →ADR-0021（deny.toml:60-65）、wasmtime 0315/0316→ADR-0025 修订 6
  （deny.toml:74-75，fuel 未启用/无 resource·record 可达性登记）；
  unmaintained 警告（proc-macro-error/atomic-polyfill/instant/paste
  等）沿 allowed 名单。M9 期间 deny.toml ignore 零改动。
- **新依赖**：minisign-verify 0.3.0（MIT、零传递依赖，cargo tree
  单行）——M9 唯一新顶层依赖，铁律 8 由 ADR-0030 承载（PR #144
  deny 四项 ok 实证）；partisync-cli 增 sqlx 为 workspace 既有 pin
  复用（WP03-T06，任务卡留痕）。
- **unsafe**：workspace `forbid(unsafe)` 维持；M9 新增代码零 unsafe。
- **Mimosa**：M9 期间无新扫描（本机扫描史最新 =
  `scan-2026-10-02T15-25-41.420Z-0a39454d7cbc`，M8 期 deep 复扫，
  全程无 enobufs）；完整扫描结论随外部审计窗口（资金回笼触发）
  一并出具。**登记不构成安全放行结论**。

## 5. ADR 清单与债务登记

**M9 期间新增/修订 ADR**：

| ADR | 主题 | 状态 |
|---|---|---|
| ADR-0026（修订 0.5） | FUSE 网关语义——SEMANTICS 同步接线节（冲突折叠回灌/回环防护） | 接受（PR #118） |
| ADR-0029 | 记忆 schema v16 + 可验证记忆层（独立证明树、LWW 簿记、放弃项登记） | 接受（PR #122） |
| ADR-0030 | 扩展签名选型——复用 ADR-0027 minisign 线位 + minisign-verify 验签 | 接受（PR #142） |

**债务台账**（M9 清偿 5 笔 + 延续/新增 8 笔）：

| 债 | 来源 | 状态/去向 |
|---|---|---|
| 本地写不同步 N1（挂载写缺装配） | M8-report §8-2 | ✅ **已清偿**（WP01，PR #116–#121） |
| watermark 统计口径 F1 | M8-report §5 | ✅ **已清偿**（WP01-T04，PR #119：sync_stats 改 sync_watermark 派生） |
| escapeHtml F2/D2 + 详情面板空库 D3 + N4 转写开关 D4 | M8-report §5 | ✅ **已清偿**（WP03-T01，PR #135；D2/D3/D4 债表注记随 PR #141） |
| CAS 读 IO 错误伪装 ENOENT（微债） | M8-report §5 | ✅ **已清偿**（WP02-T05，PR #132：仅 NotFound 伪装，其余上浮 + unwrap_or(false) 移除） |
| 扩展供应链签名/registry G6 | M7-WP01 §4-1 | ✅ **最小闭环清偿**（WP04，PR #142–#146：强制验签 + 签名流程文档；registry/更新通道维持「不自建」硬约束） |
| MCP 仅本地 stdio G8 | M8 前瞻 | ✅ **清账**（M10-WP05-T06 注记，2026-10-08）：M9 半偿骨架（PR #147–#149）+ 正式远程化全交付（M10-WP05-T01–T05，PR #182–#185/#188–#195：真实 OAuth 2.1 RS + TLS + 11 工具透传 + mock AS 全链 e2e）；真实外接 AS 端到端 = NB-WP05-1 条件触发维持（M10-WP05 §4，诚实登记不伪造端到端） |
| **T04 GUI 三态截图 + 全 tab 巡检 + 篡改红徽章演示** | WP03 SPEC §3 | ⏳ **待人工**（AGENTS.md 桌面端硬性规则例外面；实施面已就绪；docs/screenshots/ 现存 T05 data-align 与 WP01-T04 f1-sync-tab 两张，T04 三态截图缺位） |
| **CAS 内容重组缺口** | WP03-T06 新债 | ⏳ 大文件 chunk hash 列表未持久化、CAS 不可重组——reindex CAS 回退仅覆盖单块；命中计 read_errors，挂后续任务 |
| **外部审计双义务（M2-D1/OSCP）+ Mimosa 完整扫描结论** | M7/M8 延续 | ⏳ 资金回笼触发（条件行，RFP-OSCP 沿用）；M9 期间无新扫描 |
| **scanner_enobufs 覆盖边界** | M7-report D-S2 | ⏳ 披露性挂账维持：hook advisory 失同步已 2026-10-02 deep 复扫消解（M8-WP00 台账行），工具侧动态派发不可达的能力边界非项目可修；本报告提交时（2026-10-05，M9-WP99-T01 commit）hook 再次提示 enobufs 无完整扫描结论（advisory，兼容策略放行）——边界复现，结论仍随外部审计窗口 |
| **示例扩展断签窗口** | WP04 SPEC §6-R5 | ⏳ 持有人生产钥签名待人工；本机实证影响：装有旧版未签名 demo 扩展的机器上 desktop e2e `mcp_call_real_sidecar_ext_list` 失败（ext_list 验签拒绝 count=0），CI 无预置文件自跳过不受影响（§3）——**窗口登记落档**（M10-WP06-T03，2026-10-08）：对象 `examples/extensions/demo_ext.*`（实测无 `.minisig`）、闭合动作 = 持有人生产钥签名（EXT-SIGNING.md「示例扩展签名窗口」节，key-custody 纪律 AI 不可代办）；未闭合行为维持 = 强制验签下装载拒绝（P21 不变，非缺陷回归） |
| drop-caches 冷缓存复测窗口 | M8-WP07-bench §2 | ⏳ 挂 M10-WP06 复测窗口执行（原「WP06-T03 发布窗口」改挂；release profile + 容器口径不变，触发 = Linux 台架可得；非阻塞——读路径自 M8 未改 + 冷基线 1567 MiB/s 维持；登记落点 RELEASE-CHECKLIST-beta.md B1 注记，M10-WP06-T03） |
| WP05-T02 收尾回填 / WP06-T02+T03 发布执行 | M9 未开工项 | ◐ WP05 状态行回填已做（M10-WP00-T02）+ G8 行回填已做（✅ M10-WP05-T06，2026-10-08）；WP06 发布待用户确认口径（tag/draft/publish 红线前置） |
| O_DIRECT/direct_io / SSO-OIDC F4 / 移动端 / reranker-SMB | 既有登记 | 维持（不承诺 / 条件触发 / M10+ / 永久否决） |

## 6. AI 使用披露（自动统计）

- 挂接 M9-\* 任务的提交数：**52**（`xtask report` 自动统计，含
  squash 前分支提交的重复计数）；范围 `2a3ebb5..4b0f790` 实测
  **36 commit = 35 个 M9 任务提交（全经 PR squash）+ 1 用户直推**
  （445c05c README 清理，无 Task-ID，人工直推不入 AI 统计）
- 任务数：**22**；工作包分布：M9-WP00–WP06 全线（WP00×2、WP01×5、
  WP02×5、WP03×5、WP04×3、WP05×1、WP06×1；未开工：WP03-T04、
  WP05-T02、WP06-T02/T03）
- AI 辅助提交（AI-Assist）：**35/35（100%）**——逐 commit 校验无缺
  失，全部挂 GLM-5.3-Flash (ZCode) 披露；单会话单任务 + 极简指令
  自主推进模式
- AI-Review trailer：**8** 处（探针自查/对抗审查签字）
- 人工终审提交（Reviewed-By）：**8/35**——低比例与协作模式一致：
  **人工终审以 PR 批准合入 + 关键拍板批示形式执行**（#114「同意
  M9 路线图提案」、#115「批准 pr115」两条显式批示 + 其余 PR 批准
  模式）；无 AI 审查报告缺失的 merged PR
- 本报告本体由 GLM 起草（M9-WP99-T01），G3 三签不在其代签范围

## 7. 抽查审计记录（xtask trace ×5 抽样任务）

| 任务 | trace 结果 | 判定 |
|---|---|---|
| M9-WP01-T03 | 2 commits（#120 链）· spec M9-WP01 §3 ✓ · AI-Assist ✓ · 容器 e2e 0.20s 留痕 | PASS |
| M9-WP02-T02 | 3 commits（#123/#126/#127 堆叠）· spec ✓ · P20 探针 + RFC 9162 修复 | PASS |
| M9-WP03-T06 | 3 commits（#137 实装 + #141 关账）· spec ✓ · T04 待人工如实标注 | PASS |
| M9-WP04-T02 | 7 commits（#143/#144/#145 三堆叠）· spec ✓ · P21 六探针 | PASS |
| M9-WP05-T01 | 6 commits（#147/#148/#149 三堆叠）· spec 0.2 修订 ✓ · 探针十路 | PASS |

5/5 追溯链完整（Task-ID → SPEC → commit → PR）；无越卡文件清单、
无 Task-ID 缺失；三堆叠判例（WP04-T02/WP05-T01）PR 正文互相引用
顺序，xtask trace 按 Task-ID 聚合无漏。

### §7.1 审计清单（必要项）

- [x] fmt/clippy 本地对 main HEAD 绿；CI main HEAD（4b0f790）7 jobs
      success（run 37219704110）
- [x] `cargo test --workspace --no-fail-fast` 本机全量：108 二进制
      620 passed / 1 failed / 13 ignored；唯一失败为环境耦合 e2e
      （示例扩展断签窗口，§5 债表；CI test ubuntu+macos 绿不受影响）
- [x] cargo deny 绿（4 项 ok；ignore 增量零——M9 期间 deny.toml 零改动）
- [x] cargo audit 7 项 vulnerabilities 全部既有 ADR 登记（0020/0021/
      0025r6），无静默新增
- [x] bench：M9 无新基准任务（WP01 e2e 0.20s / WP02 175ms / WP06
      准备件记值均为工件在案）；无 SPEC 性能红线回退
- [x] 抽查 5 任务追溯链完整
- [x] AI 披露 100%（35/35 任务提交 + 本报告本体披露）

## 8. 下一阶段建议（M9 尾款 + M10 方向）

1. **M9 尾款三件**：T04 GUI 人工验收（三态截图 + 全 tab 巡检 +
   篡改红徽章演示）→ WP05-T02 收尾回填 → WP06-T02 workflow 修订；
2. **WP06 发布执行**（T03）：用户确认 beta 口径三轴 → drop-caches
   复测窗口（容器）→ tag + draft Release → 用户终审 publish——
   沿「AI 起草人终审」判例，publish 前置用户确认（红线）；
3. **外部审计双义务**（M2-D1/OSCP + Mimosa 完整结论）：资金回笼
   触发，条件行；
4. **正式远程化立项评估**：WP05 骨架 → 真实 OAuth 2.1 / TLS / 工具
   透传（评估文档 §6-O2/O3 必答题已列，随真实需求验证后走新 SPEC）；
5. **记忆层二期与搜索补全**：memory update/delete/GC（tombstone）+
   语义向量检索（条件触发）；CAS 内容重组缺口随 reindex 二期清偿；
   示例扩展生产钥签名（持有人动作）随 WP06 发布窗口一并。

## 放行签字（G3）

<!-- 三签由关账任务 M9-WP99-T02 落档（判例 M6-WP99-T01 PR #24 / M8-WP99-T01
     PR #113——用户指令后由关账任务回填，报告不代签）。 -->

- [x] 架构负责人：@lead（2026-10-05 用户指令「M9 G3 三签确认」，沿 M6-WP99-T01 / M8-WP99-T01 判例落档）
- [x] 评审人：@lead（同上）
- [x] 安全负责人（M2/M4 必需）：@lead（同上；延续义务如实登记：① 外部审计双义务
      （M2-D1/OSCP + Mimosa 完整扫描结论）随资金回笼窗口，非放行条件；② scanner_
      enobufs 覆盖边界披露性挂账维持；③ T04 桌面 GUI 三态验收 + 全 tab 巡检为
      WP03 唯一未勾验收项，挂人工窗口（WP03 实施面已关）；④ CAS 内容重组缺口
      （chunk hash 未落表）挂 M10；⑤ M9-WP04 示例扩展生产钥签名挂 WP06 发布窗口）
- Reviewed-By 补认：M9 里程碑 38 提交（2a3ebb5..75ca002）随本三签一并补认生效，
  报告签字表为凭、不改历史（先例 M6-WP99-T01 / M7 / M8-WP99-T01；§6 协作模式
  披露不变）
- 同窗口放行：M9-WP06 v0.1.0-beta 发布执行经用户确认（2026-10-05），沿
  RELEASE-CHECKLIST-beta 执行，发布产物与留痕另档
