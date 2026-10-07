# M10-WP05-T02 评估：MCP 2.0 远程化接线评估（O1/O2/O3 必答·六问）

> 性质：**接线评估件（纸面，先评估后落锤）**——SPEC M10-WP05 §2.1 六问必答，
> 零代码零依赖变更；结论由同分支 **ADR-0031**（docs/adr/0031-mcp2-remote-wiring.md）
> 落锤。证据纪律沿 M10-WP04-T05 判例：registry 源码行级 + spec 在线实证 +
> cargo tree 运行输出，禁凭记忆。rmcp 源码 = `~/.cargo/registry/src/
> index.crates.io-1949cf8c6b5b557f/rmcp-3.4.0/`（2026-10-07 实证；workspace
> pin `rmcp = "=3.4.0"`，Cargo.toml:64）；spec 引文 = modelcontextprotocol.io
> 2026-07-28 版（2026-10-07 fetch 实证）；仓库行号 = 本工作树（main@16377df）。
> 上游：M9-WP05-mcp2-remote-eval.md（G-1..G-6 / A-1..A-5 / T-R1..T-R7 /
> O1..O4，下称「M9 评估」）/ M10-WP05 §2.0-§2.1 / M10-WP00 §2 拍板#3。

## 0. 输入与基线

- **债锚点**：M10-roadmap §6-NB3（真实 OAuth 2.1 授权 + TLS + 工具透传）；
  M9-report §5 G8 行 ◐ 半清偿。范围口径（SPEC §1）：评估与 ADR 落锤照做，
  可本地测试的实施，真外部依赖如实登记债务。
- **宿主裁定（SPEC §2.0，本评估复核维持）**：真实面落 `partisync-gateway`
  新模块——11 工具直调面在 mcp.rs:609-619，`impl ServerHandler for
  McpServerState` 在 mcp.rs:548，stdio 唯一入口 `run_mcp_server`
  （mcp.rs:1704 → serve_server(state, stdio()) mcp.rs:1755）。hub 无 graph
  数据面，维持骨架不动：hub mcp_remote.rs:3-13 mock 边界 doc + :119
  「Bearer 存在即放行」探针钉死，与 gateway 真实面不同 bin 不同端口并存
  （R5 双面关系按 SPEC §6 处置）。
- **M9 评估最重要的正面发现复核成立**：2026-07-28 stateless 化（changelog
  major #1/#2，SEP-2567/2575）与既有 handler 直调模型同构。rmcp 3.4.0
  tower.rs:84-90 明文「Per SEP-2567, sessions are removed from the
  `2026-07-28` version, so requests negotiating that version are always
  served statelessly regardless of this setting」。
- **rmcp feature 现状**：gateway 默认 `["server", "transport-io"]`
  （crates/partisync-gateway/Cargo.toml:34）；Windows target 已例外启用
  `server-side-http`（同文件 :47-49，桌面 sidecar 先例）。workspace 无
  rustls/jsonwebtoken/oauth2 顶层 pin（Cargo.toml:28-85 实证，与 SPEC §2.0
  起草期断言一致）。

## 1. 问题一（O1）：rmcp 3.4.0 覆盖度复核

M9 评估 §4「未逐项核验」项逐条以 registry 源码复核。覆盖件 =
`transport-streamable-http-server` feature 的 tower.rs（`StreamableHttpService`
+ `NegotiatingStatelessHttpService`，:331 起 `impl Service`）。

### 1.1 G-3 协议头校验——**SDK 全覆盖，前提 = 显式开启 opt-in 开关**

