# M11-WP05 评估:G10 AI workflow 编排——条件评估件(四问)

> 版本: 0.1 · 性质: **条件评估件(纸面,只评估不实施)**——零代码零依赖
> 变更;范围纪律沿 M9-WP05-mcp2-remote-eval「只评估不设计」判例与
> M10-WP04-T05 评估件判例 · 证据均为本仓 path:line 或本机命令实录 ·
> 上游: M11-roadmap-proposal §3-WP05/§4-4c(已拍板默认生效 2026-10-09)、
> M10-WP00 §2 拍板#4(未兑现欠账源头)、M8-roadmap §3-G10(锚点)、
> M10-WP04-memory-vector-eval(六问判例+T1–T4 触发表)、
> docs/reviews/M11-WP05-t2-query-evidence.txt(本评估产出的 T2 实证)
> 负责人: @partiverse · 批准人: @lead

## 0. 输入与基线

**G10 沿革**:M8-roadmap §3 将 G10「AI 面纯后台化」(sidecar 管线静默索
引,无用户可编排 workflow)列为空白,锚点解法 = 「新文件入库 → OCR →
嵌入 → 转写 → 规则命中 → 动作」的**用户可编排化**(M8-roadmap:95-99,
实测竞品 Immich v3.0 Workflows + Raycast Automations;反向判例
Rewind/Limitless 关停证明云依赖脆弱、本地优先获叙事溢价);M9 顺延
(「记忆层一期验证后评估」);M10 δ2 定形为「**声明式规则管线(事件 →
记忆读写 → 动作)评估件(范围 + 威胁面 + 依赖面),不直接实施**」
(M10-roadmap:97);M11 §4-4c 拍板补课(本件)。0.3 新输入:partiverse
V3 计划把聚合面也暴露为 MCP 工具(两仓 Agent 战略对齐)。

**能力现状基线**(盘点详见 §1):gateway MCP 工具面 11 个(含 memory_
write/search/verify/update/delete 全生命周期,mcp.rs:388-542);资产域
有事件机制(graph scan_journal + fuse FuseWriteEvent mpsc),**memory 域
无推送事件**(仅 oplog 拉式同步日志);partisync-ai = 纯推理管线
(embed/OCR/转写/缩略图/EXIF/C2PA),**无任何 LLM/agent 调用面**;扩展
宿主白名单仅 index.read + clock.read(无网络/MCP/记忆);partisd(M11-
WP02 新交付)= loopback MCP 常驻面。

**T2 联动实证(本评估产出,归档
docs/reviews/M11-WP05-t2-query-evidence.txt)**:BM25 词面检索下,
「语义近似但字面不同」查询确定性失败——语料含中文营收文档与英文例会
纪要,Q1 `revenue report` → **0 命中**(语义对应中文营收文档)、
Q2 `会议纪要` → **0 命中**(语义对应英文纪要文档),对照字面查询
`fermentation` 正常命中(索引健康)。即:**旗舰检索入口存在已归档的
语义缺口实证**,候选 4(语义向量检索)的 T2 触发条件满足(触发起码
= 启动立项评估;评估结论仍可为不实施)。

## 1. 问题一:范围——G10 编排面的切分

**在范围内(锚点管线的底座侧最小闭环)**:

1. **事件源**:复用既有两条——资产域 scan_journal(JournalEvent,
   journal.rs:1-24,P8 先落盘后应用)与 fuse FuseWriteEvent
   (events.rs:11-20,mpsc → wiring 落 oplog);memory 域以 oplog 拉式
   轮询为事件源(store.rs:856 pending_oplog)或新增推送(后者属新面)。
2. **规则形态**:声明式(declarative)规则 = `事件过滤 → 条件表达式 →
   动作序列`,规则本身为可验证数据(可入 CAS/记忆,沿内容寻址);**
   不做**过程式脚本、不做图编排 DSL。
3. **动作面(底座侧 MVP)**:memory_write(库 API store.rs:2111 已
   程序化可用)+ 打标/整理(asset_organize 已有 MCP 面)。通知类动作
   (外发)不在底座——Partiverse/聚合侧职责(D19 分工)。

**明确不在范围**:可视化编排 UI(D19 冻结令——UI 归 Partiverse;底座
只保证规则数据格式对前端友好);LLM/agent 循环(partisync-ai 无此面,
且属独立大件);外部系统通知(webhook 等 = 网络面新地);hub 侧编排。

**建议 MVP 切片**:规则数据模型 + 事件订阅(scan_journal 起步)+
条件求值 + memory_write 动作 + 规则的 CRUD(经 MCP 工具暴露,与
partiverse V3 MCP 化对齐)。

## 2. 问题二:威胁面

