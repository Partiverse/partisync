# M9-WP05 评估：MCP 2.0（2026-07-28 spec）远程化对齐

> 性质：评估报告（纸面，spec 对齐面基于 2026-10-04 modelcontextprotocol.io
> 与 docs.rs 在线实证；无实测负载）。范围纪律沿 M7-WP03-enterprise-topics
> 「只登记不设计」判例：本报告评估差距与威胁面，**不设计**正式远程化方案。
> 上游：M9-WP00 §1-WP05 / §4 债表 G8 行；M9-roadmap-proposal §3-δ/§5-WP05/
> §6-G8。结论供 SPEC docs/specs/M9-WP05.md 承接。

## 1. 输入与基线

- **债务锚点**：G8「MCP 仅本地 stdio」（M8 前瞻 G8 → M9-WP00:73，承接为
  WP05「评估 + 骨架」）。窗口敏感：MCP 2.0 于 2026-07-28 定稿（roadmap
  §1「双定稿窗口 6–12 个月」），本仓库 rmcp 已精确锁定该版 spec
  （Cargo.toml:64 `rmcp = "=3.4.0"`，ADR-0019 注释「精确锁定 2026-07-28
  spec」）——协议版本对齐已天然成立，缺口全部在**传输与授权面**。
- **现状盘点（path:line 实证 2026-10-04）**：
  - `crates/partisync-gateway/src/mcp.rs:1-5`——9 工具（asset_search/
    asset_read/asset_organize/dataset_export/job_status + memory_write/
    memory_search/memory_verify），传输 stdio
    （`rmcp::transport::io::stdio()`），「无连接状态」注释在案；
  - `crates/partisync-gateway/src/mcp.rs:519`——`ServerHandler for
    McpServerState`（call_tool/list_tools 直调，handler 级与传输解耦）；
  - `crates/partisync-gateway/src/mcp.rs:1579`——`serve_server(state,
    stdio())` 唯一运行入口；
  - `crates/partisync-gateway/Cargo.toml:34`——features `["server",
    "transport-io"]`；`:47-50`——Windows target 已例外启用
    `server-side-http`（桌面 sidecar stdio 不可用场景），说明 rmcp 的
    HTTP 服务器形态在仓库已有最小暴露，但从未作为远程服务面评估。
- **评估对象切分**：「远程化」= 把既有 MCP 工具面经 Streamable HTTP 暴露
  给非同进程客户端。涉及三个正交面：传输（stdio → Streamable HTTP）、
  授权（无 → OAuth 2.1 RS）、部署边界（本机子进程 → 网络服务）。

## 2. 2026-07-28 spec 对齐面（差距分析）

以下规范性引文均出自 modelcontextprotocol.io 2026-07-28 版（2026-10-04
fetch 实证）：`basic/transports/streamable-http`、`basic/authorization`、
`basic/authorization/authorization-server-discovery`、
`basic/security_best_practices`。

### 2.1 传输面：stdio → Streamable HTTP

| # | 2026-07-28 要求（MUST/SHOULD） | 现状 | 差距 |
|---|---|---|---|
| G-1 | 服务器 **MUST** 提供单一 MCP endpoint 支持 POST（例 `https://example.com/mcp`）；每条 JSON-RPC 消息独立 POST | stdio 行协议，无 HTTP 面 | 传输整体缺席；rmcp 3.4.0 提供 `StreamableHttpService`（feature `transport-streamable-http-server`，docs.rs 2026-10-04 实证），gateway 未启用 |
| G-2 | **2026-07-28 移除 GET 流端点与协议级 session**（Mcp-Session-Id/DELETE/Last-Event-ID 均非本版机制；收到旧头部 **忽略不铸造**）——spec 原文「MCP is stateless and has no protocol-level sessions」 | stdio 无连接状态 | **无协议级差距**：2026-07-28 的无状态形态与既有 handler 直调模型同构；应用层状态（state handle）须服务器自管并绑定已验证身份（见 T-R5） |
| G-3 | 每 POST **MUST** 带 `MCP-Protocol-Version` 头且与 body `_meta.io.modelcontextprotocol/protocolVersion` 一致（否则 400 + JSON-RPC `-32020 HeaderMismatch`）；`Mcp-Method`（全部请求）与 `Mcp-Name`（tools/call 等）头 REQUIRED；服务器 **MUST** 校验头-体一致 | stdio 无头概念 | 骨架只做握手面不做全量校验；正式接线时该项是 StreamableHttpService 的合规责任面（未逐项核验，登记 §5-O1） |
| G-4 | 服务器 **MUST** 校验 `Origin` 头（存在且非法 → 403，防 DNS rebinding）；本地运行 **SHOULD** 只绑 127.0.0.1 | 不适用（无 HTTP） | 骨架沿 hub-demo 默认 bind 127.0.0.1:8090（hub-demo.rs:29）；Origin 全量校验属正式接线面 |
| G-5 | 客户端 Accept **MUST** 含 `application/json` + `text/event-stream`；服务器逐请求选单 JSON 或请求级 SSE | 不适用 | 骨架恒返回单 JSON（握手面无流式需求）；SSE/`subscriptions/listen` 属正式接线面 |
| G-6 | 未实现的 RPC 方法 → **404 + `-32601`**（与遗留 HTTP+SSE 服务器的裸 404 区分）；notification 被接受 → **202 Accepted** 无体 | 不适用 | 骨架直接实现这两条（§SPEC 验收），它们是握手面自足语义 |

