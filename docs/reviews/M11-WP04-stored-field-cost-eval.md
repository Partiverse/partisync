# M11-WP04-T01 评估:检索摘要 stored 原文域——代价评估(入线拍板前置)

> 版本: 0.1 · 性质: **代价评估件(纸面 + 实测,只评估;评估红 = 诚实
> 撤回不入线)**——沿 M11 提案 §3-WP04「代价评估先行」拍板(§4-5 默认
> 生效 2026-10-09)· 证据 = 工作流实测实录(eval-inputs 汇总·stored
> 成本,全部 verified)+ 仓内 path:line
> 负责人: @partiverse · 批准人: @lead

## 0. 背景与评估问题

中文检索摘要碎片流设计债(M10-WP01 §4):stored 摘要 = CJK fan-out
文本(单字+bigram 碎片),「命中词高亮」对中文内容近似失效。清偿方案
= 增加 stored-only 原文字段(反 fan-out 摘要取原文)。**代价疑问**:
stored 膨胀多大?全量 reindex 影响多大?——本件以实测回答。

## 1. 机制实测(先弄清膨胀发生在哪)

**关键修正(实测发现)**:放大**不在存储层,在写入入口**——upsert/
upsert_batch 在 add_text 之前先对 ocr/transcript 做 cjk_fan_out()
(bm25.rs:394-398/428-433,实现 :219-246),STORED 字段落盘的**本就是
扇出后的串**(search highlight 呈「笔 记 笔记 …」形即此因,bm25.rs
get_first 取到的正是 stored fan-out 串)。纯中文 run 的扇出膨胀 =
11n−7 字节 vs 原文 3n(**理论 3.667x**,标点/数字断 run 实测 3.168x)。

schema 现状:六字段全部 STORED(bm25.rs:66-81;tantivy 0.26 `TEXT`
常量 stored:false,M10-WP01-T02 特意改的)。**「stored 原文域」方案 =
新增一个 stored-only 原文字段(不参与索引,零 postings 增量),与既有
扇出域并存**——不是替换。

## 2. 放大与成本实测(eval-inputs·stored 成本,全部 verified)

| 项 | 实测 | 说明 |
|---|---|---|
| 扇出串放大(纯中文) | **3.168x**(100KB 原文 → 324,621B 扇出串) | 标点/数字断 run |
| tantivy .store 域落盘(LZ4) | **2.70x**(单文档 100KB)~ **3.96x**(200×2KB 文档) | 压缩按小块,回收率不同 |
| 全索引目录体积 | **8.53x / 9.41x**(direct vs fanout) | **postings/positions 域主导**(tokens 35.2x:1683→253,800),非 .store |
| 写耗时(200×2KB) | direct 6.0ms vs fanout 16.5ms(2.75x) | add 线性 + commit 含 ~5-10ms 固定成本 |
| stored 翻倍增量 | 写阶段 **+7.9ms**;全链(裸二进制 0.10s)≈ **+8%**;cargo run 口径 +1.8% | 边际 ≈5.7ns/stored-byte ≈ 40µs/2KB 文档 |
| reindex 基线 | 200 文件 cold/warm 中位 **0.44s**(±5%;tmpfs,真机磁盘未测) | 含 cargo 启动 0.33s,裸 0.10-0.11s |

环境:i5-12600KF/62GB,/tmp=tmpfs(无磁盘 IO 瓶颈,计时纯净);规模
外推(1M+ 文档)未实测,按 ~5.7ns/stored-byte 线性纸面推断。

## 3. 代价判定(对「stored-only 原文域」方案)

1. **存储**:新增原文 stored 字段 ≈ 内容原字节数 1x;相对现行 stored
   (已含 3.17x 扇出串)增量 ≈ **+31%**(1x/3.17x)——非翻倍。
   v0.1 时的「~2x」预估**偏高,实测修正**;
2. **postings/positions 零增量**(stored-only 不分词不入索引)——
   当前索引体积主导域(8.5~9.4x 的真正大头)完全不受影响;
3. **reindex 写阶段**:+31% stored 字节 ≈ +5ms/200 文档(按 5.7ns/B),
   全链增量 **<1%**;
4. **查询侧**:get_first 取原文字段,反 fan-out 摘要 = 截断/定位原文,
   无额外开销。

**判定:代价绿(可接受)**——「~2x 存储膨胀 + 全量 reindex 不可承受」
的先行担忧在实测面前不成立:真实增量 = 存储域 +31% 与 reindex 全链
<1%。高亮修复收益(中文命中词定位从近似失效恢复)对旗舰检索入口为
直接用户价值。

## 4. 入线建议与实施要点(届时 SPEC 落锤)

- **建议入线**:方案 = 新增 `FIELD_OCR_TEXT_ORIG`/`FIELD_TX_ORIG`
  stored-only 字段;反 fan-out 摘要 = 直接取原文字段做词定位(高亮
  逻辑从扇出串迁移到原文串);**全量 reindex 一次性成本 <1s/200 文档
  量级(实测外推)**;
- 实施排期建议:与本 WP 拍板后的检索深化 SPEC 合并起草(高亮修复与
  摘要来源同面);
- 附带收益:本评估的 fan-out 放大实测(8.5~9.4x 目录级)顺带回答了
  「索引体积为什么涨」的历史疑问——扇出串进 postings 是主因。

## 5. 诚实登记

- tmpfs 计时(无磁盘 IO);真机磁盘 fsync 场景增量会更高,未测;
- 1M+ 文档规模外推为纸面(200/400 文档两点实测,线性外推);
- 放大率复刻工程为 /tmp/bench-wp04/fanout-bench(tantivy 0.26.2 同版,
  schema 逐字复刻,未入仓——命令与输出已在 eval-inputs 汇总归档);
- transcript_text 通道未单独测(与 ocr 同函数同构,推断适用);
- release 模式 CLI 未测(计时按 debug 口径如实报告)。
