# M11-WP05-T03 评估:语义向量检索二期——立项评估(T2 触发后)

> 版本: 0.1 · 性质: **立项评估件(纸面,只评估不实施)**——沿
> M10-WP04-T05 判例与 memory-vector-eval §6 触发纪律 · 触发依据:
> T2 满足(docs/reviews/M11-WP05-t2-query-evidence.txt,v2 全链归档;
> 触发判定见 M11-WP05-g10-workflow-eval §0/§4.4)· 证据均为本仓
> path:line 或工作流实测实录(2026-10-10,eval-inputs 汇总)
> 负责人: @partiverse · 批准人: @lead

## 0. 触发与纪律

memory-vector-eval §6 触发表:「T1–T3 任一满足即启动立项评估;实施
立项建议三者齐备且 T4 对齐」。**T2 已满足**(2026-10-10 归档,v2 补
全链:index=3/reindex=3/语料自洽对照),本件 = 被触发的立项评估。
评估 ≠ 实施;实施另须 T1–T3 齐备 + T4。

## 1. T1–T4 逐条现状(全部为 2026-10-10 工作流实测/核验)

| 触发 | 现状 | 证据 |
|---|---|---|
| T1 叶量 ≥10⁵ 且 FTS 低召回 | **未满足**:全机 memory 叶量峰值 3 行(27 个测试残留 graph.db 扫描);无任何 ≥10⁵ 生产语料(partisync-big.db 为资产域 10³,无 memory 表;LCSTS 903MB/2.4M 行原档未留盘) | eval-inputs 汇总·向量立项 findings#2(verified) |
| T2 语义缺口查询实证 | **已满足**(自建语料可复现演示,v2 全链归档;证据强度注记:低于真实使用中出现,复核意见已采纳进 G10 评估件 v0.2) | t2-query-evidence-v2.txt |
| T3 模型冒烟 + 维度契约 | **半清偿(本次实质进展)**:BGEM3 经 fastembed 7.0.1 生产同路径实测 **dim=1024**(embed 89.1ms,l2norm 1.0;hf-mirror 协议不兼容,须代理直连——基建缺口登记);两权重文件 blake3 已算出备料;**BGESmallZHV15 冒烟 3 次均代理断流未完成**(512d 判据仍为 in-repo 判例非新鲜实测) | eval-inputs·向量立项 findings#1/#2 段 |
| T4 窗口对齐 | **部分**:M11 检索深化 SPEC 未起草;partiverse S4「M3 前(≈2027-03)解锁」= 外部需求锚已登记 | S4 原文摘录(eval-inputs) |

## 2. 维度契约落锤(本次实测的直接产出)

BGEM3 实测 1024d **正式证伪**「ai-embed(BGEM3)→ TextDense(768d)直
连」旧假设(vector.rs TextDense 判例注释本就自认「legacy 768d 占位,无
生产模型对应」)。若立项:**方案 B 模型选型须重weighing**——
BGESmallZHV15(512d,对齐 TextDenseZh512 判例与 desktop/EvalRunner 已
验证组合,但冒烟 3 次未完成)vs BGEM3(1024d,已实测,模型体积 2.3GB)。
倾向:方案 B 维持 512d 选型(BGE-small-zh-v1.5),BGEM3 1024d 另列
资产域旗舰通道候选——**届时 SPEC 落锤,本件只登记实测事实**。

存储侧:memory 表无向量列(schema.sql:218-228);VectorStore 三子索引
可沿 TextDenseZh512 判例机械扩第四通道或独立 memory_vec.usearch,零新
依赖,但扩枚举 = 公共 API change(R2 + 新 ADR,与评估件 §4 判定一致)。

## 3. 立项判定

**实施立项:暂缓(不成立)**——T1 未满足是硬条件(无 10⁵ 语料,语义
检索的规模收益无法实证;为不存在的数据量建向量通道 = 伪造触发)。

**转为「准备态」并给出清偿清单**(全部小件,随任一窗口执行):
1. models.rs MANIFEST blake3 填实(哈希已备料,eval-inputs findings#1);
2. BGESmallZHV15 冒烟重试(代理断流问题已定位:换端点/本地加载,
   文件清单已备于 /tmp/fastembed-smoke/local-bge-m3);
3. 维度契约 + memory 专享 key 域 + 降级语义随 M11-WP04(检索深化)
   SPEC 一并落锤。

**再触发条件**:真实语料叶量逼近 10⁵(partiverse S4 场景落地)或
用户指认新的语义检索需求 → 重走本评估(事实部分可全部复用)。

## 4. partiverse S4 对齐

S4 原文:「向量检索解锁(解除 M10-WP04 暂缓拍板)……M3 前」
(≈2027-03,partiverse V3 窗口)。本评估结论 = **S4 可达**:
准备态清偿清单 ~小时级,M12/M13 任一窗口可完成实施立项;风险仅在
T1 语料积累(依赖真实使用增长,非工程项)。

## 5. 诚实登记

- BGESmallZHV15 512d 未新鲜实测(3 次代理断流)——512d 判据为在案
  判例,非本次测量;「BGEM3 1024d」为本次新鲜实测;
- 未跑方案 B/C 的任何检索质量对照(需真实语料与冒烟模型,前置依赖
  同上);
- T2 演示为自建语料(复核意见已采纳降格),非真实使用流量;
- 本件为纸面评估,零代码零依赖变更。
