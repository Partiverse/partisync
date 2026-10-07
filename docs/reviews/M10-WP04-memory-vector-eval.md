# M10-WP04-T05 评估：记忆域语义向量检索条件评估

> 性质：**条件评估件（纸面，只评估不实施）**——SPEC M10-WP04 §2.3 六问
> 必答，零代码零依赖变更；结论供后续 WP 立项引用，本 WP 不落实施件。
> 范围纪律沿 M9-WP05-mcp2-remote-eval「只评估不设计」判例：性能数字凡
> 无在仓实测者一律标注「纸面预估」并登记 bench 前置，绝不以估算冒充
> 实测（M8-WP01-bench 判例口径）。证据均为本仓 path:line（2026-10-07
> 实证，main@972dc42）。上游：M9-WP02 §4 + §2.4:98 + 非目标表 :156 /
> M10-roadmap-proposal §6-NB2:61 + §7-2:171-172 / M10-WP04 §1 第三件。

## 0. 输入与基线

- **条件触发债**：M9-WP02 非目标表明文「语义向量检索（embedding 进
  memory 面）→ 条件触发（AI workflow 阶段 / WP03 旗舰入口需求时评估）」
  （docs/specs/M9-WP02.md:156）；M10 拍板 #2「tombstone 先行 + 向量评估
  随行」（docs/specs/M10-WP04.md:28-32）。
- **一期现状**：`memory_search` 引擎 = FTS5 trigram（v17 防御性建表，
  crates/partisync-graph/src/store.rs:351-356，CJK 子串语义正确注释
  store.rs:2532-2536）+ LIKE 兜底（FTS 不可用或查询 <3 字符，
  store.rs:2569-2572）+ tag/json_each 精确过滤 + id 直查（store.rs:2548-
  2562）；评分 = bm25 秩 / 常量，**非语义分**（M9-WP02.md:93）。
- **评估对象切分**：「记忆域语义向量检索」= memory content 经嵌入模型
  映射为稠密向量 + 向量索引近邻检索 + 与既有 FTS/tag 通道融合/并存。
  涉及四个正交面：嵌入（模型与推理时机）、向量索引（结构与 key 域）、
  承诺边界（P20 不变量）、公共工具面（gateway MCP schema）。

## 1. 问题一：既有件盘点——零新增依赖可走多远

### 1.1 嵌入推理（fastembed，workspace pin `=7.0.1`，Cargo.toml:58）

| 件 | 位置 | 状态 |
|---|---|---|
| `ai-embed` feature 门控 | crates/partisync-ai/Cargo.toml:12（`ai-embed = ["dep:fastembed"]`，默认关，ADR-0018 判例） | 在仓 |
| 写入侧 EmbedStage：`EmbeddingModel::BGEM3` 文本 + `ClipVitB32` 图像 | crates/partisync-ai/src/stages/embed.rs:127-137（BGEM3 初始化）、:100（结构）、:1（stage 文档「BGE-M3（文本）」） | 在仓；sidecar 离线管线用，模型缺失/初始化失败 → Skipped 降级（embed.rs:6） |
| 查询侧 QueryEmbedder：`EmbeddingModel::BGESmallZHV15` 512d 懒加载 | crates/partisync-desktop/src/state.rs:101-133（embedder + BGESmallZHV15）、:139-158（embed_query）、:126-127（判例注释「新 embedding 路径不用 legacy 768d TextDense」） | 在仓且**生产已接线**（desktop 旗舰 search_hybrid，ipc.rs:158-171） |
| EvalRunner 真档：BGESmallZHV15 语料+查询双腿 | crates/partisync-index/src/eval/runner.rs:296（BGESmallZHV15）、:243-247（run_hybrid_no_rerank 三阶段文档）、:346（corpus → TextDenseZh512 落库） | 在仓；`index-embed` feature 门控（runner.rs:253） |
| 模型清单钉版 + blake3 校验机制 | crates/partisync-ai/src/models.rs:43-51（MANIFEST：bge-m3/clip-vit-b32 条目） | 机制在仓；**blake3 均为 None**（models.rs:45 注释「2026-09-22 会话 HF 不可达，冒烟移交登记」）——真模型冒烟是全仓欠账，非本 WP 引入 |

