# Task: M11-WP04-T04 真库实测(存储域增量 / reindex 耗时增量 / 中文高亮前后对照)

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP04-T04 |
| **类型** | docs 实测任务(SPEC M11-WP04 §2.3/§3 的 T04 实测复核面;零产品代码、零依赖变更) |
| **来源** | SPEC M11-WP04 §3 验收项「T04 实测报告」+ §2.3 性能承诺(存储 ≤+35% / reindex <5%);T01 代价评估(docs/reviews/M11-WP04-stored-field-cost-eval.md)入线拍板的实测复核义务 |
| **创建日期** | 2026-10-10 |
| **执行日期** | 2026-10-11 |
| **口径** | release 双二进制(cli-main@2c321d3 / cli-t03@02c06f5)同机同库对照;/tmp=tmpfs;/usr/bin/time -p wall;每轮冷建;main/t03 交错 + 换序复测 |
| **语料** | partisync-big.db 原样(实测 0 文档可索引,登记)+ LCSTS 真实中文语料(仓外 ~/Data/lcsts/,经真实 partisync index 路径同构灌入两侧沙箱库,6000 文档主口径)+ T01 风格英文合成对照层(机制隔离) |

## 交付

docs/reviews/M11-WP04-t4-eval-evidence.txt(新,全链证据):中文主口径存储域增量 **+9.5%**(≤+35% 达线,
ZH-A 层 +10.3% 互证,.store 域内 +26.9% 与 T01 +31% 判定吻合、postings 零增量);reindex 全链耗时增量
**+7.8%**(合并 6 轮中位;两组 3 轮协议 +9.0%/+5.2%,t03 6/6 轮全慢,CPU +8.6% 佐证,**超 <5% 预算线**,
与 T01 <1% 估算的失真已用英文对照层隔离:英文 +0.0% 墙钟,代价为 orig 域 LZ4 压缩真实中文 UTF-8 的
固有成本,非 T03 代码缺陷);中文高亮前后对照原样归档(main 碎片流 vs t03 可读原文 + [[..]] 包裹,
与 SPEC §2.2/§6-R2/R3 形态一致);检索等价 top-20 命中集(score+content_id)逐项零差异、top-3 分数
逐位相同(SPEC §2.0 行为钉实证)。预算线裁决(存储绿 / reindex 红)交 WP 责任人。

## 涉及文件清单(Iron Rule 9,SPEC §5 T04 面)

docs/reviews/M11-WP04-t4-eval-evidence.txt(新)·
docs/tasks/M11-WP04-T04-eval.md(本卡)

## 验收(SPEC §3 T04 面;留待评审勾选)

- [ ] 真库 reindex 前后对照在案:存储域增量 ≤ +35%(实测 +9.5%,达线)
- [ ] reindex 耗时增量对照在案且如实判定(<5% 预算 vs 实测 +7.8%,超线,证据与机制隔离在案)
- [ ] CLI search 中文高亮前后对照输出归档(碎片流 vs 可读原文,原样摘录)
- [ ] 检索等价旁证在案(top-20 命中集 score+content_id 零差异)
- [ ] 诚实边界登记(prescribed 语料 0 文档可测、负载漂移、段拓扑、transcript 通道未单测、search 延迟未压测)
- [ ] 预算线裁决经 WP 责任人复核(reindex <5% 对中文主导语料实测超线:维持预算/修订预算/改存储形态)