| 校验面 | rmcp 3.4.0 实现（tower.rs） | spec 依据（2026-07-28 streamable-http 页，在线实证） |
|---|---|---|
| 头-体一致性（`MCP-Protocol-Version` vs body `_meta.io.modelcontextprotocol/protocolVersion`） | `validate_request_protocol_version_meta` :494-570——不一致或缺头 → `header_mismatch_jsonrpc_response` :711-721（HTTP 400 + JSON-RPC `-32020`，错误码映射 :648 `HEADER_MISMATCH → 400`） | 「The header value **MUST** match the `io.modelcontextprotocol/protocolVersion` field … reject … with `400 Bad Request` and a `HeaderMismatch` JSON-RPC error」 |
| 缺头拒绝（fail-closed） | `validate_required_protocol_header` :573-602——opt-in（`stateless_protocol_metadata_required`，**默认 false** :200）开启后缺头必拒 400/-32020 | 「Every POST request … **MUST** include an `MCP-Protocol-Version` header」；不支持旧版客户端的 server **MUST** 拒绝无头请求 |
| 缺 `_meta.protocolVersion` 拒绝 | `validate_required_protocol_meta` :605-634——opt-in 后缺 `_meta` → 400/-32602 invalid_params | changelog minor #12 + major #2（每请求 `_meta` 携带协议版本） |
| initialize 头-体一致 | `validate_header_matches_init_body` :460-492（无头放行 = 握手首轮豁免，:459 注释） | 同上（协议协商面） |
| `Mcp-Method` / `Mcp-Name` / `Mcp-Param-*`（SEP-2243 标准头） | `validate_standard_headers` :724-766——版本 ≥ `STANDARD_HEADERS`（= `V_2026_07_28`，model.rs:178）即校验头-体一致；tools/call 按工具 schema 校验 `Mcp-Param-*`（:747-755） | 「These headers are **REQUIRED** for compliance」；「Any server that processes the message body **MUST** validate … `400` + `-32020`」 |

**结论**：G-3 五个子面 SDK 全有；T03 必须显式 `with_stateless_protocol_metadata_required(true)`
（默认关 = 缺头放行，与 fail-closed 相悖）——这是 O1 复核新发现的关键配置位。

### 1.2 G-4 Origin/host 校验——**SDK 全覆盖，默认态松，须配置收紧**

- `allowed_hosts` 默认 `["localhost", "127.0.0.1", "::1"]`（:195-199）——Host
  校验默认即开（DNS rebinding 主防线）；文档 :112-124 明示公网部署须覆写。
- `allowed_origins` **默认空 = Origin 校验关闭**（:126-150「Defaults to an
  empty list, which disables Origin validation for backward compatibility」）；
  `enforce_origin_validation` :225-230 可开空名单全拒。校验语义 = RFC 6454
  (scheme, host, port) 等值（:858-880 `origin_is_allowed`），存在且非法 → 403
  （`forbidden_response` :768-774）；缺失 Origin 放行（与 spec「If the `Origin`
  header is present and invalid, servers **MUST** respond with HTTP 403」一致
  ——spec 只约束「存在且非法」）。
- 调用点在 `Service::call` 最前（:1540 `validate_dns_rebinding_headers`），
  **先于一切授权/业务面**——SPEC §2.2「先于授权面」可由 SDK 判定。

**结论**：T03 须显式配置 Origin 白名单（或 enforce 空名单）；「非白名单
Origin → 拒绝」探针可直接钉 SDK 行为。

### 1.3 G-5 Accept 协商 + 单 JSON 模式——**SDK 全覆盖**

- POST：Accept 必含 `application/json` + `text/event-stream` 双 mime，否则
  406（:1724-1734）；Content-Type 必须 `application/json`，否则 415（:1736-1747）。
- 单 JSON：`json_response(true)`（:91-97/:251-254）——stateless POST 优先
  `Content-Type: application/json` 返回终态消息（:2090-2110）；handler 发出
  中间 notification/request 时回落 SSE 保序（:2112 起，注释「so no message is
  lost」）。`serve_negotiated_request_directly` 终态单消息同样走单 JSON
  （:1294-1306）。
- **结论**：`json_response: true` + stateless = G-5「逐请求择一、单 JSON
  达标」；SSE 回落仅作保序手段，SPEC §4 非目标「服务器侧流式」不触发。

### 1.4 G-6 状态码语义——**SDK 全覆盖**

- 未知 RPC 方法 → handler 默认 `ErrorCode::METHOD_NOT_FOUND`（handler/server.rs:26
  文档、:561 `on_custom_request` 缺省返回 -32601）→ `jsonrpc_http_status`
  :649 映射 **HTTP 404**（带 -32601 体）。
- notification POST → **202 Accepted 无体**（`accepted_response()`
  common/server_side_http.rs:24-29，stateless 分支 tower.rs:2139-2143）。
- GET/DELETE（2026-07-28 客户端不该发）→ 405 + `Allow: POST`（:441-448，
  分派 :1557-1570）。
- **结论**：M9 评估 G-6「骨架直接实现」转为「SDK 已实现，探针直钉」。

### 1.5 auth 线位（RMCP_AUTH）——**RS 侧零 SDK 现成件，是 ADR 必答题**

