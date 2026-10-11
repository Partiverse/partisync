# Task: M12-WP04-T06 P2 轮 3 前置——通道对证 + ADR-0036 草稿

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M12-WP04-T06 |
| **类型** | docs-only（ADR 草稿，状态=提案待 @lead 核签；核签前零 Cargo 面改动） |
| **来源** | M12-WP04-T01 提案 P2 包 + 用户拍板「按推荐案走」（P2 排轮 3，ADR 前置） |
| **创建日期** | 2026-10-12 |

## 交付

docs/adr/0036-cli-partisd-client.md（新，草稿/提案）：通道对证实证（rmcp 3.4.0 registry 源码自带 `client` + `transport-streamable-http-client` 特性，Cargo.toml:42-56）→ 决策 = CLI 复用既有 pin 零新顶层依赖、新边 CLI→rmcp 登记；备选四枚否决理由；后果（reqwest 传递树入锁的 deny/audit 复核义务）；**待核签三项**（边登记/reqwest 接受度/stop 语义三选一）+ Re-evaluation 条件。

## 涉及文件清单（Iron Rule 9）

docs/adr/0036-cli-partisd-client.md（新）· docs/tasks/M12-WP04-T06-p2-transport-adr.md（本卡）

## 验收

- [x] 对证基于 pinned registry 源码行号（非记忆），沿铁律「禁止凭记忆编写第三方 API」；
- [x] 状态 = 草稿（提案待核签）——零 Cargo 面 diff、零实施动作；
- [x] 备选 ≥3 枚各附否决理由（铁律 8「无聊依赖」记录质量在备选）；
- [x] 编号 T06 续 SPEC 任务表（T02–T05），无撞号；
- [x] docs-only；纯两文件新增；提交挂 Task-ID `M12-WP04-T06`。

> **0.1 补记（2026-10-12）**：用户超长任务运行授权「签位默认确认」→
> ADR-0036 落**已接受**（v1.0，@lead 默认确认核签）；三项处置落锤
> （stop = shell-out `kill`，unix-only 登记）。核签 PR 与本补记同车。