### 1.2 向量索引（usearch，workspace pin `2.26`，Cargo.toml:53）

| 件 | 位置 | 状态 |
|---|---|---|
| VectorStore 三子索引：TextDense 768d / ImageDense 512d / TextDenseZh512 512d（均 `MetricKind::Cos` + F32 量化） | crates/partisync-index/src/search/vector.rs:31-42（枚举）、:44-52（dims_and_metric）、:74-80（结构） | 在仓 |
| key 映射：content_id → blake3 前 8 字节 u64；碰撞口径 2⁶⁴ 生日界（≤10⁹ 条目 <2.7%），upsert 以 remove+add 保幂等 | vector.rs:22-28（content_key）、:9-14（碰撞口径注释，SPEC §2.3 点名） | 在仓 |
| 写入面：upsert（幂等覆盖）/ add_new（bulk 跳成员检查） | vector.rs:147-155 / :177-185；remove 成本实测 10⁴ 规模 ~30ms/次超线性（:243-246 与 :170-171，M5-WP05-T02 基准） | 在仓 |
| 读取面：search / search_keys（原始 u64 key 供映射层） | vector.rs:339-375 / :381-393；key 单向不可反解、调用方比对还原（:369-373） | 在仓 |
| 融合层：RRF k=60（BM25 + 向量 + 可选稀疏 + 可选 reranker） | crates/partisync-index/src/search/hybrid.rs:1-13（流程）、:25（`RRF_K = 60.0`）、:94/:104（融合式） | 在仓 |
| 引擎门面：`hybrid_search` / `bm25_only` / `vector_only` / `vector_search_precomputed` | crates/partisync-index/src/search/engine.rs:6、:136、:225 | 在仓 |

### 1.3 生产接线现状（资产域 vs 记忆域）

- **资产域——读面已生产**：desktop `search_hybrid`（ipc.rs:143-180：
  `QueryEmbedder` 出查询向量 → `IndexEngine::hybrid_search`，kind =
  TextDenseZh512）真用户面运行（M8-WP05 旗舰判例）。
- **资产域——CLI/gateway 半接线**：CLI `partisync search --mode hybrid`
  声明 hybrid 但**回退 bm25**（「混合检索需 embedding 模型生成查询向量」
  crates/partisync-cli/src/main.rs:953-960）；gateway asset_search 走
  `bm25_only`（crates/partisync-gateway/src/mcp.rs:1072）。
- **资产域——语料腿缺口**：标准 IndexWriter 的 artifact 加载只认
  `embed_text_dense.bin`（768d）与 `embed_image_dense.bin`（512d）两文件
  （writer.rs:238-263 `load_vector_from_artifact`）；**zh512 的资产侧批量
  落库路径本仓仅 EvalRunner 一处**（runner.rs:346）——桌面真实资产语料腿
  未见（grep `TextDenseZh512` 全仓仅 vector.rs/ipc.rs/state.rs/runner.rs
  四文件命中，2026-10-07）。
- **记忆域——零向量通道**：memory 表无向量列（schema.sql:218-228）；
  检索引擎只有 FTS5+LIKE（store.rs:2540-2650）；memory 域与 index crate
  无任何代码交联。

### 1.4 盘点结论

**组件级零缺口，接线级全缺口。** 「fastembed 嵌入 + usearch HNSW +
RRF 融合」全套件在仓且资产域有生产判例（desktop zh512 链路）；记忆域
要补的是三条腿——写入腿（memory content → 向量落库）、查询腿（查询
嵌入 → 近邻检索 → memory_id 回表）、生命周期腿（update 墓碑/GC 物理删
的向量同步）——全部是**工程接线**，不触依赖面（fastembed/usearch/sqlx
均既有 pin）。两个必须先清的**维度对齐风险**：

1. `TextDense` 768d 被 in-repo 判例明确标注「legacy 占位、无生产模型
   对应、不推荐新代码使用」（vector.rs:33-36；state.rs:126-127 同判例）
   ——且 BGE-M3 真实输出维度**在仓从未实测**（models.rs:45 blake3 None
   冒烟欠账；fixtures 语料自称「768d 简化版」crates/partisync-index/
   tests/fixtures/wp06_corpus/d0019_fastembed_bge_m3.md:59）。
   `ai-embed(BGEM3) → TextDense(768d)` 直连的维度契约是**未验证点**。