- `auth` 模块为**客户端侧** OAuth：auth.rs:8-12 直接 import `oauth2` 的
  `PkceCodeChallenge`/`AuthUrl`/`TokenUrl` 等客户端流类型；:511 `AuthClient`
  （:527 new/:536 get_access_token）+ `CredentialStore` :301 +
  `StateStore` :455 + `AuthorizationManager` :1115——全部为「客户端取
  token」语义。
- `WWWAuthenticateParams`（:704-730，parse/is_insufficient_scope/is_invalid_token）
  是**客户端解析 401 挑战**的工具，不是 RS 侧产出挑战的面。
- `JwtSigningAlgorithm`（:940-984，doc「JWT signing algorithm for
  private_key_jwt authentication (SEP-1046)」，feature
  `auth-client-credentials-jwt`）= 客户端**签** JWT 断言向 AS 认证，非 RS
  **验** access token。
- 全模块无 JWKS 拉取验签、无 aud 校验、无 401/403 挑战产出线——**RS 侧
  token 校验须自建**（校验件选型 → ADR-0031 决策 3）。
- 依赖代价：`auth` feature 拉 `oauth2 5.0` + `reqwest` + `url`（rmcp
  Cargo.toml [features] 实证）；RS 场景不发起任何 OAuth 流程，启用即无谓
  扩面。**不启用**。

### 1.6 cargo tree 传递依赖实证（2026-10-07 运行）

实验：在 gateway Cargo.toml 临时启用 `transport-streamable-http-server` 后
`cargo tree -p partisync-gateway -e normal`（实验后已回退，工作树零残留）：

- rmcp v3.4.0 子树新增依赖 = `async-trait 0.1.92` / `base64 0.23.1` /
  `bytes 1.12.1` / `http 1.5.0` / `http-body 1.1.0` / `http-body-util 0.1.5` /
  `rand 0.10.3` / `sse-stream 0.2.6` / `tokio-stream 0.1.19` /
  `tower-service 0.3.3` / `uuid 1.26.1`——**全部已在 Cargo.lock**（经
  openald/reqwest/iroh/tauri 等既有线位入图）；Cargo.lock 唯一 delta =
  gateway 的 rmcp feature 列表加一行 `"async-trait"`。**T03 传输 feature
  翻转 = 零新 crate**。
- `auth` 线位若启用将新增 `oauth2 5.0`（lock 无）——本 ADR 不启用（§1.5）。
- `jsonwebtoken` 11 仅随 `auth-client-credentials-jwt`（lock 无）——与 rmcp
  的使用无关；校验件直引是否新增顶层依赖 → ADR-0031 决策 3。
- TLS 线位现状：`rustls 0.23.45` / `aws-lc-rs` / `tokio-rustls` /
  `hyper-rustls 0.27.10` / `rcgen 0.14.10` 均已在 lock（`cargo tree -i
  rustls@0.23.45` → reqwest ← iroh 线；rcgen ← irpc ← iroh-blobs 线）——
  服务端 TLS 与测试证书若选同线，新增 crate 近零（→ 问题四）。

### 1.7 O1 复核结论表

| 项 | M9 评估口径（2026-10-04 docs.rs） | 本复核结论（2026-10-07 源码） | T03 配置位 |
|---|---|---|---|
| G-1 单端点 POST | StreamableHttpService 存在 | ✅ :331/:1048/:1075 tower Service，挂 axum 既有 pin | axum Router 组装 |
| G-2 stateless | 未核验 | ✅ :84-90 SEP-2567 明文；`NeverSessionManager`（session/never.rs:19）配套 | legacy_session_mode=false（默认） |
| G-3 协议头 | 未核验 | ✅ 五子面全覆盖（§1.1） | **opt-in 开关必须显式开** |
| G-4 Origin/host | 未核验 | ✅ 全覆盖，默认松（§1.2） | **allowed_origins 须显式配** |
| G-5 Accept/单 JSON | 未核验 | ✅ 全覆盖（§1.3） | json_response=true |
| G-6 状态码 | 骨架自实现 | ✅ SDK 已实现（§1.4） | 探针直钉 |
| A-3 RS token 校验 | 零现成件（预告） | ✅ 复核坐实零现成件（§1.5） | 自建校验件（ADR 决策 3） |

## 2. 问题二（O2）：AS 选型——三方案对比

