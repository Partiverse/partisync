# Task: M12-WP04-T07 WP04 SPEC v0.2——轮 2/3/4 契约补全

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M12-WP04-T07 |
| **类型** | docs-only（SPEC 修订，铁律 1；实施任务卡按排期纪律 v0.2.0 tag 后开工） |
| **来源** | M12-WP04-T01 提案（#254）+ 用户拍板「按推荐案走」+ ADR-0036 已接受（#259，默认确认核签） |
| **创建日期** | 2026-10-12 |

## 交付

docs/specs/M12-WP04.md v0.1→v0.2：①轮 2 契约 §4（P1 机读契约层：--json 全动词增量旗标——成功 stdout 单行 compact JSON/失败零 stdout+stderr 互斥、长驻动词启动摘要诚实边界、默认输出逐字节不变；退出码契约 docs/CLI-CONTRACT.md + ls 空 1→0/watch 中断 130 两处行为统一显式登记）；②轮 3 契约 §5（P2 四动词：partisd status/ping/stop/search --daemon，通道=ADR-0036 已接受决策）；③轮 4 契约 §6（P4 doctor 六类体检表 + 降级语义 + 退出码规则）；④任务表扩至 T03–T10（T07 本稿/T08 P1/T09 P4/T10 P2，实施全落 tag 后）；⑤非目标节相应收缩。

## 涉及文件清单（Iron Rule 9）

docs/specs/M12-WP04.md · docs/tasks/M12-WP04-T07-spec-v02.md（本卡）

## 验收

- [x] 四轮契约齐备（轮 1 §2–3 / 轮 2 §4 / 轮 3 §5 / 轮 4 §6），tag 后实施零 SPEC 等待；
- [x] 行为变化显式登记（ls 空结果退出码、watch Ctrl-C）——不静默；
- [x] 长驻动词进度流结构化 = 诚实边界非目标（§4.1/§9）；
- [x] 轮 3 引用 ADR-0036 已接受状态（#259），通道面不再开放问题；
- [x] docs-only；纯两文件；提交挂 Task-ID `M12-WP04-T07`，CI 全绿后 squash 合入。
