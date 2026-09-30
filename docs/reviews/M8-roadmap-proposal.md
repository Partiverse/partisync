# M8+ 开发前瞻计划提案（M8-Roadmap Proposal）

> 版本: 0.1 · 状态: **提案（待用户拍板）** · 日期: 2026-09-30 ·
> 性质: 前瞻调研 + 立项建议（非 SPEC——各 WP 正式化须走
> partisync-spec-draft 批准流程，铁律 1）·
> 关联: M7-WP00 §1（WP 图滚动判例）、M7-WP03 §3-T03（企业特性议题
> 登记 → M8+ 立项建议表——本文档为其超集，T03 产出时引用本文档）、
> M6-report §5.3（债务总表）、RFC M3-WP02 §8（最老移交债）、
> ADR-0023/0024/0025（重新评估条件）·
> 输入源: ①全仓债务/顺延项扫描（docs/reports + docs/specs + docs/adr +
> docs/tests/properties.md，2026-09-30）；②赛道趋势 web 实测调研
> （Spacedrive V2 / MCP 生态 / WASI 0.3 / E2EE 企业化 / FUSE 网关，
> 2026-09 视角，来源见附录）；③调研方案与执行方案未兑现项核对。
> 负责人: @lead · 批准人: （拍板时回填）

## 1. 动机

M6 G3 关账（`f4f58e0`）后产品化基线交付完整，M7 四主题已闭合三个
（WP00 总纲 / WP01 WASM 扩展实施 / WP02 AI 安全复核），WP03（FUSE/SMB
桥接评估）T01 spike 已实测收官（PR #49，fuser 0.18 线位成立 + 容器真
挂载 2/2 绿 + 随机读 p95 74.6µs），剩 T02/T03/T04 文档收官。此时制定
M8+ 前瞻计划的必要性来自三个信号：

1. **M7-WP03 评估结论即将产生两个「是否实施」拍板点**（FUSE 实施 WP、
   企业特性立项），需要一张已经盘过全局的路线图承接，避免逐点临时
   决策；
2. **债务侧出现结构性信号**：RFC M3-WP02 §8 的 Hub-raft 业务接线移交
   自 M3 起无任何后续 WP 显式承接（最老未结移交项）；发布工程
   （执行方案 §5.4 G4）至今零实跑——全仓 **0 个 git tag、无 release
   workflow**，「基建底座」定位下没有一个签名可分发的对外版本；
3. **赛道窗口信号**（实测调研）：Spacedrive V2 至 2026-09 仍是
   alpha；MCP 官方 registry 明确不验代码签名（信任验证留白）；
   WASI 0.3 已定稿（2026-06-11）——三者分别构成竞争窗口、生态空位、
   升级窗口。

## 2. 现状盘点：能力面地图（实测，以 HEAD `940fb5c` 为准）

### 2.1 已交付（八层能力全通，为前瞻的地基事实）

| 能力层 | 交付物 | 实测锚点 |
|---|---|---|
| 本地引擎 | 百万文件索引 / CAS / pack v2 / EC / 分层 / GC | M0-M3 关账；随机读 p95 74.6µs（FUSE spike 实测） |
| 同步 | 三端 E2EE / bisync / Merkle 对账 / iroh / UploadAck | M2 关账 + M5-WP01（P99 8.35ms） |
| Hub | raft 分片元数据 / 联邦路由 / 云事件流 / 扫描调度器 | M3 + M5-WP02/03/04（RouteQuery P99 298µs；apply 508k evt/s） |
| 检索 | BM25+向量 RRF 混合 / LCSTS 真档评估 | Recall@10=0.95（M6-D67-T02） |
| AI 面 | sidecar 管线（OCR/Whisper/嵌入）/ MCP 五工具 / C2PA | M4 关账 + 真机点验 |
| 桌面 | Tauri 2 壳 / 窗口状态 / 扩展面板 | M6-WP03 八任务 + M7-WP01-T04 |
| 扩展 | wasmtime 宿主 + 注权 manifest（P13/P14） | 47/47 探针绿（M7-WP01） |
| 规模与质量 | 10⁶ 实测 + 10¹² 仿真外推 / 夜间混沌套件 / 18+6 探针安全面 | M5 关账 + M7-WP02 双复核 |

### 2.2 薄面与空白（前瞻计划的靶面，全部实测核对）

