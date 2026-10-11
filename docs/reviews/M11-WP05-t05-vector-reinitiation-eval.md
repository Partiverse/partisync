# M11-WP05-T05 评估:语义向量检索二期——重触发立项评估(T1 触发后)

> 版本: 0.1 · 性质: **立项评估件(纸面,只评估不实施)**——沿
> M10-WP04-T05 判例与 memory-vector-eval §6 触发纪律 · 触发依据:
> **T1 已满足**(叶量 118076 实测,docs/reviews/M11-WP05-t1-corpus-evidence.txt:37;
> 用户裁决「T1 语料满足」已登记,docs/tasks/M11-WP05-T04-t1-corpus.md:44)
> · 本件 = 对 M11-WP05-t03-vector-initiation-eval.md(下称 t03)「实施
> 立项:暂缓」判定的复核升级 · 证据均为本仓 path:line(2026-10-11 于
> /tmp/wp-m11-reeval,基 main@2c321d3)或本件全文引录的本次运行实录
> 负责人: @partiverse · 批准人: @lead

## 0. 触发与纪律

memory-vector-eval §6 触发表:「T1–T3 **任一满足即启动立项评估**;
实施立项建议三者齐备且 T4 对齐」(M10-WP04-memory-vector-eval.md:248,
逐条判据 :252-255)。t03 时点 T2 已满足、T1 硬条件未满足,判定暂缓
(t03 §3:42-44)。**2026-10-10 T1 已清偿**:LCSTS 语料批量导入实插
118076 叶(count(*) 实测 ≥10⁵,幂等重跑同根,verify ok=true;
M11-WP05-t1-corpus-evidence.txt:35-51),FTS 低召回实证在案(100 条
语义探针 98% 零命中 :89;正控 total=14 存活 :228-229,详录 :72/:75,
同件),且 T1 判定经用户复核裁决「T1 语料满足」(M11-WP05-T04-t1-corpus.md:44,
随 #228 登记)。**本件 = T1 触发的重立项评估**。评估 ≠ 实施;实施
另须 T1–T3 齐备 + T4(:248)。

## 1. T1–T4 逐项复核

| 触发 | 现状 | 证据 |
|---|---|---|
| T1 叶量 ≥10⁵ 且 FTS 低召回有实证 | **已满足**:120000 行尝试 → 实插 118076(语料内 1924 条幂等去重),root=b84dace1…27575 全链 verify ok=true,重跑实插 0 且同根;FTS 通道 100 条 LCSTS summary 探针 98/100 零命中、正控「立法法」total=14 管道存活、LIKE 兜底 2 字全表扫 0.181s 旁证;**判定经用户裁决登记** | t1-corpus-evidence.txt:35-51/:89(98% 零命中)/:205-208/:228-229(正控 total=14);任务卡裁决行 M11-WP05-T04-t1-corpus.md:44;证据链 = 导入器 #226(dcc21e7)→ 全链证据 #227(87a1333)→ 裁决登记 #228(2c321d3) |
| T2 语义缺口查询实证 | **已满足(自建语料降格口径维持 t03 登记)**:Q1 `revenue report`/Q2 `会议纪要` 确定性 0 命中、对照字面命中、语料自洽查询命中、index=3/reindex=3 全链归档;**证据强度 = 自建语料可复现演示,低于真实使用中出现**——t03 §5:68 降格与 G10 评估件 v0.2 §0 降格口径原样沿用,本件不再升格 | t2-query-evidence-v2.txt:13-23;M11-WP05-g10-workflow-eval.md:32-42/:108-111; t03 §1:22 |
| T3 模型冒烟 + 维度契约 | **冒烟腿已清偿(本次实质进展)**:BGESmallZHV15(fastembed 7.0.1 生产同版本 pin,Cargo.toml:58)实测 **dim=512**(10 句全 512,见 §2 引录),逐句延迟真中位 3.52ms(harness 打印 3.6ms 为上中位
lat[5],修正口径见 §2 注),init 144ms;**五个模型文件 blake3 全套算出备料**(§2 哈希表)供 MANIFEST 填实(models.rs:43-51 机制在仓、blake3 均 None)。t03:23 的「BGESmallZHV15 冒烟 3 次代理断流未完成」欠账就此清偿,BGEM3 1024d 实测维持。**未清偿残段**:MANIFEST 钉版落码与「维度契约/key 域/降级语义在 SPEC 落锤」(判据后半句,M10-WP04-memory-vector-eval.md:254)属实施件,本件只备料 | 本件 §2 实录;models.rs:43-51;t03 §1:23 |
| T4 窗口对齐 | **建议(非自动)**:M11-WP04(检索深化)非目标已明排「语义向量检索实施:候选 4 维持暂缓,线独立」(docs/specs/M11-WP04.md:108-109)——该行写于 T04 之前,其暂缓理由「T1 语料门未满足」已被本次 T1 清偿事实覆盖,但 WP04 线边界不改(不在 WP04 线内顺手改,M11-WP04.md:108 表述以本件为准登记事实更新);**实施位建议随 WP06 SPEC 批准后窗口**(新立实施 SPEC 承载,详见 §3);外部需求锚 = partiverse S4「M3 前(≈2027-03)解锁」(M11-roadmap-proposal.md:216,t03 §4:57-60) | docs/specs/M11-WP04.md:104-109;M11-roadmap-proposal.md:216 |

## 2. T3 冒烟实录(本会话实际运行,2026-10-11)

仓外临时 harness /tmp/bge-smoke(fastembed `=7.0.1` 与 workspace 同 pin,
Cargo.toml:58 同版;10 句中文知识库风格语料,`src/main.rs:22-33`),模型
缓存 Xenova/bge-small-zh-v1.5(snapshot 75c43b069aac4d136ba6bc1122f995fedcfd2781,
下载腿经 hf-mirror 固化;本次运行缓存命中、未走网络)。运行命令与输出全文:

```
$ HF_HOME=/tmp/bge-smoke/cache FASTEMBED_CACHE_DIR=/tmp/bge-smoke/cache \
  HF_ENDPOINT=https://hf-mirror.com /tmp/bge-smoke/target/release/bge-smoke
[init] model loaded in 144.107687ms
[warmup] 2 vectors, dim=512
[dim] all dims: [512, 512, 512, 512, 512, 512, 512, 512, 512, 512]
[latency] per-sentence ms: [3.080436, 3.1567689, 3.198838, 3.416419, 3.471535, \
          3.572524, 3.681356, 3.708141, 3.8925769, 4.408807]
[latency] median = 3.6 ms, min = 3.1 ms, max = 4.4 ms
[latency] full-batch(10) = 13.2 ms
[blake3] f1c661f38644b20ef51d58cfb9cf92cf21ed80ed796c8994eb78cfd4f4764529  onnx/model.onnx (94851877 bytes)
[blake3] 2948cdb566aa39990222fc687d6b2b214f91c602627c7f664b8748114756afad  tokenizer.json (439125 bytes)
[blake3] 8455defe17a2d02c705ccd02d2703a401e9a012b172041ab1dfc89ee734b081b  config.json (716 bytes)
[blake3] 4bd4620b7434b5da448bf9a6cd8859aec19f7bdefacf6f2dd6f0eac70ccfa259  tokenizer_config.json (367 bytes)
[blake3] a18633786802dd5a6c0bd6b1a99ea58d6c846b73efd862830fdb86318943fdff  special_tokens_map.json (125 bytes)
EXIT=0
```

> **延迟口径修正(对抗复核发现,2026-10-11)**:上引输出中
> `[latency] median = 3.6 ms` 为 harness 打印的 **lat[5] 上中位**
> (10 元素排序后第 6 个,/tmp/bge-smoke/src/main.rs:53),非真中位;
> **真中位 = (3.471535+3.572524)/2 = 3.52ms**。复核者以
> HF_HUB_OFFLINE=1 离线复跑同二进制复现 lat[5] 行为(打印 3.5,
> dim=512 与五文件 blake3 逐字节一致)。本件/任务卡/SPEC 一律以
> 真中位 3.52ms 口径引用,原始输出保持逐字不改;harness 本体为仓外
> 临时件,未修改。

**MANIFEST 候填哈希表**(供实施件钉版 models.rs:45 用;source =
Xenova/bge-small-zh-v1.5,即 fastembed `EmbeddingModel::BGESmallZHV15`
实际拉取 repo;钉版时须沿 ModelSpec 逐文件登记):

| 文件 | blake3 | 字节 |
|---|---|---|
| onnx/model.onnx | f1c661f38644b20ef51d58cfb9cf92cf21ed80ed796c8994eb78cfd4f4764529 | 94,851,877 |
| tokenizer.json | 2948cdb566aa39990222fc687d6b2b214f91c602627c7f664b8748114756afad | 439,125 |
| config.json | 8455defe17a2d02c705ccd02d2703a401e9a012b172041ab1dfc89ee734b081b | 716 |
| tokenizer_config.json | 4bd4620b7434b5da448bf9a6cd8859aec19f7bdefacf6f2dd6f0eac70ccfa259 | 367 |
| special_tokens_map.json | a18633786802dd5a6c0bd6b1a99ea58d6c846b73efd862830fdb86318943fdff | 125 |

**维度契约落锤建议**:t03 §2:32-34 倾向「方案 B 维持 512d 选型
(BGE-small-zh-v1.5)」——本次 512d 由「in-repo 判例」(TextDenseZh512,
crates/partisync-index/src/search/vector.rs:39-41)升格为**新鲜实测**;desktop/EvalRunner 已验证组合判例
维持(state.rs:125-127/:141/:154;memory-vector-eval.md:35-36)。BGEM3
1024d(t03:23 实测)仍列资产域旗舰通道候选,不入本件范围。

## 3. 立项判定(按触发热纪律,与 t03 对照升级)

**实施立项:放行(有条件)**——对照 t03 §3「暂缓(不成立)」升级,
理由:T1 硬条件已清偿(118076 叶实测 + 用户裁决,§1),t03 暂缓的
唯一硬理由(t03:42-43「无 10⁵ 语料,为不存在的数据量建向量通道 =
伪造触发」)不再成立;T2 已满足;T3 冒烟腿已清偿(§2)。

**放行前置条件(全部满足才开工,缺一即退回)**:

1. **新立实施 SPEC**(落锤四件,随 WP06 SPEC 批准后窗口,T4 对齐):
   ①维度契约 = 方案 B 512d(§2);②memory 专享 key 域(独立于资产域
   content_id key 域,沿 crates/partisync-index/src/search/vector.rs:22-28 content_key 判例以 memory_id
   → blake3 前 8B u64);③降级语义 = 方案 C 暴力扫作冷启动/回退
   (沿 FTS→LIKE 降级判例,M10-WP04-memory-vector-eval.md:241-243;
   暴力扫延迟纸面 ~5–20ms/查询,:214)——回退机制备选注:本件 §6-3
   亦提出「向量腿不可用时回退现状 BM25 通道(FTS5+LIKE)」表述,两处
   均为待 SPEC/ADR 落锤的建议,张力由实施 SPEC §6-Q2 二选一显式覆盖;
   ④MANIFEST blake3 钉版
   (models.rs:45-51,用本件 §2 哈希表);
2. **新 ADR 一枚(记忆域语义检索面)**——实施立项须新 ADR:ADR-0018
   影响范围只锚定 partisync-index,记忆域接线触 graph+gateway+模型
   清单扩位(M10-WP04-memory-vector-eval.md:259-262);
3. **R2 评审面**:memory_search 公共面走可选参数增量(M10-WP04-
   memory-vector-eval.md:245);若 SPEC 选扩 VectorKind 枚举路线则 =
   公共 API change(t03 §2:37-38),R2 + pre-merge-safety;
4. **依赖方向裁决**:graph(领域层)不得反向依赖 index(能力层)
   ——crate 地图硬规则(AGENTS.md crate 地图);向量组件复用
   vector.rs/usearch/hybrid.rs 的具体接线位(见 §4)是 SPEC 必答项。

放行 ≠ 自动开工:规格先行(Iron Rule 1),无已批准 SPEC 不写实现。

## 4. 实施方案建议(届时 SPEC 落锤,本件只登记事实与建议)

- **目标域 = memory 域(记忆域语义向量检索),非资产域**:评估对象
  标题即「记忆域语义向量检索条件评估」(M10-WP04-memory-vector-eval.md:1),
  对象切分 = memory content 嵌入 + 近邻检索 + 与既有 FTS/tag 融合
  (:22-25);实证基线 = 记忆域零向量通道——memory 表无向量列
  (:65-67;本仓复核 crates/partisync-graph/src/schema.sql:218-228,
  memory 表九列(:219-227),全列 TEXT/INTEGER,无任何 BLOB 向量列)。
  资产域已有 desktop zh512 生产链路
  (memory-vector-eval.md:52-55),不在本件范围。
- **组件盘点(零新依赖成立)**:「fastembed 嵌入 + usearch HNSW +
  RRF 融合」全套件在仓——usearch pin Cargo.toml:53、fastembed pin
  Cargo.toml:58;VectorStore 子索引模式(crates/partisync-index/src/search/vector.rs:32-42)、key 映射
  (crates/partisync-index/src/search/vector.rs:22-28)、RRF k=60 融合层(crates/partisync-index/src/search/hybrid.rs:5-10/:25)。盘点结论
  =「组件级零缺口,接线级全缺口」(memory-vector-eval.md:71-76):
  要补写入腿/查询腿/生命周期腿三条工程接线。
- **落点建议**:接线位置须 SPEC 落锤,候选 = ①独立 memory_vec.usearch
  artifact + 伴生索引(承诺面之外,M10-WP04-memory-vector-eval.md:137),
  在 graph 侧直用 usearch/fastembed 既有 pin(新增 crate 依赖边而非
  新顶层依赖,不触铁律 8,但依赖图新边宜随 ADR 一并登记);②任何
  graph → index 反向依赖须 ADR(AGENTS.md crate 地图)。**扩枚举与
  key 域混域是 t03/一期评估双重否决的方案 A 出局项**
  (memory-vector-eval.md:241-244;t03 §2:37-38)。
- **写入时机**:嵌入异步伴生不进写事务(memory-vector-eval.md:246)
  ——T04 导入判例(118076 叶 31s 批量直插,t1-corpus-evidence.txt:35)
  表明嵌入腿若同步入写路径会量级化拖慢写入。

## 5. 存储代价预估(纸面类比,沿 T01 stored 评估口径)

**纪律**:本节全部为纸面预估,标注「纸面」并登记 bench 前置——沿
memory-vector-eval §7「性能数字凡无在仓实测者一律标注纸面预估」
(M10-WP04-memory-vector-eval.md:5-7)与 T01 stored 代价评估的实测锚定
口径(M11-WP04-stored-field-cost-eval.md:41-42 规模外推未实测如实标注;
:56-59 实测修正先行担忧的判定方式)。

| 项 | 纸面推算 | 锚点 |
|---|---|---|
| 原始向量体积 | 118076 × 512d × 4B(f32) = **241,819,648 B ≈ 230.6 MiB** | dim=512 本次实测(§2);f32/Cos 沿 VectorStore 三子索引判例(crates/partisync-index/src/search/vector.rs:44-52 dims_and_metric 全 Cos);字节数经 python3 复算(241,819,648;对抗复核纠正此前误写 241,811,456,偏差 8,192 B,结论不变) |
| 相对语料库体量 | ≈ T1 语料 graph.db 主文件的 **1.04 倍**(主文件实测 233,230,336 B) | /home/acme/Data/lcsts-memory/graph.db 实测(2026-10-11 ls;仓外 T04 产物) |
| 全量嵌入时长 | 逐句真中位 3.52ms → ≈ **6.9 分钟**;批量口径 full-batch 13.2ms/10 句 → ≈ **2.6 分钟** | §2 实测延迟线性外推(单机实测,量级参考;上中位 3.6ms 口径 ≈7.1 分钟,真中位修正见 §2 注,偏差 ~2% 不改量级结论) |

类比结论:向量通道的存储增量与 T01 stored 原文域(+31% 存储域、
reindex <1%,M11-WP04-stored-field-cost-eval.md:56-58)同数量级可接受,
**但这是纸面判断**——实施 SPEC 须含 usearch 索引落盘实测(含 HNSW
图开销,非裸 f32)与全量嵌入/单查询延迟 bench 前置。

## 6. 非目标与风险

**非目标**:资产域向量面改动(desktop zh512 链路维持现状);
bge-reranker 精排(crates/partisync-index/src/search/hybrid.rs:13-14
  feature 默认关,不在最小闭环);
G10 编排联动;任何 GUI(冻结令,AGENTS.md 桌面端规则;UI 归
Partiverse 域);分词器/fan-out 域改造(M11-WP04.md:106-107 非目标,
线独立)。

**风险**:

1. **嵌入模型供应链**:模型 = Xenova/bge-small-zh-v1.5 ONNX(94.85MB
   实测),下载腿经 hf-mirror;t03 曾实测 BGEM3 对 hf-mirror 协议不兼容
   须代理直连(t03 §1:23)——本件 §2 为缓存命中离线推理,**下载腿协议
   兼容性本次未复测**;blake3 钉版(models.rs:40 校验机制)是供应链
   防护核心,实施件首任务必须先钉版再接线;
2. **存储代价**:§5 全部纸面;usearch HNSW 图开销未实测;
3. **降级诚实边界**:向量腿不可用时沿 FTS→LIKE 降级判例回退
   (memory-vector-eval.md:243;store.rs:2533/:2540-2650 判例),**回退语义 =
   回到现状(FTS5 trigram + LIKE),现状本身已实证低召回**——降级
   语义在 SPEC 落锤时须明确「低召回是已知基线,不因加向量腿而恶化,
   也不承诺向量腿补齐所有语义缺口」;
4. **生命周期腿**:update 墓碑/GC 物理删的向量同步是三腿之一
   (memory-vector-eval.md:72-76),漏接 = 已删记忆仍可被向量召回,
   SPEC 须含 tombstone/GC 同步契约(P20 不变量面复核)。

## 7. 诚实边界

- **T2 降格**:语义缺口实证为自建三文件语料可复现演示(t2-query-
  evidence-v2.txt:4-6,语料为临时件、v2 补录原文),非真实使用流量
  中出现;t03 §5:68 降格口径原样沿用,本件不升格;
- **T1 语料真实口径**:LCSTS = 公开微博短文本摘要数据集
  (t1-corpus-evidence.txt:4),非本产品生产使用流量;叶量 118076 为
  导入实测(幂等、verify ok,真实数字),但语料内容分布 ≠ 生产记忆
  分布——语义检索收益在真实工作负载的表现未证,本件判定的是
  「触发条件成立」,不是「收益已证」;
- **冒烟为单机实测非 CI**:§2 实录为本机单次手工运行(仓外
  /tmp/bge-smoke release 二进制,2026-10-11),无 CI 复现管道;
  环境未做温度/负载控制,延迟数字为量级参考;哈希钉版时建议实施件
  以可复现下载流程(HF_ENDPOINT=hf-mirror 直连)再验哈希一致;
- **l2norm 未测**:t03 BGEM3 冒烟含 l2norm=1.0 检查(t03 §1:23),
  本次 harness 无归一化探针——归一化语义与 MetricKind::Cos
  (crates/partisync-index/src/search/vector.rs:44-52)的匹配性留 SPEC 落锤时复测;
- **纸面预估清单**:§5 存储/时长全部纸面;暴力扫延迟引一期评估纸面
  数字(:214)未复测;
- **冒烟延迟口径修正(对抗复核发现,2026-10-11)**:harness 打印的
  `median` 实为排序后第 6 元素上中位(/tmp/bge-smoke/src/main.rs:53,
  `lat[5]`),非真中位;真中位 = (3.471535+3.572524)/2 = **3.52ms**。
  本件/任务卡/SPEC 一律以真中位口径引用(§5 外推 6.9 分钟,较误标
  7.1 分钟偏差 ~2%,量级结论不变);harness 本体为仓外临时件,未修改
  ——实施件重写探针时应打印双元素均值。复核者以 HF_HUB_OFFLINE=1
  离线复跑同二进制复现 lat[5] 行为(dim=512 与五文件 blake3 逐字节
  一致,本件哈希表维持有效);
- **本件为纸面评估,零代码零依赖变更**;本 WP 新增文件仅本件与
  任务卡两件,未改任何仓内既有文件(本件与任务卡的后续修订属
  自身勘误,不触 Iron Rule 9)。
