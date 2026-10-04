# ADR-0030: 扩展签名分发选型——复用 minisign 线位（组件装载期强制验签）

版本: 0.1 · 状态: **草稿**（随 SPEC M9-WP04 批准生效——合入 = 批准，
沿 M9-WP00 判例；批准人 @lead）·
关联: SPEC M9-WP04 · ADR-0027（发布签名 minisign，接受——本 ADR 是其
预留「Rust 侧验证」路径的兑现与扩展面延伸）·
docs/reviews/M9-roadmap-proposal.md §3.4/§5-WP04（G6 空位 + cosign/OCI
趋势输入）· docs/reviews/M7-WP03-enterprise-topics.md §2（威胁模型
T-S1/T-S2，缓解 P1「扩展生态开放前必做」）· M9-WP00 §2 拍板项③
负责人: @lead · 批准人: @lead · 起草日期: 2026-10-04

## 背景

G6 扩展供应链空白（M7-WP01 §4-1 登记 → M9-WP00 §4 债表承接「WP04，
不自建 registry」）：ext-host 装载扩展仅凭目录同名配对
（`<name>.wasm` + `<name>.json`，manifest.rs 冻结规则）+ 目录扫描
（crates/partisync-ext-host/src/registry.rs:276 `scan`），**无签名校验、
无来源认证**——威胁模型草案（M7-WP03-enterprise-topics §2）定级
T-S1 恶意扩展投放 / T-S2 供应链篡改，缺口「下载源不可验证、用户无法
区分官方/第三方」，缓解方向「签名（Ed25519，锚定分发者公钥）+ 装载期
强制验签」，优先级 P1（扩展生态开放前必做）。

选型问题（M9-WP00 §2 拍板项③）：**复用 ADR-0027 minisign 线位 vs 引入
cosign/Sigstore**。约束：①「本地优先」立场——验证不依赖在线服务
（ADR-0027 否决 Sigstore/cosign 的原始理由）；②不自建 registry（硬
约束，分发模型为本地目录放置）；③新依赖需 ADR + cargo deny（铁律 8，
本 ADR 即载体）；④单人团队 + 双人保管，密钥面越小越好。

趋势输入（提案 §3.4）：WASM 签名分发收敛 cosign/OCI；但 MCP 官方
registry 不验 artifact 签名——对齐收益今日无兑现点。

## 四轴对比

| 轴 | 复用 minisign（ADR-0027 线位） | 引入 cosign/Sigstore |
|---|---|---|
| **验证面** | 单钥单签、纯离线；双钥已运行（主钥 `EBC32789A716D70A` + CI 子钥 `E056CBB62BF3EF34`，公钥入仓 docs/release/RELEASE-PUB-KEY.md:28-40）；终端零用户动作（公钥内嵌 ext-host + 装载期自动验签） | keyless 需 OIDC + Rekor 在线——本地优先冲突原样适用（ADR-0027 候选表否决理由）；key-based 可离线，但用户验证面 = 安装 cosign CLI（Go 二进制）或 Rust 侧 sigstore-rs（成熟度不足） |
| **依赖体量** | `minisign-verify` 0.3.x：纯验签、MIT、**零依赖**（docs.rs 实证 2026-10-04），deny 面近零增量；签名侧 minisign CLI 不进 workspace 依赖图（ADR-0027 决策 1 既有口径） | sigstore-rs 拉 OIDC/JWT/TLS 大依赖树且不成熟，cargo deny 面显著扩大；CLI 形态则验证外部化、CI 与用户机双重安装负担；两形态均与「无聊依赖」纪律相悖 |
| **OCI 生态对齐** | 无 OCI 生态——但**兑现条件不存在**：本 WP 硬约束不自建 registry，分发 = 本地目录扫描（registry.rs:276），artifact 不住 OCI，对齐无处附着；MCP 官方 registry 不验签 → cosign 兼容今日买不到互操作，纯未来期权 | 收敛方向真实（提案 §3.4），保留为修订触发条件（见修订登记）；届时可作「第二签名附」并陈，minisign/cosign 格式不互斥（ADR-0027:48 中性项既有登记） |
| **ext-host 接线成本** | `<name>.wasm` 同名第三文件 `<name>.minisig`；装载序 manifest 校验后、component 编译前插一步验签（拒绝先于任何解析/暴露，P14 fail-closed 同构）；minisign-verify 三步 API（`from_base64`/`Signature::from_file`/`verify`）；产品代码 <100 行 | 签名信封（simple signing envelope JSON）+ 公钥/证书解析需 sigstore-rs 或手拼，接线成本数倍；wasm component（非容器形态）非 cosign 主路径，工具链摩擦大 |