| # | 空白 | 证据 | 影响 |
|---|---|---|---|
| G1 | **对外发布零实跑** | `git tag` 为空；`.github/workflows/` 仅 ci/nightly/auto-merge；G4（签名+SBOM）从未执行 | 一切企业/生态/分发叙事的前提缺失 |
| G2 | **UI 单页薄面** | `crates/partisync-desktop/ui/` 仅 index.html + app-core.js + styles.css | 引擎能力对终端用户不可见；前端提速指令（2026-09-29）的持续靶面 |
| G3 | **FUSE 仅只读 spike** | fuser 在独立 spike workspace（产品依赖图零改动，M7-WP03 契约） | 写回日志 + overlay（调研方案 §5.13 差异化项）未实施 |
| G4 | **Hub 治理平面缺位** | 无审计日志/配额/多租户/SSO——企业特性仅议题登记（M7-WP03-T03 未产出） | E2EE 企业化的真正战场在治理平面（趋势 §3.4） |
| G5 | **Hub-raft 业务接线未兑现** | RFC M3-WP02 §8：`Hub::open` 无节点身份/组拓扑参数、线性一致语义未在 API 兑现、请求幂等去重未实现 | 最老未结移交债；线性一致承诺与实现脱节 |
| G6 | **扩展供应链缺位** | 无签名分发/registry；M7-WP01 §4 显式推迟至企业特性议题 | 「野蛮生长」风险已被 M7-WP03 §1 点名 |
| G7 | **扩展 guest 无终止保障** | M7-WP01 §6-R8：epoch/fuel 正解留 T05 后立项；现靠 spawn_blocking + 10s timeout 兜底 | 恶意/劣质扩展可挂死单个工具槽 |
| G8 | **MCP 仅本地 stdio 侧车** | McpSidecar over stdio（M6-WP03-T05）；Hub 未暴露远程 MCP server | 与 MCP 2026-07-28 spec（stateless + OAuth 2.1 resource server）演进脱节 |
| G9 | **移动端零交付** | ADR-0024 非目标显式留 M7+，需独立 ADR | QUIC 连接迁移（WiFi↔蜂窝）是同步产品移动端的天选特性（调研方案 §3.5），无人承接 |
| G10 | **AI 面纯后台化** | sidecar 管线静默索引；无用户可编排 workflow；reranker 挂起（ADR-0023 P2） | AI 价值兑现依赖可编排化（趋势 §3.5） |

## 3. 赛道趋势输入（2026-09 web 实测调研；判断部分为纸面推断，已标注）

1. **Spacedrive V2 仍 alpha、转 COSS**（实测）：V1 死于依赖腐化
   （prisma fork + libp2p），V2 2025-12 起 alpha 至今未 stable；商业
   模式 = 免费核心 + 付费垂直扩展（Photos/Chronicle/Ledger）+ 企业
   层。**判断（纸面）**：「本地优先文件引擎」窗口期仍在，且
   PartiSync 已交付的 iroh/CAS/混合检索/扩展运行时组合恰是 V2 才
   刚补齐的面——竞争变量是交付与分发速度，不是技术路线。
2. **MCP 生态信任层留白**（实测）：官方 registry（2025-09 preview）
   只做 DNS 域名验证，不验代码、不签名；typosquatting / tool
   description 投毒 / postmark-mcp 后门已实证；2026-07-28 spec 走
   stateless-first + OAuth 2.1 resource server。**判断（纸面）**：
   PartiSync 的 CAS digest 锚定 + 注权 manifest（P14）天然是
   registry 信任模型的密码学强化版，「签名 MCP 组件分发」是现有
   生态没人占的空位，且与 G6 直接互补。
3. **WASI 0.3 定稿**（实测：2026-06-11 批准，原生 async + 线程；
   wasmtime 为参考实现）。**判断（纸面）**：扩展运行时升级到
   WASI 0.3 可解 G7 的异步面（插件异步 I/O 不阻塞宿主）并解锁与
   Spin/wasmCloud 组件互通；须沿 ADR-0025 重新评估条件走（toolchain
   1.95+ 与六项基准重测联动）。
4. **E2EE 企业化时序判例**（实测：Proton Drive 2025-07 SOC 2 →
   2026-01 审计日志+audit-ready 导出 → SSO/HIPAA/BAA；Tresorit 同构）。
   **判断（纸面）**：企业化不动加密架构，竞争全在 Hub 侧治理平面
   （G4）——PartiSync 的 raft 元数据 + 云事件流（SQS/HMAC）恰是
   audit trail 的天然底座。
