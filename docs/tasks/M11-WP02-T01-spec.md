# Task: M11-WP02-T01 SPEC 起草 + ADR-0032 + D5 复现实证归档

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP02-T01 |
| **类型** | docs-only（SPEC 起草 + ADR + 实证归档；R1 定级随 WP 整体声明） |
| **来源** | M11 提案 §3-WP02 + §1.2-4（D5 基线义务）+ M10-WP03 §6-D5/D3 + partiverse docs/05 §9.3-S1 |
| **创建日期** | 2026-10-09 |

## 交付

1. SPEC docs/specs/M11-WP02.md（§2.0 并发模型裁定 = 多读者只读打开 +
   按需写者；T01–T06 任务表；P24 预登记义务随 T02）
2. ADR docs/adr/0032-partisd-daemon-form.md（决策 1 形态 / 2 并发模型 /
   3 loopback MCP 复用 / 4 sidecar 保留 / 5 安全边界）
3. D5 复现实证归档 docs/reviews/M11-WP02-d5-repro-evidence.txt
   （lsof `10wW` 独占持锁 + CLI search LockBusy 全文，Kubuntu 本机
   CLI 面复现，沿「验证走 CLI」指令免 GUI）

## 涉及文件清单（Iron Rule 9）

docs/specs/M11-WP02.md · docs/adr/0032-partisd-daemon-form.md ·
docs/reviews/M11-WP02-d5-repro-evidence.txt · docs/tasks/M11-WP02-T01-spec.md

## 验收（SPEC §3.0 T01 行）

- [ ] SPEC 落档（§1–§7 齐备，D5 三选对证落锤，P24 预登记义务写明）
- [ ] ADR-0032 落档（决策 1–5 + 待批准项）
- [ ] D5 复现实证归档（持锁实证 + 冲突复现两段齐备）
- [ ] fmt/clippy/test 三件套全绿（docs-only 不触码，CI 复核）；零代码
      diff；提交挂 Task-ID `M11-WP02-T01`