2. zh512 路径（BGESmallZHV15 → 512d）已双端实证对齐（desktop 查询腿
   state.rs:126 + EvalRunner 语料腿 runner.rs:346）——若记忆域走中文
   模型，这是唯一维度契约已闭环的现成组合。

## 2. 问题二：三方案对比（叶量 10⁵ 口径）

规模基线：一期叶量口径 10⁴–10⁵（ADR-0029:54「一期叶量 10⁴–10⁵，二叉
排序树足够」）；10⁵ 亚秒全量重算为在仓口径（merkle.rs:10、store.rs:2504
）；增量树重估墙在 10⁶（ADR-0029:73）。512d F32 向量单条 2 KiB，10⁵ 条
≈ 200 MB（纸面算术）。

| 维度 | 方案 A：复用 TextDense 新 key 域 | 方案 B：独立 memory 向量 artifact | 方案 C：sqlite 暴力扫（BLOB 伴表） |
|---|---|---|---|
| 形态 | memory 向量进既有 `text_dense.usearch`，key = blake3(memory_id) 前 8B | 第四子索引或独立 `usearch::Index` 文件（如 `memory_vec.usearch`），key 域 memory_id 专享 | 伴表 `memory_embedding(memory_id, vec BLOB)`，查询全量载入做余弦扫 |
| 新增依赖 | 0 | 0 | 0（sqlx 既有 pin） |
| 维度契约 | ✗ 冲突：TextDense 768d 无生产模型对应（vector.rs:33-36）；BGEM3 维度未实测（models.rs:45）；BGESmallZHV15 512d 根本进不了 768d 子索引 | ✓ 自选维度（512d 对齐 zh512 已验证组合，或冒烟后另定） | ✓ 自选维度，无索引 schema 约束 |
| key 域 | ✗ 与资产 content_id 共享 2⁶⁴ 碰撞空间：search 返回 key 需调用方比对还原（vector.rs:369-373），跨域命中只能丢弃（召回损失；10⁵ 规模概率可忽略但语义混域） | ✓ 域隔离 | ✓ 无 key 域问题（memory_id 主键直连） |
| 资产域耦合 | ✗ 资产 rebuild 全量重扫 sidecar embed-done 集合（writer.rs:201-210），memory 向量混入后互踩 | ✓ 完全隔离 | ✓ 完全隔离 |
| 生命周期钩子 | ✗ 每次更新/GC 都是对共享索引 remove（10⁴ ~30ms/次超线性，vector.rs:243-246） | △ update/GC 单点 remove 同成本；但可整树 rebuild 批量重清（writer.rs rebuild 判例）避开 N×remove | ✓ 同事务删伴表行即可（无超线性删除） |
| 查询延迟（10⁵） | HNSW 毫秒级（desktop 旗舰交互判例） | 同 A | 纸面预估：SIMD ~5–20ms、标量 ~30–150ms（无在仓实测，**bench 前置**） |
| 写入延迟 | usearch add 分摊近 O(log)（容量倍增 reserve，vector.rs:207-215） | 同 A | O(1) BLOB 落表 |
| 10⁶ 展望 | 可用（HNSW） | 可用（HNSW） | ✗ 线性扫不可用（与增量树同一量级墙，ADR-0029:73） |
| 实现量 | 最小但语义最脏 | 中（三腿新写，但每腿有判例可抄：state.rs 查询腿/writer 语料腿/runner 批量腿） | 最小（纯 sqlx；嵌入腿仍要 fastembed） |
| 判例对齐 | ✗ 违背 in-repo「新 embedding 路径不走 TextDense」判例（state.rs:126-127） | ✓ 与 desktop zh512/EvalRunner 判例同构 | 沿 FTS 不可用→LIKE 兜底的自家降级判例（store.rs:2569-2572） |