约束（SPEC §2.1 + M10-WP00 §2 拍板#3「先出接线 ADR 再落锤」）：①单人团队；
②本地优先；③mock AS 本地 e2e 验收须无外网依赖（SPEC §2.5/§2.4）；④铁律 8
（新依赖面走本 ADR）；⑤真实外接 = NB-WP05-1 债条件触发（SPEC §4）；
⑥CIMD（A-4）= AS 侧机制。

| 维度 | 方案 A：外接标准 OAuth 2.1 IdP（Keycloak/Auth0/ORY 等） | 方案 B：自建最小 AS（授权码+PKCE+consent 全套） | 方案 C：仅 RS + dev mock AS（生产 AS = 配置入参） |
|---|---|---|---|
| 安正面 | AS 安全由成熟实现承担（最强）；RS 只做验签 | **最弱**——AS 是 spec 明示超出范围的敏感件（authorization 页「The implementation details of the authorization server are beyond the scope」），自建 = 攻击面自造 | RS 校验面完整 fail-closed（aud/iss/exp/scope）；AS 不在本仓生产边界；token 校验矩阵探针可本地穷举 |
| 依赖面 | 零新增（RS 验签件仍需 jsonwebtoken）；但引入外部服务账户 | 新依赖 + 大实现面（consent/CIMD 抓取/DCR 全套） | dev 依赖 jsonwebtoken（签发+验签同件）；产品侧零 AS 依赖 |
| 运维面 | 外部服务可达性/账户/密钥管理，单人团队重 | 全套自担，单人团队不可承受 | 零外部依赖；公网部署时换配置指向外接 AS（NB-WP05-1） |
| 本地 e2e | ✗ 不可自足（需外网 + 外部账户，与 SPEC §2.5「无外网依赖」验收冲突） | △ 可本地跑但实现成本与安全风险双高 | ✓ 仓内线程内 AS（axum 既有依赖）签标准 JWT，全链本地 |
| CIMD（A-4）支持面 | 外接 IdP 原生支持与否视选型（登记兼容面） | 须自实现（含外网抓取 client metadata 的 SSRF 面——spec security_best_practices CIMD SSRF 节） | 不在仓交付（AS 侧机制）；mock AS 走「预注册 client_id」形态（spec 三注册机制之一） |
| spec 对齐 | 完全对齐（AS 侧 spec 合规由 IdP 兜） | 对齐成本极高 | RS 侧职责全对齐；AS 侧职责让渡给外接/条件触发 |

**对比结论**：B 出局（安全 + 运维双不可承受）；A 与「本地 e2e 无外网依赖」
验收冲突且即 NB-WP05-1 债本体——**作为生产形态登记，不作为本期交付**。
拍板 = **方案 C**：RS 校验面按标准 JWT（签名/iss/aud/exp/scope，RFC 8707
受众绑定）实现，生产部署的 `authorization_servers` 指向真实 AS 仅为配置
入参——未来外接 IdP 时 RS 侧零重构（方案 A 的 RS 面与方案 C 完全同构）。

**CIMD 登记面（A-4 转正）**：CIMD 是 AS 对 URL 形 client_id 的服务端机制
（draft-ietf-oauth-client-id-metadata-document-00）；本仓不做 AS → CIMD
服务端支持不落仓。RS 侧对 client 注册形态**无感**（RS 只验 token 声明，
不关心 client_id 形态）——外接 IdP 支持 CIMD 与否不影响本仓 RS 面。

## 3. 问题三（O3）：scope 切分拍板建议（11 工具全量映射）

spec 依据（2026-07-28 在线实证）：Scope Minimization（最小初始集 + 精确
挑战 + 禁全量目录发放 + 禁通配/omnibus scope）；Scope Selection Strategy
（401 挑战带 `scope` 参数优先）；Runtime Insufficient Scope（403 +
`error="insufficient_scope"` + 单挑战含全部所需 scope）；Step-Up Flow
（客户端侧并集重授权）；「Servers **MUST** account for scope hierarchies」
——**采用扁平无层级集**，规避层级归并歧义。

**拍板建议：扁平三域 `mcp:read` / `mcp:write` / `mcp:export`**（数据面按
读写与出仓切分；`scopes_supported` 缺省 = 本表三值）：

