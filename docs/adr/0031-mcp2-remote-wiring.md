# ADR-0031: MCP 2.0 远程化接线选型——仅 RS + rmcp Streamable HTTP + jsonwebtoken 验签 + rustls 线位 + 扁平三域 scope

版本: 0.1 · 状态: **草稿**（随 SPEC M10-WP05 批准生效——合入 = 批准，
沿 M9-WP00/M9-WP04 判例；批准人 @lead）·
关联: SPEC M10-WP05（§2.0 机制不锁死声明 → 本 ADR 落锤）·
docs/reviews/M10-WP05-mcp2-wiring-eval.md（六问评估件，下称「评估件」，
2026-10-07 同分支）· ADR-0019（rmcp `=3.4.0` 精确锁定，接受——本 ADR
只增 feature 不动 pin）· M9-WP05-mcp2-remote-eval.md（O1..O4 上游）·
M10-WP00 §2 拍板#3（「先出接线 ADR 再落锤」）
负责人: @partiverse · 批准人: @lead · 起草日期: 2026-10-07

## 背景

M10-WP05 将 M9 骨架（hub mock 面，Bearer 存在即放行）推进为 gateway 真实
远程面。SPEC §2.0 宿主裁定 gateway 侧 + 机制不锁死，五个机制题（AS 选型/
传输件/token 校验件/TLS 机制/scope 模型）交本 ADR。评估件六问已逐题给出
对比与证据（rmcp 3.4.0 registry 源码行级 + cargo tree 运行输出 + spec
2026-07-28 在线实证，全部 2026-10-07），本 ADR 按其结论落锤。

硬约束：①铁律 8——新顶层依赖由本 ADR 承载；②评审深度 R2（鉴权/授权面，
SPEC §2.5）；③本地 e2e 无外网依赖（SPEC §2.5）；④fail-closed（墙先于门，
SPEC §2.2）；⑤deny 白名单不改（红线，deny 拦截即回本 ADR 修订）。

## 决策

1. **AS 选型 = 仅 RS + dev mock AS（评估件 §2 方案 C）**。本仓不建生产
   AS：授权面只实现 resource server 侧校验（签名/iss/aud（RFC 8707 受众
   绑定）/exp/scope，fail-closed）；`authorization_servers`/`resource`/
   `scopes_supported`/`bearer_methods_supported` 全部为 PRM 配置入参。
   mock AS = 仓内线程内 axum（既有 pin）+ dev 依赖签发标准 JWT，仅供
   T05 e2e，不入产品依赖图。生产外接标准 IdP = NB-WP05-1 债条件触发
   （届时仅换配置，RS 面零重构——方案 A 的 RS 面与本决策同构）。CIMD
   （A-4）为 AS 侧机制，不落仓；RS 对 client 注册形态无感。**否决
   rmcp `auth` 线位**（评估件 §1.5：纯客户端侧 OAuth，启用无谓拉
   `oauth2 5.0` + reqwest 面且不提供任何 RS 校验件）。

2. **传输件 = rmcp `transport-streamable-http-server` 的
   `StreamableHttpService`，挂 axum 0.8 既有 pin**（评估件 §1.7 O1 复核
   全覆盖结论）。gateway 新模块 `mcp_remote.rs` + 新入口 bin
   `partisync-mcp-http.rs`（stdio 面/桌面 sidecar 零改动）。rmcp feature
   批次：`server + transport-io`（既有）+ `transport-streamable-http-server`
   （T03 载体 PR 启用；Windows 例外行 `server-side-http` 保持）。配置
   fail-closed 四件套：`NeverSessionManager`（session/never.rs:19，配合
   2026-07-28 stateless）+ `json_response(true)`（单 JSON 达标，SSE 仅
   保序回落）+ `stateless_protocol_metadata_required(true)`（缺头必拒
   400/-32020——**默认 false 是 O1 复核发现的关键坑位**，评估件 §1.1）+
   `allowed_hosts`/`allowed_origins` 显式白名单（Origin 校验默认关，评估
   件 §1.2）。端点形状沿 M9 骨架口径：`POST /mcp` + PRM
   `GET /.well-known/oauth-protected-resource`。**否决**：axum 骨架手写
   全量协议面（O1 已证 rmcp 五子面全覆盖，手写 = 重复造轮 + 漂移面）。

3. **token 校验件 = 新顶层依赖 `jsonwebtoken 11`**（评估件 §1.5：rmcp
   RS 侧零现成件，自建必答题的唯一无聊依赖解）。职责：RS 侧 JWT 验签
   （`jwk` 模块 JwkSet 解析 JWKS + `Validation` issuer/audience/exp 校验，
   docs.rs 2026-10-07 实证在册）；JWKS 拉取/缓存用 reqwest（workspace
   既有 pin）手写数十行，不引第三件。algorithm allowlist 显式钉定
   （不接受 token header 指定任一算法）。feature 集（aws_lc_rs vs
   rust_crypto 线——aws-lc-rs 已在 lock 经 rustls 0.23 默认 provider）与
   精确 pin 在 T04 载体 PR 以本地源码查证钉定后回填本 ADR 修订登记
   （禁凭记忆红线，评估件 §7）。**查证结论回填（2026-10-08，T04 载体
   PR）**：pin = `=11.1.0`（crates.io 11 线最新，2026-09-16 发布；本地
   .crate 源码解包查证）· feature = `default-features = false,
   features = ["aws_lc_rs"]`（aws_lc 后端全算法含 EdDSA/Ed25519，
   `Jwk::from_encoding_key` 经 `ed_pub_components_from_private_key` 实
   证可用；不启 `use_pem`——产品/测试路径均走 JWK/DER 零 PEM，少拉
   pem/simple_asn1 两件）· MSRV 1.88 ≤ 工具链钉子 1.94.0 · 依赖闭包
   全既有（aws-lc-rs 1.18.1 ≥ 源码要求 1.15.0）。dev 侧 mock AS 签发同用
   jsonwebtoken（主依赖对 integration tests 可见，零 dev-dependency 批次
   增量；测试密钥经 rcgen 既有 dev 依赖 Ed25519 生成）。**否决**：手写
   HMAC 自发 token（偏离标准 JWT/JWKS，外接 IdP 时全重写）；
   oauth2/jsonwebtoken 之外再加 jwks 客户端 crate（依赖面无谓扩张）。