5. **AI 价值靠可编排兑现**（实测：Immich v3.0 Workflows、Raycast
   Automations；反向判例 Rewind/Limitless 2025-12 关停——云依赖
   时间线记忆产品被证明脆弱）。**判断（纸面）**：本地优先的时间线
   + C2PA 溯源叙事反而因 Rewind 之死获得安全叙事溢价；G10 的解法
   是「新文件入库 → OCR → 嵌入 → 规则命中 → 动作」的用户可编排化。
6. **FUSE 挂载面企业语义收敛**（实测：mountpoint-s3 GA 后持续
   加读缓存/目录支持；rclone mount 企业痛点 = 元数据重操作产生
   S3 API 账单）。**判断（纸面）**：mountpoint-s3 式窄契约已被
   行业收敛为正解（M7-WP03 路线验证正确）；PartiSync 本地 Merkle
   索引天然免 LIST 风暴，「元数据成本可见性/配额护栏」可做成
   挂载面差异化。

## 4. 战略方向评估（四条路线；推荐为纸面推断，待拍板）

| 方向 | 内容 | 支撑 | 风险/代价 | 排序建议 |
|---|---|---|---|---|
| **α 企业就绪**（Enterprise-Ready） | FUSE/SMB 落地（G3）+ 发布工程（G1）+ Hub 治理平面（G4）+ Hub-raft 接线债（G5） | M7-WP03 评估结论直接承接；债务最密集区；企业市场 19.2% CAGR（实测调研） | 治理平面面广；发布工程一次性基建投入 | **主线推荐** |
| **β Agent 原生**（AI 记忆层） | MCP 远程化（G8）+ workflow 编排（G10）+ 本地时间线 | MCP spec 演进顺风；Rewind 之死的叙事溢价 | 编排引擎是新子系统，规模不小 | 交错插入（M9 主力候选） |
| **γ 扩展生态**（Ecosystem） | 签名 registry/分发（G6）+ WASI 0.3（G7）+ 垂直扩展 COSS | MCP 信任留白空位；WASI 0.3 窗口 | 生态需要第三方开发者，冷启动难 | 交错插入（地基项先行，生态爆发后置） |
| **δ 大众分发**（Consumer） | 移动端（G9）+ 安装器 + UI 深耕（G2） | QUIC 连接迁移红利；前端提速指令 | 移动端需独立 ADR + 双平台基建；单人模式带宽硬约束 | 后置（M10+），UI 增量持续穿插 |

**推荐组合逻辑**（待拍板）：M8 走 α 主线——① M7-WP03 评估收官后
FUSE/企业特性两拍板点零上下文损耗顺势落地；② G1 发布空白是定位级
硬伤，先补齐才能让 γ/δ 的一切叙事成立；③ G5 最老地基债越晚越贵；
④ 沿前端提速指令，α 各 WP 均定义 UI 可演示增量。β 的 MCP 远程化与
γ 的签名 registry 在 M9 汇合为「对外服务面 + 信任面」主题；δ 移动
端后置到 UI 基础与发布管线成熟之后。

## 5. M8+ 工作包前瞻提案（滚动；每 WP 正式化须走 SPEC 批准）

### M8（主题：企业就绪一期——挂载面落地 + 发布工程 + 治理地基）

