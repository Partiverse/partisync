# ADR-0035: 记忆域语义向量检索面——graph 独立通道与依赖图新边、降级语义 fail-open（BM25 通道回退，非向量暴力扫）、嵌入异步伴生与崩溃全量回填、RRF 本地实现接受重复、memory 专享 key 域独立实例

版本: 0.1 · 状态: **草稿**（随 SPEC M11-WP06 批准生效——合入 = 批准，
沿 ADR-0030/0032 判例；批准人 @lead）· 关联: SPEC docs/specs/M11-WP06.md
（本 ADR = 其 §6-1 实施立项前置件）、M11-WP05-t05-vector-reinitiation-eval
（重触发立项评估 §3 前置 2）、ADR-0018（影响范围只锚定 partisync-index，
本 ADR 扩展其射程至记忆域，M10-WP04-memory-vector-eval.md:259-262）、
ADR-0029（memory 可验证承诺层，P20 语义零变更边界）· 负责人:
@partiverse · 批准人: @lead · 起草日期: 2026-10-11

## 背景

记忆域检索现状 = FTS5 trigram + LIKE 兜底（store.rs:2533 起，memory_search
全路径见 :2530-2536 注释），非语义分。T1 触发条件已于 2026-10-10 清偿：
LCSTS 语料实插 **118076 叶**（≥10⁵，幂等重跑同根 verify ok=true，
M11-WP05-t1-corpus-evidence.txt:35-51），FTS 低召回实证在案（100 条语义
探针 98% 零命中 + 正控存活，:89/:228-229），用户裁决「T1 语料满足」已
登记（docs/tasks/M11-WP05-T04-t1-corpus.md:44）。重触发立项评估判定
**实施立项放行（有条件）**，实施窗口 = 本 WP06 SPEC 批准后（评估件
§1/§3）。组件盘点「组件级零缺口，接线级全缺口」——fastembed
（Cargo.toml:58 pin `=7.0.1`，ADR-0017/0018 锚定）+ usearch（Cargo.toml:53
pin `"2.26"`）+ RRF 融合判例（hybrid.rs:5-10/:25，RRF_K=60.0）全部在仓，
本 WP 做三条工程接线（写入/查询/生命周期，memory-vector-eval.md:71-76）。

硬约束：①crate 地图方向（AGENTS.md：领域层不得反向依赖能力层——
graph→index 属违禁方向）；②铁律 8（新顶层依赖须 ADR + cargo deny——
usearch/fastembed 均既有 workspace pin，`grep usearch crates/*/Cargo.toml`
实证仅 index/Cargo.toml:18 命中，本 ADR 只登记**依赖图新边**，deny.toml
与 workspace 顶层依赖区零变更，SPEC M11-WP06 §5 明文）；③P20 记忆承诺
语义零变更（向量不入承诺树不入 oplog，SPEC §2.0）；④与评估件存在一处
**已登记的机制偏离**（SPEC §6-Q2）：评估件 §3 前置 1-③ 原文 =「方案 C
暴力扫作冷启动/回退」，本 ADR 须对「BM25 通道回退 vs 向量暴力扫回退」
显式二选一（决策 2）。

## 决策

1. **落点 = partisync-graph 独立向量通道，不依赖 partisync-index；依赖图
   新边（graph → usearch/fastembed 既有 pin）随本 ADR 登记**。记忆域
   三腿（写入伴生/查询融合/生命周期同步）落 partisync-graph 新模块
   `memory_vector.rs` + 独立 `memory_vec.usearch` artifact + 轻量伴表
   （嵌入状态跟踪，schema v19 additive-only，沿 v16/v17/v18 判例
   schema.sql:1/:214/:238）；feature 门控 `graph-embed =
   ["dep:fastembed"]` 默认关（沿 ai-embed/index-embed 判例，
   partisync-index Cargo.toml:11-12）。**否决**「落点 partisync-index
   复用其向量栈」：graph→index = 领域层反向依赖能力层，crate 地图
   违禁方向，且把记忆域私有生命周期（tombstone/GC 腿）耦合进资产域
   引擎 crate；**否决**「方案 A 资产域混域复用」（维度契约 + key 域
   混域双硬伤，M10-WP04-memory-vector-eval.md:244 出局判例维持）。
   依赖图新边登记：`partisync-graph` 新增对 workspace 既有
   `usearch = "2.26"`、`fastembed = "=7.0.1"` 的依赖边（均非新增顶层
   依赖，不触铁律 8；deny.toml / workspace Cargo.toml 顶层依赖区
   **零变更**，SPEC §5 末行明文）。