### 2.2 授权面：OAuth 2.1 Protected Resource Metadata

| # | 2026-07-28 要求 | 现状 | 差距 |
|---|---|---|---|
| A-1 | MCP server（HTTP 传输）作为 **OAuth 2.1 resource server**；**MUST** 实现 RFC 9728 PRM；PRM 文档 **MUST** 含 `authorization_servers`（至少一个 AS） | 无任何授权面 | 授权整体缺席；骨架只产出 PRM **文档形状** + 401 挑战，不验证 token |
| A-2 | 401 响应 **应带** `WWW-Authenticate: Bearer resource_metadata="<PRM URL>"`（spec 例示路径 `/.well-known/oauth-protected-resource`）；PRM 放置两种合法形态：endpoint 路径插入 `/.well-known/oauth-protected-resource/{path}` 或根路径 | 无 | 骨架同时挂根 PRM 端点 + 401 挑战头，两者指向一致 |
| A-3 | 服务器 **MUST** 校验 token 签发给本服务器（受众绑定，RFC 8707 resource indicator）；无效/过期 → 401；scope 不足 → 403 + `WWW-Authenticate error="insufficient_scope"`；**MUST NOT** 接受/转投非本服务器 token（token passthrough 显式禁止，security_best_practices） | 无 | **骨架明确不做**（SPEC §非目标）；正式接线的最大安全面——见 T-R1/T-R2 |
| A-4 | 客户端注册三机制：Client ID Metadata Documents（SHOULD）/ 预注册 / RFC 7591 DCR（已 deprecated） | 无 | 不做；正式接线时 AS 选型（自建 vs 外接）独立 ADR |
| A-5 | AS 发现：客户端按优先序探测 `/.well-known/oauth-authorization-server[/{path}]` 与 openid-configuration；issuer 串等值校验 | 无 | AS 侧不在本仓库范围（`authorization_servers` 指向外部/mock） |

### 2.3 工具面

协议语义层（tools/list、tools/call、错误口径）在 `McpServerState` 已
与传输解耦（mcp.rs:519），理论上可被任一传输复用；但**本 WP 不接线**——
远程面承载真实数据 = 部署边界变更（见 T-R6），必须先有授权实装，顺序
不可倒置。

## 3. 威胁模型要点（远程化新增面）

沿 M7-WP03-enterprise-topics §2.3 判例格式（威胁/现状缓解/缺口/缓解
方向，只登记不设计）。T-R* 编号与 M7 的 T-S* 序列衔接。

