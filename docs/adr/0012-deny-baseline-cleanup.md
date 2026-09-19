# ADR-0012: deny 基线清理——CDLA-Permissive-2.0 白名单 + 内部依赖 wildcard 根治

状态: 已接受 · 日期: 2026-09-20 · 决策人: @lead（AI 代理起草，依据 SEC-AUDIT-2026-M3-002/003 整改建议执行；「无聊依赖」治理批次，先例 ADR-0002）
关联: deny.toml、SEC-AUDIT-2026-M3-002 §四.2/§六、SEC-AUDIT-2026-M3-003 §六（deny 失败 P1）、M2 密码学审计报告 §2.4

## 背景

cargo-deny 0.20.2 口径下基线既有两项失败（fjall 进场前即红，非 ADR-0011 引入；
SEC-AUDIT-2026-M3-002/003 两轮独立审计均确认，并要求在 WP01 实现推进前关闭）：

1. **licenses**：`webpki-root-certs 1.0.9`（rustls-platform-verifier → reqwest
   dev 链）许可证 `CDLA-Permissive-2.0` 不在白名单；
2. **bans**：`wildcards = "deny"` 判罚 14 处内部 path 依赖（7 crate）——
   path 依赖未写版本号即被视为 `*` 通配。

## 决策

1. deny.toml `[licenses] allow` 增补 **CDLA-Permissive-2.0**——Linux 基金会
   Community Data License Agreement（Permissive）2.0：类 BSD 的宽松数据许可，
   OSI 认可、 SPDX 标准条目、社区广泛接受（rustls 生态默认携带）。仅增白名单，
   不动任何阈值。
2. **wildcard 根治**（不改 deny.toml）：7 个 crate 的 14 处内部 path 依赖统一
   显式声明 `version = "0.1.0"`（与 workspace 版本一致）——依赖声明卫生修复，
   保持 `wildcards = "deny"` 严格阈值不变。

## 备选

- `wildcards` 阈值降为 `warn`：放松门禁换取零改动——否决（阈值只升不降纪律）；
- 把内部 crate 登记进 `[workspace.dependencies]` 走 `.workspace = true` 继承：
  结构更优但改动面大（root + 全员），留待后续重构任务，本 ADR 取最小修复；
- 排除（skip）webpki-root-certs：治标且掩盖真实许可信息——否决。

## 后果

- `cargo deny check licenses bans` 转绿；advisories 依赖 RustSec DB 网络拉取，
  本机 GitHub 不可达期间无法执行（离线缓存口径见 SEC-AUDIT-2026-M3-002 §5.1：
  0 vulnerabilities / 406 deps），网络恢复后补跑；
- 内部 crate 版本号现在两处声明（workspace.package + 各依赖行）——发版时
  需同步；上述 workspace.dependencies 重构任务将消除该重复。