## 决策

1. **复用 ADR-0027 minisign 线位**。验签 crate：`minisign-verify`
   （0.3.x，MIT、零依赖）进 workspace——**新顶层依赖由本 ADR 承载**
   （ADR-0027:31-33 预留「Rust 侧验证用 minisign-verify」路径兑现；
   deny 核对随实施任务门禁实证，若出现多版本/传递许可异常，沿
   ADR-0011 先例以修订本 ADR 方式登记）；签名产出仍 minisign CLI
   （CI/开发者本机安装，不进依赖图）；
2. **锚定公钥 = 既有发布双钥**（主钥 + CI 子钥同列内嵌 ext-host，
   任一通过即有效——沿 RELEASE-PUB-KEY.md:22-26 既有验签语义）。
   一期无第三方分发者，扩展与发布产物同源同信：CI 子钥泄漏本就等于
   发布签名失守，复用不新增单点、不扩大攻击面。**专钥分域**
   （extension-dedicated 子钥）登记为修订触发条件（首个第三方分发者
   或企业客户强制 Sigstore/X.509 时）；
3. **装载期强制验签，无豁免通道**：`.wasm` 缺 `.minisig` → 拒绝；
   验签失败/格式非法 → 拒绝；拒绝点在 component 字节进入 wasmtime
   编译器**之前**（编译面攻击零暴露）。`allow_legacy = false`（新
   签名面无历史包袱）；
4. **签名对象 = `<name>.wasm` 字节**。manifest（`.json`）一期不入
   签名面——其篡改面评估为缩权/拒绝向（capability 提权无路径：
   [P14] 注入面 = 声明面 ∩ 宿主白名单；工具名替换撞内建或扩展间
   撞名均显式拒，registry.rs:263-267 `Duplicate`），联合摘要签名
   登记为开放问题（SPEC M9-WP04 §6-R3）。

## 后果

- 正面：G6/T-S1/T-S2 最小清偿（信任锚先于生态开放落位）；验证零在线
  依赖（本地优先）；验签 crate 零依赖、deny 面近零增量；与发布签名
  同一工具链、同一公钥文档面——认知与运维成本最低；
- 负面/风险：OCI 生态对齐让渡为未来期权（触发条件见修订登记）；
  manifest 完整性缺口（决策 4，开放问题）；双钥复用的组织面风险 =
  钥泄漏影响「发布 + 扩展」双面（与现状发布面等量，不新增单点）；
- 中性：签名格式不锁死——未来 cosign 签名可作第二附并陈，不动验签
  主路径；本 ADR 不改变 ADR-0027 发布签名任何既有口径。

## 修订登记

| 版本 | 日期 | 修订内容 | 触发/依据 |
|---|---|---|---|
| 0.1 | 2026-10-04 | 初稿随 SPEC M9-WP04 起草（T01）：minisign 线位落锤 | M9-WP00 §2 拍板项③（「先出评估 ADR 再拍板」） |
| （预留） | — | 专钥分域 / cosign 第二签名附 / manifest 联合摘要 | 触发条件：首个第三方分发者；OCI 分发通道落地；企业强制 Sigstore/X.509；manifest 篡改面出现真实闭合需求 |

## 待批准项

- [ ] 用户确认选型落锤（草稿合入 = 批准，沿 M9-WP00 判例——合入时勾销）
- [ ] 示例扩展以生产公钥签名（密钥持有人动作，挂 T03；测试路径用
      test-only 钥自足，不阻塞 T02——沿 ADR-0027 待批准项②判例）
