# M12-WP03-T01 评估件——示例扩展签名腿方案对证（install-demo-ext.sh 验签脱节清偿）

> Task-ID: M12-WP03-T01 · 日期: 2026-10-11 · 上游: M11-report §5 债表
> 行 4 + M12 charter WP03 · 现场事实核对在案（signature.rs / release.yml /
> EXT-SIGNING.md / commands.rs，行号随文）

## 1. 问题重述

`scripts/install-demo-ext.sh` 只拷贝 `demo_ext.{wasm,json}` 不带
`.minisig`——M9-WP04 起 [P21] 强制验签（registry.rs:65-89，缺签 =
`LoadError::Unsigned` 拒载）生效后，**装了必被拒**：本机 desktop e2e
`mcp_call_real_sidecar_ext_list` 红灯根因（M11-report §3/§5-4），
「扩展装载」体验面自 M10 起持续不可用（EXT-SIGNING.md:48-57 示例扩
展签名窗口，M10-WP06-T03 登记）。

## 2. 锚定钥现场事实（方案空间的决定性约束）

`ANCHOR_PUBKEYS: [&str; 2]`（signature.rs:17-24，**任一通过即有效**）：

| 钥 | 指纹 | 保管 | 已有签名判例 |
|---|---|---|---|
| 主钥 | EBC32789A716D70A | 离线双人保管（生成仪式 2026-10-01） | alpha/beta/rc 产物 |
| **CI 子钥** | E056CBB62BF3EF34 | **仅 GitHub secret**（RELEASE_SIGNING_CI_KEY，人不经手） | release.yml:186-199 每次发布产物签名 |

## 3. 方案对照

| 方案 | 内容 | 判定 |
|---|---|---|
| A 主钥签名 | 为 demo fixture 走离线双人取钥仪式 | **否**——仪式成本与 fixture 价值不成比例；key-custody AI 不可代办 |
| B 仅守卫健壮化 | e2e skip 守卫校验 minisig 存在性 | **部分采纳（伴生项）**——消除误红，但「扩展装载」体验面维持不可用，EXT-SIGNING 窗口未闭合 |
| **C′ CI 子钥签名，minisig 随仓** | 一次性 workflow_dispatch 用 RELEASE_SIGNING_CI_KEY 签 `examples/extensions/demo_ext.wasm`，`.minisig` 经 PR 入仓，脚本加拷贝 | **推荐**——CI 子钥**已是锚定钥**：零 ANCHOR_PUBKEYS 变更、零 ext-host 代码变更、零 [P21] 语义变化；钥不经手沿 ADR-0027/key-custody 与 release.yml:186-199 同款判例；签名真实有效（装载体验真正闭合） |
| D 新锚 dev 钥 | ANCHOR_PUBKEYS 增第三钥 | **否**——验签锚定面扩张 = R2 双审 + ADR，为 demo fixture 不成比例 |

红线核对：P21 验签语义零变化（方案 C′ 是「补齐合法签名」而非「绕过
验签」）；ADR-0027 密钥保管不变（CI 子钥继续仅存 secret）；无许可证
面（.minisig 是签名文件非代码）。

## 4. 结论与去向

采纳 **C′ 主线 + B 伴生**，落 SPEC `docs/specs/M12-WP03.md`：T01 = 本
评估 + SPEC（本 PR）；T02 = 实施（签名工作流 + minisig 入仓 PR +
脚本加拷贝与告警 + e2e 三态守卫 + 防漂移校验步）。清偿对象：M11-report
§5 行 4 + EXT-SIGNING 示例扩展签名窗口（M10 债）+ T08 域 H「扩展装
载」不可体验项。
