# Task: M9-WP04-T01 WP04 SPEC 起草 + 签名选型 ADR-0030 落锤 + P21 登记

> **范围外**：minisign-verify 实装与探针 = T02；签名流程文档与示例扩展
> 签名（密钥持有人动作）= T03；不操控 GUI（本 WP 无桌面 UI 面）；不改
> M9-WP00.md 状态回填（随 T03 收尾）。

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M9-WP04-T01 |
| **类型** | 起草（纯文档 PR；R0——产品依赖图零改动） |
| **优先级** | P0（M9-WP00 §2 拍板项③「扩展签名选型 WP04 SPEC 时落锤」的兑现任务） |
| **范围** | SPEC M9-WP04 全文 + ADR-0030（minisign vs cosign 四轴对比落锤）+ docs/tests/properties.md P21 登记行 + 本卡 |
| **创建日期** | 2026-10-04 |
| **来源** | M9-WP00 §1-WP04/§2 拍板项③/§4 债表 G6 行 + M9-roadmap-proposal §3.4/§5-WP04 + M7-WP03-enterprise-topics §2（威胁模型 T-S1/T-S2，P1）+ ADR-0027（minisign 线位） |

## 契约落点

1. **选型落锤（ADR-0030）**：复用 ADR-0027 minisign 线位——四轴对比
   （验证面 / 依赖体量 / OCI 生态对齐 / ext-host manifest 接线成本）后
   cosign 让渡为修订触发条件（OCI 分发通道落地 / 第三方分发者 / 企业
   强制 Sigstore）。minisign-verify 0.3.x 入根由 ADR-0030 承载
   （ADR-0027:31-33 预留路径兑现，deny 核对随 T02 门禁实证）。
2. **验签契约（SPEC §2.2，P21）**：`<name>.wasm` 同名 `.minisig` 第三
   文件；装载序 manifest 校验后、component 编译前强制验签（wasmtime
   攻击面零暴露，P14 fail-closed 同构）；缺签 `Unsigned` / 坏签
   `BadSignature` 双变体装载即拒；双公钥（发布主钥 + CI 子钥）任一
   通过即有效；无豁免开关。
3. **P21 登记行**（properties.md）：「扩展装载必经验签」不变量——
   篡改/缺签/未知钥三路必拒且先于编译；候选登记随本 PR，转正随 T02
   探针（沿 P19「T01 登记、T02 实现」判例）。
4. **任务切分（SPEC §任务清单）**：T01 起草（本卡）→ T02 实装（验签
   + 五路探针 + fixtures 签名）→ T03 文档收尾（EXT-SIGNING.md +
   RELEASE-PUB-KEY 增补 + 示例扩展签名持有人动作 + WP00 状态回填）。

## 边界与既有决定

- SPEC 状态「草稿（合入 = 批准）」沿 M9-WP00 判例（M8-WP00 PR #59）；
  ADR-0030 状态「草稿（随 SPEC M9-WP04 批准生效）」沿 ADR-0029 判例。
- minisign-verify API 事实起草期经 docs.rs 实证（0.3.0：
  `PublicKey::verify(&self, bin: &[u8], signature: &Signature,
  allow_legacy: bool)`；MIT；零依赖），T02 以本地 cargo doc/deny 复核
  为准（SPEC §6-R1）。
- 锚定公钥复用发布双钥（RELEASE-PUB-KEY.md:28-40 已入仓，AI 不接触
  密钥材料）；专钥分域挂修订触发。
- manifest 不入签名面：篡改面评估为缩权/拒绝向（P14 无提权路径 +
  `register()` 扩展间撞名 `LoadError::Duplicate` 显式拒，
  crates/partisync-ext-host/src/registry.rs:263-267），开放问题登记
  SPEC §6-R3。

## 验收

- [x] docs/specs/M9-WP04.md 落盘：任务切分 + §3 验收可执行 + §5 文件
      清单 + §6 风险（六项含处置）；
- [x] docs/adr/0030-extension-signing-toolchain.md 落盘：四轴对比表 +
      决策四条 + 后果 + 修订登记表 + 待批准项；
- [x] docs/tests/properties.md P21 行登记（格式沿 P14/P19 行）；
- [x] 本卡落盘（先卡后工）；
- [x] 产品依赖图零改动（Cargo.toml/deny.toml/rust-toolchain.toml 不在
      本任务清单）；
- [x] fmt/clippy/test 三件套全绿（文档 PR 沿门禁全量跑）。