| WP | 主题 | 输入 → 交付方向 | 依赖/触发 | 前置拍板 |
|---|---|---|---|---|
| M8-WP00 | 总纲 + 本提案正式化 | 本文档拍板意见 → M8 WP 图 + M7-WP03 遗留承接登记 | M7-WP03 收官（T02/T03/T04） | M8 主题方向 |
| M8-WP01 | **FUSE 实施**（fuser 入产品依赖图） | ADR-0026 批准 + spike 实测 → P15 转正；SEMANTICS.md 语义面；写回日志 + overlay 一期（覆盖/改名/删除 → 本地写回日志，调研方案 §5.13 差异化）；Samba 桥接操作手册；macOS FUSE-T 打包分发评估 | **条件 WP：ADR-0026 拍板 + 用户立项**（M7-WP03-T04 产出后）；冷缓存读基准/macOS FUSE-T 复测前置项随 WP 清偿 | FUSE 立项 |
| M8-WP02 | **发布工程 G4 实跑** | 执行方案 §5.4 → 首个签名 tag（建议 `v0.1.0-alpha`，口径：CLI + 桌面壳 + hub 三产物）；工程签名密钥双人保管；SBOM（cargo auditable）；可复现构建验证（`--locked` + toolchain 钉版）；release workflow + changelog 自动化；桌面壳 dmg 打包评估 | 无硬依赖（可与 WP01 并行）；是新依赖引入时 ADR + deny 门禁的常设面 | 版本口径与密钥保管方式 |
| M8-WP03 | **Hub 治理平面一期** | M7-WP03-T03 议题登记 → 审计日志（整理/管理操作 audit trail，复用 raft 元数据 + 云事件流底座，JSONL audit-ready 导出）；空间配额（硬限 + 软告警）；多租户最小面（空间↔租户映射） | M7-WP03-T03 产出威胁模型与候选清单；配额挂载面护栏（趋势 §3.6）随 WP01 联动 | 治理平面范围（建议只做审计+配额，SSO 留 M9） |
| M8-WP04 | **Hub-raft 接线债清偿** | RFC M3-WP02 §8 全部移交项 → `Hub::open` 节点身份/组拓扑；线性一致语义在 Hub API 兑现；请求 id 幂等去重；follower ReadIndex；并发写单调 proptest | 独立可并行；**M8 内优先级最高**（最老地基债） | 无 |
| M8-WP05 | **桌面壳功能面一期**（前端提速承接） | G2 薄面 → 检索/浏览/同步状态三大主界面实装（IPC commands 已就绪：stats/mcp_call/扩展面板判例）；扩展面板增强；demo 剧本升级 | 无硬依赖；沿 M7-WP01-T04 UI 接线判例（MockRuntime 单测 + 截图验收） | 界面信息架构 |
| M8-WP06 | **扩展终止保障**（小 WP） | M7-WP01 §6-R8 → epoch/fuel 实装（wasmtime 原生机制），替换 timeout 兜底 | 可穿插任意时点；wasmtime 47 线内 API 需查证 | 无 |

### M9（方向锚点：对外服务面 + 信任面 + Agent 化；正式化留 M8 关账后）

- **MCP 远程 server**（G8）：Hub 暴露远程 MCP 端点，按 2026-07-28
  spec（stateless-first + OAuth 2.1 resource server / PRM）；
  wasm 工具经 Hub 对外即「远程 MCP server」——与签名分发天然衔接。
- **扩展签名分发 + registry**（G6）：CAS digest 锚定 component 版本
  + 注权 manifest 签名 + 对接/自建 registry（威胁模型以
  M7-WP03-T03 产出为基）；目标「签名 MCP 组件分发」空位占位。
- **AI workflow 编排一期**（G10）：声明式规则管线（事件 →
  OCR/嵌入/转写 → 条件 → 动作），UI 可视化编排（前端提速承接）；
  本地时间线视图（C2PA 溯源叙事）。
- **SSO/OIDC + 外部审计窗口**：治理平面二期（若 M8-WP03 验证企业
  需求真实）；外部密码学审计 + OSCP 复核随资金回笼触发执行
  （M5-WP00 §4 提案保留待用，条件性、不占 WP 编号）。
- **WASI 0.3 迁移评估**：沿 ADR-0025 重新评估条件（toolchain 1.95+
  + 六项基准重测）做升级 WP 或维持 47 线登记。

### M10+（远期锚点，不承诺顺序）

- **移动端**（G9）：iOS/Android 壳（Tauri 2 mobile 或独立评估，
  需独立 ADR）+ iroh QUIC 连接迁移实测 + 移动同步策略（电量/网络
  感知）。前置：UI 基础成熟（M8-WP05/M9 编排 UI）+ 发布管线稳定。
- **联邦实战扩军**：多 hub 联邦从 loopback 测试床走向跨机实测；
  10⁹ 真档压测（M5-WP05 外推表的实测清偿）。
- **条件触发行**（不占编号）：reranker `hybrid_with_rerank`（
  ADR-0023 触发条件）；扩展自定义数据模型随空间同步（graph schema
  演进，独立研究 WP）；扩展热重载；SMB 原生线协议（已永久否决）。

## 6. 债务清偿映射总表（前瞻计划 × 全景扫描）

