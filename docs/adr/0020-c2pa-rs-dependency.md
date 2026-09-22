# ADR-0020: M4-WP05 C2PA 依赖：c2pa-rs

版本: 1.0 · 状态: **已接受** ·
关联: M4-WP05（C2PA 摄取校验与保留）、ADR-0017（推理栈批次先例：feature 门控重栈）、
ADR-0019（rmcp 精确锁定先例） ·
调研方案 §6 选型表（c2pa-rs = Adobe 官方 Rust SDK） ·
负责人: @lead · 起草日期: 2026-09-22

## 背景

M4-WP05 需要在摄取时校验并保留 C2PA manifest（调研方案 §6.3 管线第六环节、
§5.3 `content.c2pa` 列）。选型约束：

1. **工具链钉子 1.94**（rust-toolchain.toml，升级须独立 ADR + 全量回归）；
2. 构建链卫生：避免 openssl-sys（perl/make 构建链 + 平台差异），与 ADR-0017
   「重栈 feature 门控默认关」同纪律；
3. 只读校验场景：不需要签名器、HTTP 解析器、PDF 容器。

候选与版本事实（crates.io，2026-09-22 核实）：

| crate | 版本 | rust-version | 评估 |
|-------|------|--------------|------|
| `c2pa` | 0.91.0（2026-09-21） | **1.96.0** | ❌ 超钉子工具链 |
| `c2pa` | 0.90.22（2026-09-10） | 1.88.0 | ✅ |

## 决策

一句话：**`c2pa = "=0.90.22"`，`default-features = false`，仅开
`rust_native_crypto`**。

```toml
# workspace Cargo.toml
c2pa = { version = "=0.90.22", default-features = false, features = ["rust_native_crypto"] }
```

1. **精确锁定** `=0.90.22`：0.90 系列仍活跃发版（含 CVE 级修复节奏），锁定到
   patch 与 ADR-0019（rmcp `=3.4.0`）同口径；0.91.0 需 rustc 1.96，待工具链
   升级 ADR 通过后跟进（重新评估条件见下）。
2. **关闭 default features**：default = `openssl` + `default_http`
   （reqwest/ureq/wasi/wstd 四套 HTTP 客户端）。校验（读）路径两者皆不需要。
3. **rust_native_crypto**：纯 Rust 密码学栈（p256/p384/p521、rsa、
   ed25519-dalek——后者为 c2pa 必选依赖），ES256/RS256/Ed25519 验证全覆盖
   （已核实 0.90.22 源码 `crypto/raw_signature/rust_native/`）。
4. **只读面**：不开 `file_io`/`pdf`/`add_thumbnails`/`fetch_remote_manifests`/
   `json_schema`。

## 备选方案

| 方案 | 否决理由 |
|---|---|
| c2pa 0.91.0 + 工具链升 1.96 | 工具链升级 = 全 workspace 回归 + 基准重跑，代价远超本 WP；且不解决 openssl 问题 |
| c2pa + openssl feature | openssl-sys 构建链（perl/make）+ deny 面积大增；纯 Rust 路径已覆盖主流算法 |
| 自研 C2PA 解析（JUMBF/COSE 子集） | 规格体量（C2PA 2.4 数百页）+ 生态兼容性风险；铁律 8「无聊依赖」——该用别人的轮子 |
| 观望（不进场） | M4 DoD「可信与溯源」差异化能力的地基； WP05 是 M4 剩余工作包 |

## 后果

**正面**：零 openssl、零新增 HTTP 栈；Ed25519/ES256 全兼容；版本锁定可复现。

**负面 / 放弃**：
- 依赖树净增 30+ crates（coset/x509-parser/cbor 等，均为 MIT/Apache/BSD/ISC 族，
  cargo deny 四段实测为准）；
- 0.90.22 非 latest，0.91 修复需等到工具链升级；
- `verify_trust` 显式关闭（离线确定性）——Trusted 判定暂缺，非目标内登记；
- 时间戳验证（TSA/ocsp）依赖 HTTP resolver 的部分在离线 Context 下降级为
  informational/failure 状态记录，不阻塞清单保留。

## 重新评估条件

- 工具链钉子升 ≥1.96（独立 ADR）→ 跟进 0.91+ 并重评 feature 面；
- 出现影响校验路径的 RUSTSEC 公告 → 立即评估版本跟进或豁免；
- 需要签发/嵌入（Builder 写路径）或 PDF 容器 → 重开 feature 决策。