| # | 威胁 | 分析 | 处置建议 |
|---|---|---|---|
| 1 | **内容注入链**:被索引文件的内容(攻击者可控)经事件→条件→动作写入记忆层 | 记忆承诺树会把注入内容升格为「可验证记忆」并经 sync 放大到对端 | 条件表达式只允许结构化字段匹配(path/tag/size),**禁止对文件内容做自由文本条件**;动作产出的 memory 打 `origin=workflow` 标记(可审计可 GC) |
| 2 | **规则来源与篡改**:规则数据若可被静默改写 = 持久化后门 | 规则入 CAS/承诺集则天然防篡改(内容寻址);沿 P21 扩展验签判例可加签章 | 规则变更走与 memory 相同的承诺/审计路径;管理面操作经 MCP 鉴权(沿 M10-WP05) |
| 3 | **动作面权限升级**:动作若扩到外发/执行即成 RCE 面 | ext-host 白名单判例:FS/网络/环境「根本不存在注入点」(inject.rs:10-13)是最强设计 | MVP 动作白名单锁死 memory_write/organize;任何动作扩面 = R2 公共面变更 + ADR |
| 4 | **常驻面暴露**:规则引擎若驻 partisd,loopback 无鉴权(ADR-0032 决策 5) | 本机恶意进程可改规则 | 维持 loopback 信任域声明;规则变更审计行(oplog 已天然记录) |
| 5 | **资源耗尽**:规则风暴(事件洪峰×规则×动作) | fuse 事件已有去抖+先落盘判例(P8) | 规则执行配额/去抖沿 P8 语义;MVP 单线程顺序执行即可 |

## 3. 问题三:依赖面

- **零新依赖可走多远——足够支撑 MVP**:事件源(mpsc/journal)、动作
  (memory_write 库 API)、条件求值(自研极小表达式:字段比较 + 集合
  匹配,无正则引擎依赖亦可达 MVP)全部为仓内既有件。编排调度 = tokio
  既有。**结论:MVP 零新依赖成立**。
- **预估 ADR 触发点**:①若条件表达式引入规则引擎 crate(如
  rules-engine/CEL 类)= 新顶层依赖 → ADR(铁律 8);②若动作面扩张到
  通知/外发 = 网络面 → ADR + R2;③ext-host 白名单扩容(让扩展当规则
  动作)= 公共面变更 → R2 + pre-merge-safety。
- **partisync-ai 关系**:编排引擎不含推理(ai 侧管线已是独立 stage 体
  系);G10 锚点中「OCR/嵌入/转写」环节 = 既有 sidecar 管线的**产物**
  作为事件载荷,编排面不重复建设。

## 4. 问题四:立项建议

1. **建议立项,但排 M12 主线候选、M11 不实施**(沿「只评估不实施」
   纪律):理由——①输入输出面首次齐备(事件源×2 + 记忆全生命周期
   API + loopback 常驻面,WP02 刚交付);②拍板欠账已还(本件);
   ③M11 主线(WP02)刚收官,连续大件违背单人带宽纪律;④M8 原判
   「编排引擎是新子系统,规模不小」仍然成立。
2. **MVP 形态建议**(届时 SPEC 落锤):声明式规则数据模型(内容寻址
   可验证)+ scan_journal 事件源 + memory_write 动作 + MCP 规则管理
   工具;UI = Partiverse 域。
3. **前置拍板项**(立项时):威胁面表 #1/#3 的处置落锤;动作白名单
   边界;规则数据是否入承诺集。
4. **T2 联动(本件已触发)**:查询实证已归档(t2-query-evidence.txt),
   **候选 4(语义向量检索)立项评估的 T2 触发条件满足**——建议立项
   评估随 M11-WP05 收尾一并启动(评估 ≠ 实施;T1/T3/T4 仍按
   memory-vector-eval §6 纪律)。
5. **partiverse 对齐**:V3「聚合面 MCP 化」落地时,规则管理工具天然
   并入同一 MCP 面;S1(partisd 常驻)已由 M11-WP02 兑现,编排引擎
   的宿主位已就绪。

## 5. 诚实登记(未做/未验证)

- 未跑任何向量检索对照(BGE-M3 对上述 Q1/Q2 的命中表现为**预期**
  而非实证——维度契约欠账(models.rs:43-45)未清偿,清偿 = 候选 4
  T3,届时补对照实验);
- 未做规则 DSL 原型或性能预估(全部纸面);
- partiverse V3 MCP 化计划引用自其规划文档,无代码集成实证;
- 竞品格局引 M8-roadmap/演进调研在案记值(2026-10 时点),未复测。

## 6. 结论

G10 的底座侧条件已齐(事件源 + 记忆全生命周期 + 常驻面),威胁面可
控且 MVP 零新依赖可达;**建议 M12 立项**(声明式规则管线 MVP,动作
白名单最小化),M11 以本评估件还清拍板欠账并触发候选 4 立项评估。