| 债/顺延项 | 登记处 | 本计划承接 |
|---|---|---|
| RFC M3-WP02 §8 Hub-raft 接线移交（最老） | `docs/rfcs/M3-WP02-*.md` §8 | **M8-WP04** |
| M7-WP01-R8 epoch/fuel 终止 | M7-WP01 §6-R8 | **M8-WP06** |
| 扩展分发/签名/registry 供应链 | M7-WP01 §4-1 → M7-WP03-T03 | T03 登记 → **M9** |
| FUSE 实施 + P15 转正 + 写回 overlay | M7-WP03 §4-2/4-3 + properties.md P15 | **M8-WP01**（条件） |
| 企业特性（配额/多租户/审计日志） | M7-WP03 §4-4 | **M8-WP03**（一期）+ **M9**（SSO） |
| G4 发布工程零实跑 | 执行方案 §5.4（本次盘点新增 G1） | **M8-WP02** |
| F-2 endpoint_secret 与 iroh 传输未合流 | SEC-AI-AUDIT-M7-WP02 §6 | **M8-WP04**（随设备通道认证握手消费） |
| ADR-0015 三条待确认（relay 定价/ALPN 互操作/上传窗口） | `docs/adr/0015` §待确认项 | **M8-WP04** 顺带回填 |
| 移动端 | ADR-0024 非目标 + M6-report §9.5 | **M10+** |
| reranker T04 | ADR-0023 | 条件触发，不占编号（维持） |
| 外部审计双义务（M2-D1/OSCP） | M7-WP00 §4 | 资金回笼触发（M9 窗口预期），每关账复核留痕 |
| 台账回填（ADR-0018/0019/0022/0023 状态头；M7-WP02 checkbox；M7-WP03 批准日期） | 本次盘点附录 | **M7-WP03 收官时顺带**（半天级，不单开 WP） |
| M3 无终版里程碑报告 | M4-report §5 | 挂「外部审计需要」条件，维持待定 |
| tauri-driver UI 自动化 / partisync-mcp 首启 println 怪癖 / wasmtime pooling 取舍 | M7-WP01-bench §5 遗留表 | 随 M8-WP05 / 下次 gateway 任务 / 内存压力触发分别顺带 |

## 7. 用户拍板事项清单（M8-WP00 前需逐项落锤）

1. **M8 主题方向**：推荐 α「企业就绪一期」（§4）；备选 β 先行
   （若优先 Agent 叙事）。
2. **FUSE 实施 WP 立项**：等 M7-WP03-T04 ADR-0026 草案产出后拍板
   （顺序约束：先评估结论后立项）。
3. **首个 release 口径**：版本号（建议 `v0.1.0-alpha`）、产物面
   （CLI + 桌面壳 + hub？）、签名密钥生成与双人保管安排。
4. **移动端启动时点**：M9 评估 ADR vs M10+（推荐后者）。
5. **资金回笼事件**：外部审计若在 M8 期间回笼，是否插队排期
   （默认：条件触发、不插队，按窗口容量顺延）。

## 8. 结论

引擎侧八层能力已全通且各有实测锚点，产品的下一个价值台阶不在
「更深的管线」，而在三个面的兑现：**可分发的版本**（G1 发布空白）、
**可治理的 Hub**（G4/G5 治理与一致性地基）、**可信任的扩展生态**
（G6/G7 供应链与终止保障）——分别对应 M8 主线与 M9 主题。竞争窗口
（Spacedrive V2 alpha 期）与生态空位（MCP 签名分发留白）都指向
同一结论：把已有能力面装进可发布、可治理、可信任的壳里，比继续
加深引擎更优先。本提案若获批，M8-WP00 沿 M7-WP00 判例正式化 WP 图
并逐 WP 走 SPEC 批准。

## 附录：趋势调研主要来源（2026-09 实测）

Spacedrive v2 文档站（overview/history、introduction）·
WASI Roadmap + Bytecode Alliance WASI 0.3 公告 ·
MCP Registry 公告（blog.modelcontextprotocol.io 2025-09-08）与
modelcontextprotocol/registry · MCP spec 2025-11-25 / 2026-07-28
changelog · Raycast AI（Custom Agents/Automations）· Immich roadmap
（v2.2 OCR / v3.0 Workflows）· Dropbox Dash · rewind.ai
（What happened to Rewind）· Proton Drive SOC 2 / 审计日志报道 ·
Ente 自托管文档 · mountpoint-for-s3（SEMANTICS.md）·
rclone forum（S3 元数据成本）。原始终端输出存会话记录，本文档只
保留结论与判断（判断处已标「纸面」）。
