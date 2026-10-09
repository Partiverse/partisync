# Task: M11-WP02-T04 partisd 守护进程实体化

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP02-T04 |
| **类型** | 实施(新 crate bin;壳→能力层;R1) |
| **来源** | SPEC M11-WP02 §2.3/§3.0-T04 + ADR-0032 决策 1/3/5 + partiverse S1 |
| **创建日期** | 2026-10-09 |

## 交付

1. crates/partisd 实体化:替换 8 行骨架——组装(graph + 只读索引 + 扩展
   注册表,复用 gateway `build_server_state`)+ loopback MCP 服务面
   (rmcp StreamableHttp,默认 127.0.0.1:7650,与 sidecar 同工具面)
2. 生命周期:pidfile 独占创建防双实例(陈留 pid 经 /proc 存活检查覆写
   接管);SIGTERM/SIGINT 优雅退出(移 pidfile、退出码 0)
3. 红线落码:非环回 bind 启动守卫直接拒绝(ADR-0032 决策 5)
4. 探针 tests/wp02_daemon.rs ×3:pidfile 双实例拒绝 + loopback
   initialize 200(MCP 协议载荷)+ 非环回拒绝;SIGTERM 优雅退出断言
   内嵌(退出码 0 + pidfile 移除)

## 涉及文件清单(Iron Rule 9)

crates/partisd/Cargo.toml · crates/partisd/src/main.rs ·
crates/partisd/tests/wp02_daemon.rs(新)·
docs/tasks/M11-WP02-T04-partisd-daemon.md

## 验收(SPEC §3.1 T04 行)

- [x] partisd 启动 → loopback MCP 工具面可用(initialize 200 + 协议载荷)
- [x] pid 文件防双实例(第二实例非零退出 + 指明双实例)
- [x] SIGTERM 优雅退出码 0 + pidfile 移除
- [x] 非环回 bind 拒绝(红线落码)
- [x] fmt/clippy/test 绿;零新增顶层依赖(全部 workspace 既有:
      gateway/rmcp/axum/tokio);改动仅限本卡清单
