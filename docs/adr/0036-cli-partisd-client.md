# ADR-0036: CLI↔partisd loopback 客户端面——复用 rmcp 3.4.0 client 传输特性

状态: **已接受**（2026-10-12，@lead 默认确认核签——超长任务运行授权
「签位默认确认」指令落档，沿 ADR-0033/0034 会话内核签判例）
日期: 2026-10-12 · 决策人: @lead（默认确认机制，用户 2026-10-12 指令
「都让你执行超长任务了，默认确认核签」）
关联: docs/reviews/M12-WP04-cli-enhancement-proposal.md（P2 包）+
docs/specs/M12-WP04.md（轮 3 预告）+ ADR-0019（rmcp =3.4.0 精确锁定）+
ADR-0032（partisd 形态：loopback MCP 服务面 + 127.0.0.1 信任域决策 5）

## 背景（对证事实，2026-10-12 main）

- CLI 与 partisd **完全脱节**：16 动词全部直连库，CLI 无任何 MCP/HTTP
  客户端依赖（盘点 crates/partisync-cli/Cargo.toml 全表）——同一查询
  CLI `search` 恒降级 BM25（main.rs:954-962），partisd `--memory-embed`
  有 hybrid 三腿；`status/ping/stop` 管理动词缺失（pidfile TCP 探活已有，
  partisd main.rs:57-97，仅服务启动互斥）。
- 通道需求 = MCP streamable-HTTP 客户端，目标 `http://127.0.0.1:7650/mcp`
  （partisd main.rs:250-265）。
- **关键对证**：workspace 既有 pin **rmcp 3.4.0 自带 client 侧特性**
  （registry 源码 Cargo.toml:42-56）：`client` /
  `transport-streamable-http-client` / `transport-streamable-http-client-reqwest`
  等——协议与传输层由既有 pin 承载，**零新 workspace 顶层依赖**
  （reqwest 仅作 rmcp 特性的传递依赖进 Cargo.lock，非
  `[workspace.dependencies]` 条目）。

## 决策（一句话）

CLI 侧复用 workspace 既有 `rmcp = "=3.4.0"` pin 的 `client` +
`transport-streamable-http-client` 特性作为 partisd loopback 客户端通
道；新边 **CLI → rmcp** 登记于本 ADR（沿 ADR-0035 新边登记判例）。

预期 Cargo 面 diff（实施 PR 随本 ADR 已接受后落地）：

```toml
# crates/partisync-cli/Cargo.toml [dependencies]
rmcp = { workspace = true, features = ["client", "transport-streamable-http-client"] }
```

管理动词：`status`/`ping` = pidfile 解析 + `std::net::TcpStream` 探活
（零新依赖）；`stop` 语义见「待核签问题 ③」。

## 备选（已否决）

| 备选 | 一句话理由 |
|---|---|
| 新顶层 reqwest + 手写 MCP 协议层 | 新顶层依赖（铁律 8 ADR 面）+ 协议层手写维护成本，rmcp 已承载全部——纯劣 |
| hyper 直用（axum 传递已在锁） | 同上手写协议层；仅省一个传递依赖树，得不偿失 |
| CLI stdio 直 spawn partisync-mcp 子进程 | 不解决「复用已运行 daemon（含向量腿）」语义——sidecar 面现状即此，正是 P2 要消除的能力差 |
| rmcp `transport-streamable-http-client-unix-socket` | partisd 现为 TCP loopback（ADR-0032），改传输面超本轮范围（留 re-evaluate 备选） |

## 后果

- **正**：协议/传输层零自研；零新顶层依赖（deny `[bans]`/licenses 面
  零改动）；CLI 获得与 partisd 同能力的 hybrid 检索 + 管理动词。
- **负**：①reqwest 传递树入 Cargo.lock（audit/deny 扫描面扩大——实施
  PR 必附 `cargo deny check` + `cargo audit` 输出复核，本 ADR 落「后
  果」节留痕）；②CLI 编译面增 rmcp client 特性（编译时长小幅上升）；
  ③loopback 明文 HTTP——沿 ADR-0032 决策 5 信任域设计内，不新增暴露
  面。

## 待核签问题（@lead）——**2026-10-12 已全部默认确认处置**

1. ✅ 新边 **CLI → rmcp** 登记批准（层级：壳→外部既有 pin，沿 ADR-0035
   判例——壳层向下无违规）；
2. ✅ reqwest 传递依赖树入锁接受（deny/audit 复核义务随实施 PR，见
   「后果」节）；
3. ✅ `stop` 动词语义 **= shell-out `kill <pid>`**（零依赖；unix-only
   如实登记——Windows 面沿 partisd pidfile 现状口径注记局限；daemon
   增 MCP `shutdown` 工具留作后续轮次可选增强，不阻塞 P2）。

## Re-evaluation 条件

rmcp 3.4.0 安全公告；partisd 传输面变更（unix-socket/TLS——届时
unix-socket 备选重启）；reqwest 主版本颠覆或 RUSTSEC 公告进入传递树。

## 修订记录

| 版本 | 日期 | 内容 |
|---|---|---|
| 0.1 | 2026-10-12 | 草稿（提案）：通道对证（rmcp 3.4.0 client 特性实证）+ 决策/备选四枚/后果/待核签三项。核签前不实施 |
| 1.0 | 2026-10-12 | **已接受**（@lead 默认确认核签，超长任务运行授权）；待核签三项处置落锤（stop = shell-out kill，unix-only 登记）。实施随 WP04 轮 3（charter 排期：v0.2.0 tag 后），实施 PR 附 deny/audit 复核 |