| # | 威胁 | 现状缓解 | 缺口 | 缓解方向（不设计） |
|---|---|---|---|---|
| T-R1 | token 受众错配 / passthrough：接受非本服务器 token = 跨服务重放 + 审计链断裂（spec security_best_practices「Token Passthrough」节，MUST NOT） | 无（无远程面） | A-3 全量缺席 | RS 侧强制 audience 校验（RFC 8707 `resource` 绑定）；骨架期以「不承载真实数据」兜底 |
| T-R2 | confused deputy（proxy 场景：静态 client_id + DCR + consent cookie 跳过用户同意） | Hub 非代理架构（不自建 registry 同源约束），风险结构性低 | 若未来 Hub 代理第三方 API 才成真 | per-client consent + state 校验；登记为正式远程化 ADR 必答题 |
| T-R3 | DNS rebinding（无 Origin 校验的本地 HTTP 服务被浏览器侧脚本驱使） | hub-demo 默认绑 127.0.0.1（hub-demo.rs:29） | Origin 校验 MUST 未实现（骨架也不实现，demo 面暴露） | 正式接线时 Origin 白名单 + 非 localhost 绑定须配 TLS |
| T-R4 | SSRF（OAuth 发现 URL 被恶意服务器指向内网/云元数据端点） | 本 WP Hub 只**产出** PRM，不消费发现 URL | 客户端侧面风险；Hub 作为客户端消费他人 PRM 的场景不存在 | 若未来做联邦 MCP 互调，沿 spec 客户端缓解（HTTPS 强制 + 私网段封锁） |
| T-R5 | state handle hijacking（2026-07-28 无协议会话，跨请求状态靠应用层 handle；handle 被冒用即越权） | stdio 面单进程单用户，天然无此面 | 远程化后任何跨请求状态（任务/job_status 类）都成攻击面 | handle 服务端绑定已验证身份（`<user>:<handle>` 键控）；MUST NOT 以 handle 持有替代鉴权 |
| T-R6 | 数据边界扩张：远程 MCP = 记忆/资产数据离开本机信任边界（本地优先原则的正面冲突；M9 α 主线核心资产为记忆层） | 骨架不承载真实数据（握手面 + 固定响应） | 一旦工具面透传即成真实数据出口 | 授权先行（A-3）+ scope 最小化（spec Scope Minimization 节）+ 配额复用 hub quota（M3-WP03 面） |
| T-R7 | DoS/滥用（公开端点无速率限制） | hub quota 面既有（crates/partisync-hub/src/quota.rs） | 骨架未接 quota；demo 面仅 127.0.0.1 | 正式接线接 quota + 反代层限流 |

**结论**：T-R1/T-R6 是「不做授权就不远程化真实数据」的根据；其余各项
均为正式接线的登记题，不阻塞骨架。

## 4. rmcp 3.4.0 能力面（docs.rs 实证 2026-10-04）

- `transport::streamable_http_server` 侧存在 `StreamableHttpService`
  （feature `transport-streamable-http-server`）；`auth` feature 提供
  OAuth 2.0 支持；`auth-enterprise-managed`（EMA/XAA）存在。
- **未逐项核验**：StreamableHttpService 对 G-3 头校验/G-4 Origin/A-3
  受众绑定的覆盖程度（docs.rs 页面不展开实现细节）——正式接线前必须
  以本地 `cargo doc`/源码复核（沿「禁凭记忆写第三方 API」红线的双向
  执行），登记 §5-O1。
- 依赖面影响：启用上述 feature 是否引入新顶层 crate（如 axum 已在
  workspace pin，但 rmcp HTTP 栈的传递依赖需 `cargo tree` 实证）——
  铁律 8 流程（ADR + cargo deny）随正式接线任务走，本 WP 零依赖变更。

## 5. 结论

1. **协议版本无债**：rmcp 3.4.0 已锁 2026-07-28，工具/语义面达标；
   G8 的实质缺口是**传输（G-1）+ 授权（A-1..A-3）+ 部署边界**三面，
   不是协议版本。
2. **2026-07-28 的 stateless 化对 Hub 有利**：GET 流与协议级 session
   移除（G-2）意味着远程化不需要会话亲和/粘性路由，无状态 handler
   直调模型可原样平移——评估期内最重要的正面发现。
3. **一期骨架不承诺生产可用**（SPEC 明示）：骨架 = PRM 发现（A-2 形状）
   + 401 挑战 + initialize/握手级响应 + 404/-32601 与 202 语义，
   **不接真实 OAuth、不校验 token、不承载工具执行、不承载真实数据、
   无 TLS**。mock 边界沿 M5-WP04 判例（SPEC M5-WP04 §v0.1 外部依赖
   策略：stub/adapter 落既有依赖，正式接线另立）。
4. **正式远程化 = 独立立项**（条件触发：真实多端远程需求验证后）：
   需 ADR（AS 选型/自建 vs 外接、rmcp feature 启用、TLS 终结、
   scope 模型）+ 威胁模型 §3 七项逐条处置。G8 债在「评估完成 + 骨架
   存在 + 剩余面登记」意义上于本 WP 清账，剩余面转 §6 登记。

## 6. 开放问题登记（不阻塞本 WP）

| # | 问题 | 归属 |
|---|---|---|
| O1 | StreamableHttpService/auth feature 的合规覆盖度（G-3/G-4/A-3） | 正式接线任务前置复核（cargo doc/源码） |
| O2 | AS 选型（自建 vs 外接 IdP）与 Client ID Metadata Documents 支持面 | 正式接线 ADR |
| O3 | 远程记忆工具的 scope 切分（mcp:memory-read vs memory-write 步进授权） | 正式接线 ADR（spec Scope Minimization） |
| O4 | spec 漂移：2026-07-28 之后的新 revision（如 issuer SHOULD→MUST 升级预告）触发重评 | 修订触发条件（SPEC §风险） |