**对比结论**：A 在维度契约与 key 域两处硬伤，仅当「模型恰好 768d 且
接受混域」才成立——当前无此模型，**A 出局**。B 与 C 都零依赖可行：
10⁵ 口径下 C 的查询延迟纸面可接受且生命周期最干净；B 一步对齐资产域
生产判例且 10⁶ 前景明确。两者**不互斥**：C 可作 B 的冷启动/回退模式
（usearch 子索引未建或损坏时暴力扫兜底，沿 FTS→LIKE 降级判例）。

## 3. 问题三：P20 承诺边界——向量与 embedding 不进叶编码、不进承诺

- **P20 现行口径**（docs/tests/properties.md:27）：根是 memory 承诺集的
  确定性函数（同集同根、叶按 memory_id 升序）；T01 措辞注记（2026-10-06
  ）已钉「承诺集 = memory 全体行（含墓碑）；软删除不动根，GC 动根」。
- **叶编码实况**：`leaf_data` 只编 memory_id/content_hash/tags/metadata/
  created_ns/origin_device 六字段（crates/partisync-graph/src/memory.rs:117-
  131），hlc/deleted 刻意不进（「簿记水位不进承诺」memory.rs:9-10；
  schema.sql:226-227 两列即簿记位）。
- **裁定依据（三条，均为在仓证据链）**：
  1. **确定性/存量兼容**：改叶编码 = 换承诺语义，P20-a「同集同根」对
     存量库立即破坏——ADR-0029 修订登记（docs/adr/0029-memory-schema-
     verifiable-layer.md:84）否决「可见集=活行」备选时已援引同一理由
     （「改叶编码破坏 P20 确定性存量兼容」）。
  2. **模型派生量污染承诺集**：向量随模型/版本/量化参数而变；嵌入进
     叶 ⇒ 换模型即换承诺集 ⇒ 「同集同根」被非用户意图因素打破，且
     重嵌入历史记忆会产生大规模根漂移（GC 级动根常态化），审计语义
     崩坏。
  3. **派生检索面先例**：memory_fts 本身就是 content 的派生索引、不在
     承诺面（v17 注释 store.rs:345-350「不在 schema.sql、静默降级」）
     ——向量通道与 FTS 同地位：**承诺针对记忆内容本身，检索派生面
     可丢可重建**。
- **落点**：向量无论方案 B/C 都落在承诺面之外（独立 artifact 或伴表
  BLOB）；伴表若动 schema 走 additive-only 幂等迁移（v16/v17/v18 判例，
  schema.sql:238-247 content_chunk 为最新判例）；`memory_root`/
  `memory_count`/`tombstones` 口径零变化；memory_write/update/delete/GC
  的根刷新语义零变化。**向量缺失/损坏永不影响 memory_verify 结果**
  （verify 只看叶编码六字段 + content_hash 列级校验，memory.rs:68-76）。

## 4. 问题四：公共面变更路径（pre-merge-safety R2 + ADR 触发条件）

- **现状公共面**：`memory_search` MCP 工具 schema = query/tag/memory_id/
  limit/offset 五参（crates/partisync-gateway/src/mcp.rs:480-492），
  `tools/list` 是对外唯一权威契约（M9-WP02.md:98）；graph 侧签名
  `memory_search(query, tag, memory_id, limit, offset)`（store.rs:2540-
  2547）；instructions 摘要（mcp.rs:550-560）。
- **变更形态（二选一）**：
  - (i) `memory_search` 增**可选**语义参数（如 `mode: "hybrid"`，缺省
    现行为）——后向兼容字段增量，既有客户端零破坏；
  - (ii) 新工具（如 `memory_search_semantic`）——不动既有 schema 但
    工具面分叉。
  建议取 (i)：语义通道是检索增强而非新实体，分叉双工具徒增
  instructions/文档面；且工具数变化牵连 tools/list 断言的风险（SPEC
  M10-WP04 §6-R5 已核无总数锁）在形态 (i) 下为零。