2. **降级语义 = fail-open + 显式降级标记（`degraded:
   "vector_unavailable"`），回退机制 = 现有 BM25 通道（FTS5+LIKE），
   显式否决向量暴力扫回退**（SPEC §6-Q2 二选一就此落锤，评估件机制
   偏离处置完毕）。嵌入器缺失/初始化失败/向量索引缺失时 `memory_search`
   正常返回 BM25 结果并在响应载荷显式携带降级标记，不静默、不报错。
   **不选暴力扫回退的理由**（三）：(a) **覆盖不全**——降级三场景中
   「向量 artifact 缺失/损坏」下暴力扫同样无数据可扫（原始向量存于
   usearch artifact），暴力扫只覆盖「usearch 索引态异常但向量原始件
   在位」的子场景，若要全覆盖须独立于 HNSW artifact 另存一份原始向量
   存件伴表双写，存储代价 ×2（~230 MiB @118076 叶 ×2，SPEC §2.6
   口径）；(b) **无实测背书**——暴力扫延迟仅纸面 ~5–20ms/查询
   （M10-WP04-memory-vector-eval.md:214-215，无在仓实测），以未实测
   路径作回退违反「无在仓实测者一律标注纸面并登记 bench 前置」纪律
   （同件 :5-7），而 BM25 通道是现役有在仓实测的检索面（FTS5 trigram
   索引检索亚毫秒–毫秒级，同件 §5 参照系）；(c) **判例链方向一致**——
   既有降级判例（store.rs:2533 FTS 不可用降 LIKE；CLI hybrid 回退
   BM25 且 stderr 显式提示 main.rs:955-965）均为「回退到更简单/更老
   的现役通道」，暴力扫 = 在降级时刻新增一条全量计算路径
   （10⁵×512d ≈ 5.1×10⁷ MAC/查询），与降级语义「保可用、可预期」
   相悖。fail-closed 主张（检索结果必须可承诺通道来源）**否决**：
   检索为只读查询面，fail-closed 将模型故障放大为检索全不可用，无
   安全收益（SPEC §2.4 依据 ②）。**共同底线不变：不接受静默降级**。
   与 2.2 初始化 fail-fast 不矛盾：init 期模型维度异常（首嵌 ≠ 512）
   = 配置错误必须暴露；运行期模型/索引缺失 = 环境状态，降级保可用。
3. **嵌入异步伴生，不进 memory_write 写事务；崩溃后不做增量恢复，
   回填器全量重算补齐**。写入腿 = memory_write 事务外异步伴生任务
   （宿主 = partisd；M10-WP04-memory-vector-eval.md:246 判定 + T04
   判例 118076 叶 31s 批量直插，t1-corpus-evidence.txt:35——同步嵌入
   会量级化拖慢写入）；调度落锤 = **partisd 宿主内内存队列**，不建
   持久化队列表——崩溃丢队列不做精细 journal 恢复，重启后批量回填器
   （crates/partisync-graph/examples/ 存量件，dev-target，沿
   import_lcsts.rs 判例不入产品依赖图）按伴表嵌入状态全量扫描补齐，
   upsert 幂等。向量 = 本地派生缓存（SPEC §2.0），丢失可随时全量重建，
   **无一致性债**；伴表嵌入状态仅作可观测性与回填范围判定，不承载
   恢复承诺。**否决**「持久化任务队列表 + 断点续跑」：为派生数据引入
   新的一致性记账面（队列与 oplog 的双写时序），复杂度收益不成比例。
