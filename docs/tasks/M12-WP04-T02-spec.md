# Task: M12-WP04-T02 WP04 SPEC 起草（轮 1 P3 UX 硬化）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M12-WP04-T02 |
| **类型** | docs-only（SPEC 起草，铁律 1；实施任务卡按排期纪律 v0.2.0 tag 后开工） |
| **来源** | M12-WP04-T01 细化提案（#254，用户拍板「按推荐案走」）+ charter WP04 |
| **创建日期** | 2026-10-11 |

## 交付

docs/specs/M12-WP04.md（新）v0.1：轮 1 = P3 UX 硬化——§2 契约四节（help 基建旗标表单源/未知 flag 白名单校验 exit 2/T08 ⑩ 微债清偿/clap 不引入拍板）+ §3 验收六项（四形态输出断言/同源性结构钉/16 动词合法调用回归/零顶层依赖零 unsafe）+ §5 R1 分级 + §6 任务切分 T02–T05（编号续 T01 提案；T03–T05 实施落 v0.2.0 tag 后）。轮 2–4 以修订版滚动。

## 涉及文件清单（Iron Rule 9）

docs/specs/M12-WP04.md（新）· docs/tasks/M12-WP04-T02-spec.md（本卡）

## 验收

- [x] 契约全部基于盘点 path:line 事实（main.rs:35-72/T08 证据件 :134-135）；
- [x] clap 不引入拍板显式登记（新顶层依赖回避，铁律 8）；
- [x] 任务编号续 T01 无撞号；实施时点 = v0.2.0 tag 后（charter 排期纪律）；
- [x] docs-only；纯两文件新增；零代码 diff；
- [x] 提交挂 Task-ID `M12-WP04-T02`，CI 全绿后 squash 合入。
