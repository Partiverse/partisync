# Task: M10-WP05-T01 SPEC 起草（MCP 2.0 正式远程化）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M10-WP05-T01 |
| **类型** | 规格（G0 门槛件，docs-only） |
| **范围** | docs/specs/M10-WP05.md（§1–§7 全节）+ 本卡 |
| **创建日期** | 2026-10-07 |
| **来源** | M10-WP00 §1-WP05 + §2 拍板#3（「先出接线 ADR 再落锤」）+ M10-roadmap-proposal §5-β2/§6-NB3 + M9-report §5 G8 行（◐ 半清偿） |

## 摸底输入（起草期实证 2026-10-07）

- 上游：M9-WP05-mcp2-remote-eval.md（G-1..G-6/A-1..A-5/T-R1..T-R7/O1..O4）、
  M9-WP05 SPEC（骨架交付 v0.2）、M9-report §5 G8 行、M10-WP00 §1-WP05；
- 代码：hub 骨架 mock 边界（crates/partisync-hub/src/mcp_remote.rs:119
  「Bearer 存在即放行」探针钉死）；gateway 11 工具直调面（mcp.rs:609-619）
  与传输解耦（mcp.rs:548）；stdio 唯一入口（mcp.rs:1704 + bin
  partisync-mcp.rs）；
- rmcp 3.4.0 registry 源码：auth 模块纯客户端侧（auth.rs:511）——RS 侧
  token 校验零 SDK 现成件；transport-streamable-http-server 带 Origin/host
  校验与 stateless 协议头开关（tower.rs:206-271）；
- workspace 依赖：无 rustls 线位/jsonwebtoken/oauth2 顶层 pin
  （Cargo.toml:28-85），Cargo.lock 无 oauth2/jsonwebtoken。

## 交付物

`docs/specs/M10-WP05.md`：任务切分 T02 评估+ADR（O2/O3 必答，先评估后
落锤）→ T03 传输面（fail-closed 默认态墙先于门）→ T04 授权面（P23
fail-closed 登记）→ T05 工具透传（mock AS e2e）→ T06 关账；范围裁定
（宿主=gateway、机制不锁死、R2 评审深度、真实外接 AS 端到端 = NB-WP05-1
债条件触发）见 SPEC §7/§4。

## 验收

- [x] SPEC 七节齐备、任务粒度对齐单 PR ≤400 行（超限预登记堆叠判例）；
- [x] 零代码 diff（本 PR 仅 docs/specs/M10-WP05.md + 本卡）；
- [x] 提交挂 Task-ID `M10-WP05-T01`。
