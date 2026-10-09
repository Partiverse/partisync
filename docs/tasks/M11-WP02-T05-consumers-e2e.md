# Task: M11-WP02-T05 三消费者并发 e2e 探针

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP02-T05 |
| **类型** | 实施(进程级 e2e 探针,R1) |
| **来源** | SPEC M11-WP02 §2.4/§3.0-T05 + P24-b/c |
| **创建日期** | 2026-10-09 |

## 交付

crates/partisd/tests/wp02_consumers.rs:partisd(loopback MCP 常驻)+ partisync-mcp
(stdio,rmcp 3.4.0 握手带 _meta 双键)+ partisync-cli search 三只读消费者 ×
写者 3 批(IndexEngine open_or_create 按需写者)并发——读面全程零 LockBusy、
种子可见性保持(10);fresh 只读实例对账 25 条全量(写者侧无问题)。

**发现(SPEC §6-7 登记)**:跨进程 reader 自动 reload 未生效——sidecar 长持
只读 reader 在外部写者 commit 后未自动重载(fresh 同刻见全量)。OnCommitWith
Delay 的 directory watch(MmapDirectory file_watcher,500ms 轮询 meta.json
checksum)在跨进程写者场景未触发。D5 核心目标(读面无 LockBusy)不受影响;
新鲜度缺口转底座改进债(partisd 定时 reload / 写事件触发 / 升级 tantivy 复核)。

## 涉及文件清单(Iron Rule 9)

crates/partisd/tests/wp02_consumers.rs(新)· crates/partisd/Cargo.toml
(dev-dep partisync-index)· docs/specs/M11-WP02.md(§6-7/§7 0.3)

## 验收(SPEC §3.1 T05 行)

- [x] 三消费者并发 e2e 探针绿(读面零 LockBusy + 种子可见性保持;
      §6-7 跨进程 reload 缺口如实登记转改进债)
- [x] fmt/clippy/test 绿;改动仅限本卡清单;零新增顶层依赖