| # | 工具 | 域 | 理由 |
|---|---|---|---|
| 1 | asset_search | mcp:read | 检索无副作用 |
| 2 | asset_read | mcp:read | 读 |
| 3 | asset_organize | mcp:write | asset_txn 事务写 |
| 4 | dataset_export | mcp:export | 数据出仓面（T-R6 数据边界扩张单列对账） |
| 5 | job_status | mcp:read | 状态查询 |
| 6 | memory_write | mcp:write | 写 |
| 7 | memory_search | mcp:read | 读 |
| 8 | memory_verify | mcp:read | 只读校验 |
| 9 | memory_update | mcp:write | 墓碑换身份写 |
| 10 | memory_delete | mcp:write | 软删写 |
| 11 | ext_list | mcp:read | 列举 |
| 附 | ext_*（call_extension 动态工具面） | mcp:write | 动态面无静态读写证据，保守归高域（fail-closed 同构） |

- 协议元面（initialize/tools/list 等非 tools/call 方法）：合法 token 即可，
  不映射 scope（无数据出口；tools/list 仅暴露 schema 元数据）。scope 校验
  只挂 `tools/call`。
- 最小初始集 = `mcp:read`；`mcp:write`/`mcp:export` 走 step-up（403 挑战
  精确回所需 scope，单挑战含全量）。
- 401/403 挑战均带 `resource_metadata` 指向 PRM（A-2 形状），403 另带
  `scope="<所需>"`（spec 例示形状，authorization 页在线实证）。

## 4. 问题四：TLS 机制——自托管 rustls 线位 vs 反代终结

| 维度 | rustls 线位（服务端内建 TLS） | 反代终结（caddy/nginx 终 TLS，后端纯 HTTP） |
|---|---|---|
| 依赖面 | rustls 0.23 线已在 lock（经 iroh/reqwest，§1.6）；服务端 acceptor 需 `tokio-rustls`（已在 lock）+ gateway 直接引用——**启面走本 ADR，新顶层 pin 近零增量** | 零新增依赖（进程纯 HTTP） |
| 部署面 | 单二进制自足；证书轮换/续期自担 | 证书管理成熟（caddy 自动续期）；多一跳组件 |
| 本地测试 | ✓ 进程内 TLS roundtrip 可测（自签证书） | ✗ 本地 e2e 需起反代，测试自足性破坏 |
| SPEC 验收对齐 | 「自签证书经 TLS 线位 roundtrip 到 PRM」探针**必须**有线位才能钉（§3.1 T03 行） | 无线位则该验收项不可判定 |
| 启动守卫语义 | 非 127.0.0.1 bind 且无 TLS 配置 → 拒绝启动（显式错误）可统一在进程内钉 | 守卫仍在进程内（bind 面），TLS 由外层承担，语义分叉 |

**拍板建议：rustls 线位内建**（可测性 + 单二进制自足 + 启动守卫统一）；
公网生产部署文档建议反代终结为可选拓扑（线位与反代不互斥）。本地测试
证书：`rcgen` dev-dependency（0.14.10 已在 lock，零新 crate）生成自签
fixture——优于 openssl CLI 外部生成（SPEC §6-R6 预留的两分支，取仓内可
复现分支）。deny 实证回填随 T03 载体 PR。

## 5. 问题五：部署面（默认绑定 / 非回环前置 / 速率限制归属）

- **默认绑定**：新入口 bin（SPEC §2.2 建议 `partisync-mcp-http`）默认
  `127.0.0.1` + 固定缺省端口（实施期定，文档随 bin）。依据：spec「When
  running locally, servers **SHOULD** bind only to localhost (127.0.0.1)」
  （streamable-http 页在线实证）+ T-R3 DNS rebinding 防线。
- **非回环前置条件**（启动守卫，缺一拒绝启动 + 显式错误，不静默降级）：
  ①bind 非 127.0.0.1 → 必须配置 TLS（问题四）；②`allowed_hosts` 覆写为
  真实主机名（rmcp 默认回环名单，§1.2）；③授权配置齐备（T04 起；
  T03 期由 fail-closed 墙兜底——除 PRM 外全 401）。
- **速率限制归属（T-R7 处置，只定归属不设计）**：
  - 一期（默认回环本地）：不接——本机子进程/本机客户端面，T-R7 不成真
    威胁（M9 评估 T-R7「demo 面仅 127.0.0.1」口径延续）；
  - 公网暴露前：**主归属 = 反代层限流**（nginx/caddy rate limit），应用
    层复用评估 = hub quota 面（crates/partisync-hub/src/quota.rs:20
    QuotaState）——quota 语义是存储配额非请求速率，直接复用是语义错配，
    登记为评估结论；应用层限流实施 = 条件触发债（SPEC §4「公网部署运营
    面」行已登记，本评估确认该归属不动）。

