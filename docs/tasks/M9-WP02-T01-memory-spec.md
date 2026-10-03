# Task: M9-WP02-T01 可验证记忆层一期——SPEC 起草 + ADR-0029 + 竞品摸底

> **范围外**：schema/proof API/MCP 工具实装随 T02–T05（SPEC 批准后按
> 任务卡另立）；本任务纯文档，零代码改动。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP02-T01 |
| **类型** | SPEC 起草（铁律 1 规格先行；G0 门） |
| **优先级** | P0（M9 主线 α 首件；WP01 已收官，依赖序就位） |
| **范围** | SPEC M9-WP02 草稿 + ADR-0029 草稿 + 竞品摸底报告 + 本卡；批准方式 = PR 合入（沿 M9-WP00 PR #115 判例，合入 = 批准 + M9-WP00 §2「记忆层一期范围」拍板行落锤「本地 stdio 先行」） |
| **创建日期** | 2026-10-03 |
| **来源** | M9-WP00 §1-WP02（WP01 收官后依赖序首位）+ M8-report §5（CAS exists() 微债承接）+ M9-roadmap-proposal §7-2（拍板默认） |

## 交付物

1. **竞品摸底**：`docs/reviews/M9-WP02-memory-competitor-scan.md`——
   mem0/Letta/MCP reference server 工具面与可验证性机制对照；核心结论
   = 两家头部均无 Merkle/内容寻址/密码学审计，可验证记忆层为生态空白
   （N2 判断经核查维持）。
2. **ADR-0029 草稿**：`docs/adr/0029-memory-schema-verifiable-layer.md`
   ——memory 为 graph 新实体域（additive 迁移 + oplog entity 新臂）+
   独立 RFC 6962 式二叉证明树（与 P7 对账分桶树并存）；B1–B4 备选与
   重估触发器落档。
3. **SPEC M9-WP02 草稿**：`docs/specs/M9-WP02.md`——§2 契约（schema/
   proof API/三工具/同步语义/CAS exists() 精化/P20 候选）+ §3 八条可
   执行验收 + §4 非目标（Hub/向量检索/delete/推理写入等七项归属）+
   §5 文件清单 + §6 七项风险处置。

## 验收

- [x] 摸底先于起草（roadmap 落锤的开工前置）；报告逐项注明来源与
      「未查到」显式标注；
- [x] SPEC 六节齐全，验收逐条可判定且引用 P 编号（P6/P7/P19/P20）；
      新不变量 P20 显式标记走 partisync-property-registry（登记先于
      T02 测试代码）；
- [x] graph schema 演进有 ADR（M9-WP00 §1-WP02 硬性要求）；
- [x] 零代码改动、零依赖变更；提交挂 Task-ID；
- [ ] PR 合入（= 批准）后 T02 按批准 SPEC 开工。