4. **TLS 机制 = rustls 0.23 线位内建 + rcgen dev 证书（评估件 §4）**。
   服务端 TLS acceptor 用 tokio-rustls（0.23 线已在 lock 经 iroh/reqwest，
   与 reqwest 同线无双版本）；启动守卫：bind 非 127.0.0.1 且未配置 TLS →
   拒绝启动（显式错误，不静默降级明文）。测试证书 = `rcgen` dev-dependency
   （0.14.10 已在 lock，零新 crate）生成自签 fixture（SPEC §6-R6 两分支
   取仓内可复现分支）。公网生产拓扑建议反代终结（文档建议，与线位不
   互斥）。**否决**：纯反代终结（TLS 探针「自签证书经 TLS 线位 roundtrip」
   无线位不可判定 + 本地 e2e 需起外部组件）；native-tls（openssl 链，
   违无聊依赖与既有 rustls 同线原则）。

5. **scope 模型 = 扁平三域 `mcp:read` / `mcp:write` / `mcp:export`**
   （评估件 §3 全量映射表 11 工具 + ext_* 动态面保守归 `mcp:write`）。
   无层级（规避 spec hierarchy 归并歧义）；`scopes_supported` 缺省 =
   本三值；最小初始集 `mcp:read`，write/export 走 step-up（403 +
   `error="insufficient_scope"` + 单挑战含全部所需 scope）；scope 校验只
   挂 `tools/call`，协议元面（initialize/tools/list）随合法 token；
   401/403 均带 `resource_metadata` 指向 PRM（A-2 形状）。

## 后果

- 正面：NB3 三面（真实 OAuth 2.1 RS / TLS / 工具透传）机制面全部有着
  落且 fail-closed 可探针化；传输 feature 翻转零新 crate（cargo tree
  实证，评估件 §1.6）；校验件/TLS/证书三件与 lock 既有线同源，deny 面
  新增量集中于 jsonwebtoken 本体；外接 AS 未来 = 纯配置切换。
- 负面/风险：jsonwebtoken 为新顶层依赖（铁律 8 载体 = 本 ADR；**deny
  实证已回填（2026-10-08，T04 载体 PR）**：本地 `cargo deny check` 四项
  全绿——`advisories ok, bans ok, licenses ok, sources ok`；Cargo.lock
  净增量 = `jsonwebtoken 11.1.0` + `untrusted 0.7.1`（aws-lc-rs 1.18.1
  线拉入）两件，依赖闭包其余全既有（aws-lc-rs 1.18.1 ≥ 源码要求
  1.15.0）；`cargo audit` 10 条 vulnerability 全部为 deny.toml 既有
  allowlist 债（h2/rsa/rustls-webpki/wasmtime 线），jsonwebtoken/
  untrusted advisory 零命中）；`stateless_protocol_
  metadata_required`/Origin 白名单若漏配则 fail-closed 破功——SPEC §3.1
  已钉 T03 探针（缺头必拒/非白名单 Origin 必拒）双保险；mock AS 与真实
  IdP 行为差异（R3）= NB-WP05-1 债边界不变。
- 中性：hub 骨架不动（M9 交付物 + mock 边界探针契约维持，双面关系评估
  件 §0 登记）；rmcp pin 不动（只增 feature）；SSE 服务器侧流式维持
  非目标（json_response(true) 回落路径是保序手段非流式面）。

## 修订登记

| 版本 | 日期 | 修订内容 | 触发/依据 |
|---|---|---|---|
| 0.1 | 2026-10-07 | 初稿随 SPEC M10-WP05-T02：五项决策落锤 | M10-WP00 §2 拍板#3（先评估后落锤） |
| 0.2 | 2026-10-08 | 决策 3 查证结论回填：jsonwebtoken `=11.1.0` + `default-features=false, features=["aws_lc_rs"]`（不启 use_pem）；deny/audit 实证落后果节 | T04 载体 PR 实施期本地源码查证（禁凭记忆红线，本表预留行） |
| （预留） | — | 生产 AS 外接（配置迁移文档 + 可能的 IdP 兼容注记） | NB-WP05-1 条件触发（可得 IdP/部署环境） |
| （预留） | — | spec 新 revision 重评（iss SHOULD→MUST 升级预告等） | 新 revision 发布（评估件 §6.1，触发重评不追溯） |

## 待批准项

- [ ] 用户确认选型落锤（草稿合入 = 批准，沿 ADR-0030 待批准项①判例——
      合入时勾销）
- [x] **deny 实证回填位**：jsonwebtoken 11 依赖批次落地时（T04 载体 PR）
      本地 `cargo deny check` + `cargo audit` 输出贴入本 ADR 后果节
      （沿 ADR-0030 待批准项判例；deny 拦截即回本 ADR 修订，不改
      deny.toml 白名单）——✅ 2026-10-08 回填（后果节）
- [ ] rcgen/tokio-rustls 启面 deny 核对随 T03 载体 PR 门禁实证（评估件
      §1.6 已证两者在 lock，预期零新 crate，实证回填同上）