## 6. 问题六（O4）：spec 漂移复核 + ADR-0031 决策条目汇总

### 6.1 漂移复核（2026-10-07 在线实证，禁凭记忆）

- **2026-07-28 仍为最新已发布 revision**：llms.txt 索引版本目录 =
  2024-11-05 / 2025-03-26 / 2025-06-18 / 2025-11-25 / 2026-07-28 / draft；
  无更新版本。
- **draft 树存在但零在册变更**：`/specification/draft/changelog` 页正文仅
  「Changes since the most recent release will accumulate here.」——无
  可评估漂移内容。
- **iss SHOULD→MUST 升级预告为官方明文**（authorization 页原文「A future
  revision of this specification is expected to upgrade authorization
  server inclusion of `iss` from **SHOULD** to **MUST**」）——M9 评估 O4
  的预判兑现为版本内明文；RS 侧 iss 校验照常 fail-closed，不受该升级影响
  （升级约束的是 AS 发 iss，RS 校验 iss 本就 MUST）。
- **M9 评估 G-1..G-6 / A-1..A-5 引文逐条复核一致**（transports/authorization/
  security_best_practices 三页 fetch 实证）；补充细证：changelog minor #4 =
  SEP-2243 标准头（M9 G-3 已登记）；minor #12 = 错误码区间政（-32020..-32099
  预留 spec，HeaderMismatch 定号 -32020）。
- **触发条件维持**（SPEC §6-R7）：新 revision 发布 → 重评不追溯。rmcp
  `=3.4.0` 精确锁定（ADR-0019）不受漂移影响。

### 6.2 ADR-0031 决策条目清单（五项决策各有着落）

| # | 决策项 | 落锤（详见 ADR-0031） |
|---|---|---|
| 1 | AS 选型 | 方案 C：仅 RS + dev mock AS；生产 AS = 配置入参（外接 = NB-WP05-1 条件触发）；不启用 rmcp auth 线位；CIMD 不落仓（AS 侧机制，RS 无感） |
| 2 | 传输件 | rmcp `transport-streamable-http-server` StreamableHttpService（tower Service）挂 axum 0.8 既有 pin；`NeverSessionManager` + `json_response(true)` + `stateless_protocol_metadata_required(true)` + Origin/host 白名单显式配置 |
| 3 | token 校验件 | 新顶层依赖 `jsonwebtoken 11`（RS 侧验签：JWK/JwkSet + Validation iss/aud/exp；JWKS 拉取用 reqwest 既有 pin 手写）；rmcp auth 线位否决（客户端侧，§1.5）；feature 集/凭证面 deny 实证随 T04 载体回填 |
| 4 | TLS 机制 | rustls 0.23 线位内建（tokio-rustls 已在 lock）；测试证书 rcgen dev 依赖（已在 lock，零新 crate）；非回环 bind 无 TLS → 启动拒绝；公网反代终结 = 可选拓扑文档建议 |
| 5 | scope 模型 | 扁平三域 mcp:read/mcp:write/mcp:export + §3 全量映射表；元面不查 scope；step-up 单挑战含全量 |

## 7. 诚实登记（未做/未验证）

- **未实测运行**：本评估为纸面 + 源码/依赖实证，未启动 rmcp HTTP 服务做
  roundtrip（实施 = T03 探针判定；「逐项覆盖度仍须 O1 实施期复核」由本件
  源码行级复核替代，运行期行为以 T03/T04/T05 探针矩阵为准）。
- **jsonwebtoken API 面**：docs.rs（2026-10-07 fetch）确认 `jwk` 模块
  （JwkSet）与 `Validation` 结构在册、aws-lc-rs 为 crypto provider 依赖；
  具体 feature 名/构造式 API 禁凭记忆，T04 实施期以本地 `cargo doc`/
  源码钉定（ADR-0031 决策 3 已挂查证义务）。
- **cargo tree 实验为临时改 Cargo.toml 后回退**（工作树零残留，`git status`
  实证）；正式 feature 翻转随 T03 载体 PR 走门禁。
- **mock AS 与真实 IdP 行为差异**（R3）：mock 只对齐 ADR-0031 拍板校验面
  （签名/iss/aud/exp/scope），不承诺 CIMD/DCR 全流程互操作——NB-WP05-1
  债边界不变。
- hub `mcp_remote.rs` 引用行号以 main@16377df 工作树为准；后续任务若改动
  该文件（本 WP 契约 hub 不动）须复核行号。
