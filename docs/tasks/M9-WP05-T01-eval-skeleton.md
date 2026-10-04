# Task: M9-WP05-T01 WP05 SPEC 起草 + MCP 2.0 远程化评估文档 + Hub 端点骨架

> **范围外**：真实 OAuth/token 验证/TLS/工具透传（SPEC §5 非目标，正式
> 远程化另立项）；M9-WP00.md 状态回填（随 T02 收尾）；不操控 GUI（本 WP
> 无桌面 UI 面）；不发布、不代填 G3 三签。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP05-T01 |
| **类型** | 起草 + 骨架实装（文档 PR + 代码 PR 堆叠；PR1 = R0 纯文档，PR2 = R1 hub 侧新模块零数据面） |
| **优先级** | P1（M9-WP00 §1-WP05/§4 债表 G8 行「MCP 仅本地 stdio」的兑现任务；窗口敏感项） |
| **范围** | SPEC M9-WP05 全文 + 评估文档（差距面 G-1..G-6/A-1..A-5 + 威胁模型 T-R1..T-R7 + 结论）+ 本卡（PR1）；partisync-hub mcp_remote 骨架 + 探针 + hub-demo 接线（PR2）。评估与骨架拆两个堆叠 PR，均挂本 Task-ID（沿 M9-WP04-T02 判例 #143–#145） |
| **创建日期** | 2026-10-04 |
| **来源** | M9-WP00 §1-WP05/§4 债表 G8 + M9-roadmap-proposal §3-δ/§5-WP05/§6-G8 + M8 前瞻 G8 + M7-WP03-enterprise-topics §2（威胁模型判例）+ M5-WP04 v0.1 mock 边界判例 |

## 契约落点

1. **评估文档**（docs/reviews/M9-WP05-mcp2-remote-eval.md）：对齐
   2026-07-28 spec 三要素——传输面差距（Streamable HTTP 单端点 POST /
   2026-07-28 移除 GET 流与协议级 session → stateless 同构性是核心正面
   发现）/ 授权面差距（OAuth 2.1 RS + RFC 9728 PRM MUST + token 受众
   绑定 MUST + token passthrough 禁止）/ 威胁模型 T-R1..T-R7（只登记
   不设计）。全部引文 2026-10-04 在线实证（modelcontextprotocol.io +
   docs.rs rmcp 3.4.0），现状锚点 path:line。
2. **结论（SPEC §2.3 明示）**：一期骨架不承诺生产可用——不接真实
   OAuth（mock Bearer 存在即放行，边界探针钉死为明示行为）、无 TLS、
   无工具执行、不承载真实数据；正式远程化（rmcp
   transport-streamable-http-server/auth feature + AS 选型 + TLS）=
   独立立项（评估文档 §6-O1..O4 登记）。
3. **骨架契约**（SPEC §2.2）：hub 新模块 `mcp_remote`——
   `GET /.well-known/oauth-protected-resource`（RFC 9728 形状 PRM mock
   文档）+ `POST /mcp`（无 Bearer → 401 + `WWW-Authenticate: Bearer
   resource_metadata=…`；Bearer + initialize → 单 JSON `protocolVersion:
   "2026-07-28"` + skeleton 标注 serverInfo；notification → 202；
   未知方法 → 404 + `-32601`）。纯函数核心 + axum handler 薄包装
   （零新增 dev-dependency）。hub-demo merge 接线（bind 仍
   127.0.0.1:8090）。
4. **依赖面**：零新增顶层依赖（axum/tokio/serde/serde_json 为 hub
   Cargo.toml 既有 pin；`git diff Cargo.toml` 为空实证，铁律 8）。

## 边界与既有决定

- SPEC 状态「草稿（合入 = 批准）」沿 M9-WP00 判例（M8-WP00 PR #59）。
- spec 规范性事实起草期实证（2026-10-04）：2026-07-28 Streamable HTTP
  移除 GET 流端点与协议级 session（changelog 注记原文）；MCP servers
  MUST implement RFC 9728 PRM 且 PRM MUST 含 authorization_servers；
  401 挑战形状 `WWW-Authenticate: Bearer resource_metadata="…/.well-
  known/oauth-protected-resource"`；未实现方法 404 + `-32601`；
  notification 202；Origin 校验 MUST。rmcp 3.4.0 具备
  StreamableHttpService（feature transport-streamable-http-server）与
  auth feature——覆盖度未逐项核验，登记评估文档 §4/O1（禁凭记忆写
  第三方 API 红线的双向执行）。
- gateway stdio 面零改动（mcp.rs 不在文件清单）；远程面挂 Hub 侧
  （M9-WP00 §1-WP05「Hub 端点骨架」原文）。
- mock 边界沿 M5-WP04 v0.1 判例（SPEC M5-WP04 §v0.1 外部依赖策略：
  stub 落既有依赖、正式 SDK/接线另立）。

## 验收

- [ ] docs/specs/M9-WP05.md 落盘：§2 契约（评估三要素 + 骨架端点面 +
      mock 边界明示）+ §3 任务切分 + §4 可执行验收 + §5 非目标 + §6
      文件清单 + §7 风险六项；
- [ ] docs/reviews/M9-WP05-mcp2-remote-eval.md 落盘：差距面（G-1..G-6
      / A-1..A-5）+ 威胁模型（T-R1..T-R7 表）+ 结论 + 开放问题
      O1..O4；
- [ ] 本卡落盘（先卡后工）；
- [ ] PR2 骨架探针全绿：PRM 四字段 / 401 挑战 / initialize 握手 /
      202 / 404+-32601 / mock Bearer 放行边界；
- [ ] 零新增顶层依赖 + fmt/clippy/test 门禁全绿；
- [ ] 产品依赖图变更仅限 partisync-hub 新模块（hub-demo 演示面接线），
      gateway/桌面/CLI 零触碰。