- **必经路径（实施立项时逐条执行）**：
  1. **SPEC 先行**（铁律 1）：新 WP/任务卡 + SPEC 修订，钉死参数
     schema、模型与维度契约、key 域、降级语义（嵌入失败回退 FTS——
     沿 sidecar Skipped 降级与 FTS→LIKE 兜底双判例）；
  2. **pre-merge-safety R2**：MCP tool pub API 命中覆盖表「public API →
     dual human review + SPEC version bump」（.zcode/skills/partisync-
     pre-merge-safety/SKILL.md Coverage map 行 6）；PR 描述附钉子清单
     disposition 块；R2 全审逐验收 bullet；
  3. **新 ADR 一枚**（判定见下）；instructions 文本同步（mcp.rs:550-560）；
  4. **桌面**：若桌面跟进检索入口则触发 GUI 实操硬性规则（AGENTS.md
     桌面端验收规则）；MCP 侧桌面零改动判例可沿用（M10-WP04 §2.4
     mcp_call 透传）。
- **是否超出 ADR-0018 锚定 → 须新 ADR：是。** ADR-0018 影响范围条款
  明文「受影响 crate：partisync-index（新增）；**不受影响**：其他所有
  crate」（docs/adr/0018-usearch-tantivy-bge-reranker-index-deps.md:75-
  79）——记忆域接线必触 partisync-graph（写入/伴表面）+ partisync-
  gateway（公共工具面），超出其锚定；且新增模型清单条目（如
  BGESmallZHV15 进 models.rs MANIFEST——现为 desktop 直 new 未入清单，
  state.rs:110-116）须补 blake3 钉版（models.rs:43-45 机制）。
  **但依赖面无新开项**：fastembed =7.0.1 / usearch 2.26 / sqlx 0.9 均
  既有 pin（Cargo.toml:35/:53/:58），ADR-0018 的依赖决策继续有效，新
  ADR 的标的只是**记忆域语义检索面**（模型/维度/key 域/公共 schema/
  降级语义），不是新依赖批次。

## 5. 问题五：成本/性能口径

> 除注明「实测」外均为纸面预估；**实施立项前必须 bench 冒烟**
> （M8-WP01-bench 判例；models.rs:45 冒烟欠账是全量数字的第一缺口）。

**写入侧（每条记忆一次嵌入）**

- 模型体积/加载：BGE-M3 ~2.3 GB（在仓语料引用 fixtures/wp06_corpus/
  d0019_fastembed_bge_m3.md:38）；BGESmallZHV15 显著更小且桌面旗舰已
  装机懒加载运行（state.rs:101-133，冷启动零模型加载设计 ipc.rs:147-
  149）。
- 单条推理延迟：短文本（memory content 上限 64KiB，mcp.rs:691）CPU
  纸面量级 10⁰–10¹ ms（小模型）/10²–10³ ms（BGE-M3）——**无在仓实测**。
- 热路径隔离建议（届时设计题）：嵌入为派生面（§3），应**异步伴生**、
  不进 memory_write 事务——嵌入失败/滞后不影响承诺面与同步收敛，
  沿 sidecar 离线管线判例（embed.rs:6「失败同映射 Skipped」）。

**索引维护**

- usearch add 分摊近 O(log)（容量倍增 reserve，vector.rs:207-215）；
  **remove 超线性为实测**：10⁴ 规模 ~30ms/次（vector.rs:243-246，
  M5-WP05-T02 基准）——memory_update = 墓碑+新行 ⇒ 每次更新 1 remove +
  1 add；GC 批量物理删 ⇒ 届时应整树 rebuild 重清（writer.rs rebuild
  判例）而非 N 次 remove。
- 存储：512d F32 单条 2 KiB，10⁵ 条 ≈ 200 MB（HNSW 索引或 BLOB 伴表
  同量级，纸面算术）。

**查询侧（10⁵ 叶量口径）**

- HNSW：毫秒级检索（desktop 旗舰交互使用为在仓运行判例；10⁵ 规模
  HNSW 属常识量级，立项时以 bench 钉实）。
- 暴力扫（方案 C）：10⁵ × 512d ≈ 5.1×10⁷ MAC/查询——SIMD 纸面 ~5–20ms、
  标量 ~30–150ms，**无在仓实测，bench 前置**。
- 参照系：仓库现行全量操作口径 10⁵ 亚秒（根重算 merkle.rs:10/store.rs:
  2504）；FTS5 trigram 现网查询为索引检索（亚毫秒–毫秒级）。