4. **RRF 融合式在 graph 本地实现，接受与
   crates/partisync-index/src/search/hybrid.rs:25 的 ~20 行重复并登记
   债务（SPEC §6-Q6 处置候选 (a)）**。k=60 对齐 hybrid.rs 判例。领域层
   不得反向依赖 index crate（决策 1 同源约束）；**否决**「RRF 下沉
   core」：基础类型 crate 承载检索融合语义归属存疑（core 被所有人
   依赖，检索融合非基础类型），且为消除 20 行重复引入跨 crate 公共
   API 面（R2 钉子清单级变更），不成比例。重复债务登记于本 ADR，若
   未来第三处 RRF 出现再评估下沉。
5. **key 域 = memory 专享：key = blake3(memory_id) 前 8 字节 u64，
   独立 usearch 实例，与资产域 content key 域物理隔离**。派生函数沿
   content_key 判例（vector.rs:22-28，大端 u64）；碰撞口径 2⁶⁴ 生日界
   （vector.rs:9-14）原样沿用，upsert 路径以「remove + add」保证幂等
   覆盖；同 memory_id 重复 upsert 不增条目（P25 (b)）。**否决**「与
   资产域共用单一 usearch 实例 + key 空间分区」：混域出局判例
   （M10-WP04-memory-vector-eval.md:244）——两类业务在同一索引内的
   生命周期互扰（GC/tombstone 物理删腿）与 key 语义混淆不可控。

**公共面登记**（SPEC §3 末项义务）：`memory_search` MCP 工具新增可选
参数（`mode: "hybrid"`，mcp.rs:480 schema / :615 分派 / :748-756 面），
开启时 = BM25 Top-N + 向量近邻 Top-N → RRF 融合（k=60）；无该参数的
请求工具 schema 与响应载荷形状与现状一致；降级标记经响应载荷透传；
instructions 文本同步（mcp.rs:550-553）。向量通道不入承诺面与同步面，
P20 语义零变更（verify_memory 口径不含向量，schema.sql:218-228 memory
表结构不动）；对端副本各自重算嵌入。

## 后果

**正面**：11.8 万叶真实记忆库补上语义检索腿（T1 触发的用户裁决兑现）；
落点与依赖方向合法（零反向边、零新顶层依赖、deny.toml 零变更）；
降级语义显式化终结评估件机制偏离悬案（§6-Q2 落锤）；嵌入不拖慢写入
（T4 判例量级证据）；派生缓存定位使崩溃语义退化为「重启后全量重算」，
零一致性债。

**负面-风险**：graph 依赖树增重（usearch/fastembed 编译依赖入 graph，
feature 默认关缓解）；RRF ~20 行重复成为已知债务（决策 4 登记，第三处
出现须重评）；fail-open 下消费方可能忽略降级标记（标记为显式载荷字段
+ 文档，不引入强制 fail-closed）；暴力扫回退被否决后，「usearch 索引
损坏但模型在位」场景的恢复路径 = 回填器全量重算重建 artifact（分钟级，
非查询期在线恢复）。

**中性**：BM25 通道打分/排序/高亮逐字节零变更（SPEC §2.0）；桌面
search_hybrid 链路维持现状（D19 冻结令零 GUI 面）；bge-m3/clip-vit-b32
的 MANIFEST blake3 None 欠账不在本 WP 强制范围（SPEC §2.3）。

## 修订登记

| 版本 | 日期 | 说明 |
|---|---|---|
| 0.1 | 2026-10-11 | 初稿（随 SPEC M11-WP06 §6-1，实施立项前置件） |
| （预留） | | 多模型通道 / bge-reranker 精排立项时修订（SPEC §4 非目标） |

## 待批准项

- [ ] P25 登记行（随本 WP T02，登记先于测试代码，G1 门禁）
- [ ] graph 依赖边落地后 `cargo deny check` 绿复核（T03+ 实施时）
- [ ] 存储与时长实测对预算线（artifact ≤1.5× 库主文件 / 全量嵌入
  ≤30 min @10⁵ 叶）实测归档复核（SPEC §2.6/§3）
