# M8-WP02 发布工程实跑报告（G4 检查单）

任务: M8-WP02-T02/T03 · 日期: 2026-10-01 · 版本: **v0.1.0-alpha**
（tag 指向 `84c342d`）· release workflow run `36889384528`（全绿）·
规格: [specs/M8-WP02.md](../../specs/M8-WP02.md) · 签名: ADR-0027 +
key-custody.md

## 1. Release 资产清单（draft，待用户终审后 publish）

| 产物 | 签名 |
|---|---|
| partisync-cli-macos-aarch64.tar.gz | ✅ .minisig |
| partisync-linux-x86_64.tar.gz（CLI + hub-demo-web） | ✅ .minisig |
| PartiSync.Desktop_0.1.0_aarch64.dmg | ✅ .minisig |
| SHA256SUMS | ✅ .minisig |

## 2. G4 检查单（执行方案 §5.4）

| # | 项 | 状态 | 证据 |
|---|---|---|---|
| 1 | 可复现构建（`--locked` + toolchain 钉版） | ✅ 部分 | 三构建 job 全程 `cargo auditable build --locked` + rust-toolchain 1.94 钉版；**CI 内双构建 hash 对照未执行**（§4 偏差 1） |
| 2 | SBOM（cargo auditable） | ✅ 部分 | 依赖清单已嵌入二进制（auditable 构建）；**独立 SBOM 导出文件未附 Release**（§4 偏差 2） |
| 3 | 签名（minisign 双钥） | ✅ | 端到端验签实测通过（CI 子钥公钥 + `minisign -Vm` → Signature verified）；双钥口径见 key-custody.md |
| 4 | changelog（AI 起草人终审） | ✅ | generate_release_notes 自动起草；人终审随 publish 执行 |

## 3. 签名体系（双钥，ADR-0027 + key-custody 修订）

- **主钥**（指纹 `EBC32789A716D70A`）：离线双人保管，口令分持两半；
  用于应急/最终签名，不出 CI；
- **CI 子钥**（指纹 `E056CBB62BF3EF34`）：无口令（`minisign -G -W`），
  secret `RELEASE_SIGNING_CI_KEY` 仅 CI 可读；本次全部产物由子钥签署
  （trusted comment 标 `CI subkey`）；
- 触发路径：实测 C 版 minisign 0.12 无密码环境变量（`get_password()`
  强制 tty）→ 走 key-custody 预登记演进路径切换子钥；公钥双签入
  `RELEASE-PUB-KEY.md`；
- 验签步骤（干净环境实测）：

```sh
minisign -Vm SHA256SUMS -p partisync.pub   # → Signature and comment signature verified
sha256sum -c SHA256SUMS                     # 逐产物校验
```

## 4. 偏差与遗留登记

| # | 项 | 说明 | 去向 |
|---|---|---|---|
| 1 | CI 内双构建 hash 对照 | SPEC §3 项未执行（release job 无第二构建通道） | 独立任务：release.yml 加 reproducibility job（同 input 双构建比对，macOS 产物预期偏差按 R1 登记） |
| 2 | SBOM 导出文件未附 Release | auditable 嵌入已达成，独立 CycloneDX/SPDX 导出缺 | 随偏差 1 同任务（`cargo auditable` + cyclonedx 生成附 Release） |
| 3 | dmg 未公证 | Apple Developer 账号外部依赖（SPEC 非目标） | 触发条件：对外分发企业客户时 |
| 4 | hub 产物 = hub-demo-web | hub crate 无正式服务 bin（v0.1 口径如实登记） | 正式 hub 服务 bin 随 WP03 后续任务 |
| 5 | 桌面 icon 为占位 | tauri icon 从 M6 截图生成 | 正式 icon 设计归 WP05 |
| 6 | Release 为 draft | publish（对外发布动作）待用户终审 changelog 后执行 | 用户动作 |
| 7 | macOS runner test job 与 release 并发挤占 | 今日 CI 4 次 test flake（upload_ack×2/m5_wp03×2/quota×1，均 rerun 绿、阈值未动） | 独立整改：ci.yml 加 concurrency group 或 macos test 错峰 |

## 5. 实跑迭代记录（9 跑，8 修——SPEC §1「只有真实打一次 tag 才会暴露的工程问题」的实证）

| 跑 | 失败点 | 根因 | 修复 PR |
|---|---|---|---|
| 1 | desktop `--locked` 不被接受 + linux 缺 `partisync-hub` bin | tauri-cli 参数面 / hub crate 无主 bin | #77 |
| 2 | bundler: `icons/icon.icns` 缺失 | 桌面壳从未 bundle 过 | #78 |
| 3 | release: cargo install minisign「无 binaries」 | crates.io crate 纯库 | #79 |
| 4 | tag `v0.11.0` 不存在 | **凭记忆写版本（违反查证纪律）** | #80 |
| 5 | 0.9 tag 无 Cargo.toml | 该仓库 ≤0.9 为 Go 实现 | #81 |
| 6 | tar 内路径不符 | C 版 archive 布局 minisign-linux/x86_64 | #82 |
| 7 | `get_password()` tty panic | C 版无密码环境变量 → CI 子钥切换 | #83 |
| 8 | 产物单签缺失 + checksums 前缀 | maxdepth/子目录前缀 | #84 |
| 9 | **全绿**（4 job） | — | — |

教训：①第三方工具链版本/布局必须 ls-remote/下载实测，不得凭记忆（#80
违反铁律实证）；②签名工具链选型期应验证「CI 非交互签名」路径（#83
触发保管口径修订）；③dmg 含空格名对所有 shell 循环都要 -print0。

## 6. 复核日志

- 执行: GLM-5.3-Flash（ZCode，M8-WP02-T02/T03）· 门禁: 每 PR CI 8/8 绿
- Release publish（对外可见）待用户终审后手动执行（draft 状态）
- 复现构建验证、SBOM 导出、公证触发条件见 §4
- **用户终审签收（2026-10-02）**：changelog 终审通过，v0.1.0-alpha 已
  publish（draft=false，publishedAt 2026-10-01T16:40:49Z，
  github.com/Partiverse/partisync/releases/tag/v0.1.0-alpha）——
  **首个对外签名可分发版本正式生效**，G4 第 4 项闭环