- 交互预算结论：查询嵌入一次（同写入成本）+ 近邻扫，总延迟由**嵌入
  推理主导**而非向量检索；交互式 UI 场景小模型（zh512）是现成解。

## 6. 问题六：拍板建议 + 实施触发条件

### 拍板：**条件不成熟，暂不立项实施**（本 WP 只存档本评估件）

理由（证据链）：

1. **无痛点实证**：一期叶量口径 10⁴–10⁵（ADR-0029:54），FTS5 trigram
   已覆盖 CJK 子串检索（store.rs:351-356）；「语义近似但字面不同」的
   查询失败至今无在仓记录/用户登记——M9-WP02:156 的条件触发前提
   （「AI workflow 阶段 / WP03 旗舰入口需求时」）尚未发生。
2. **先决验证件未清**：TextDense 768d 无生产模型对应（vector.rs:33-36
   ）、BGE-M3 维度/冒烟欠账（models.rs:45）——维度契约（§1.4 风险 1
   ）必须先清；记忆域三腿（写入/查询/生命周期）是纯增量工程，在
   WP04 tombstone+GC 刚收敛的当下不宜叠加。
3. **方向既有排序**：M10-roadmap §7-2（:171-172）明文「生命周期面
   收敛后检索深化才有稳定对象」——本 WP 收敛生命周期后，检索深化
   自然排后续窗口。

### 届时实施形态建议（存档备查，非本 WP 承诺）

- **主选 = 方案 B**（独立 memory 向量通道，独立 key 域；模型走
  BGESmallZHV15 512d 对齐 desktop/EvalRunner 已验证组合），
  **方案 C 暴力扫作其冷启动/回退模式**（沿 FTS→LIKE 降级判例）；
  方案 A 出局（维度契约 + key 域混域双硬伤，§2）。
- 公共面走 `memory_search` 可选参数增量（§4 形态 i）；新 ADR 一枚
  （记忆域语义检索面，§4）；嵌入异步伴生不进写事务（§5）。

### 实施触发条件（T1–T4；T1–T3 任一满足即**启动立项评估**，立项实施建议三者齐备且 T4 对齐）

| # | 触发条件 | 判据来源 |
|---|---|---|
| T1 | **规模**：memory 叶量 ≥ 10⁵，且 FTS 通道零召回/低召回有实证记录（如 MCP 侧查询计数透出） | ADR-0029:54 量级墙前留量；M9-WP02:156 |
| T2 | **需求**：AI workflow / WP03 旗舰入口出现「语义近似但字面不同」查询实证 | M9-WP02.md:156 条件行原文；M10-roadmap §6-NB2:61 |
| T3 | **先决清偿**：嵌入模型冒烟完成（真实维度实测 + MANIFEST blake3 钉版，models.rs:45 TODO 清偿）+ 维度契约/key 域/降级语义在 SPEC 落锤 | models.rs:43-51；§1.4 风险 1 |
| T4 | **窗口**：与 M10 二期检索深化 / WP03 向量检索 UI 入口立项窗口对齐 | M10-roadmap-proposal §6-NB2 / §7-2:171-172 / §8 WP03 行:112 |

### 超出 ADR-0018 锚定的判定（SPEC §2.3 第 6 问点名项）

**是，实施立项须新 ADR**——判定与标的见 §4 末条：ADR-0018:75-79 影响
范围只锚定 partisync-index；记忆域接线触 graph+gateway+模型清单扩位。
依赖 pin 本身（fastembed/usearch/sqlx）无需重开依赖 ADR，无 cargo deny
新增项（本评估零依赖变更，与 SPEC §3 全程行一致）。

## 7. 诚实登记（未做/未验证）

- 未运行任何嵌入模型推理（真模型冒烟需下载权重；models.rs:45 登记
  HF 不可达欠账在先）——§5 全部推理延迟为纸面预估并标注。
- 未运行暴力扫 micro-bench（零代码任务约束；触发立项后随实施件补）。
- 「zh512 资产侧批量落库路径未见」为 2026-10-07 grep 全仓结论（§1.3
  ），不排除库外手工流程；实施立项时须复核。
- SPEC §1 草稿引 `schema.sql:226` 为 deleted 列，实测当前行号 :227
  （本文件一律以实测行号引用）。
