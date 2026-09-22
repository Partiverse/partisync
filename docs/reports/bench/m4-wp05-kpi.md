# M4-WP05 KPI 底稿（T05 校验与保留基准）

日期: 2026-09-22 · 环境: Apple Silicon (arm64, macOS 25.6.0, 24 GB) ·
debug profile（未跑 release 基准——校验路径为冷门摄取侧，KPI 以正确性为主）·
关联: SPEC M4-WP05 验收标准、ADR-0020（c2pa =0.90.22 纯 Rust 面）

## 1. 依赖进场事实

| 项 | 值 |
|---|---|
| crate / 版本 | `c2pa =0.90.22`（crates.io 2026-09-10 发布；0.91.0 需 rustc 1.96 > 钉子 1.94，跟进条件见 ADR-0020） |
| features | `default-features = false, features = ["rust_native_crypto"]`——零 openssl、零新增 HTTP 栈 |
| 依赖树增量 | c2pa 及其传递（coset/x509-parser/ed25519-dalek/rsa 等），编译时间 +6m21s（partisync-ai 全量重编口径） |
| deny | licenses/bans/sources 真实退出码 0；advisories 本地缓存 DB（GitHub 不可达，网络披露同 WP03） |
| advisories 豁免 | RUSTSEC-2023-0071（rsa Marvin Attack）：本仓库仅公钥验证路径，私钥解密侧信道不可达；上游无修复版（RustCrypto #626）；撤销条件登记于 deny.toml |

## 2. 校验语义矩阵（测试实证，wp05.rs 8/8）

| 输入 | 期望 | 实测 | 测试 |
|---|---|---|---|
| ES256 自签 JPEG（含 c2pa.created） | done + `state=Valid` + blob JSON | ✅ | `validates_and_preserves_signed` |
| 无清单 PNG | done + `state=absent` + 无 blob | ✅ | `absent_on_unsigned` |
| 垃圾字节（非容器） | skipped `format-unsupported-by-c2pa` | ✅ | `unsupported_container_skipped` |
| 篡改扫描数据（len-100） | done + `state=Invalid` + blob 保留 | ✅ | `tampered_still_preserved_as_invalid` |
| text/plain | pipeline skipped `not-applicable` | ✅ | `pipeline_not_applicable` |
| 签名样本跑管线 | content.c2pa 列 = 可解析 manifest JSON | ✅ | `pipeline_writes_content_column` |
| 无清单跑管线 | 列 NULL + 重跑 no-op | ✅ | `pipeline_absent_and_idempotent` |
| gateway asset_read | `sidecar_stages.c2pa` 字段 | ✅ | gateway `asset_read_returns_full_metadata` |

## 3. 签发侧 fixture 事实（复现口径）

- 测试证书：P-256 (prime256v1) 自签，KU=digitalSignature(critical)，
  EKU=**1.3.6.1.4.1.62558.2.1**（C2PA Signing，critical），BasicConstraints CA:FALSE。
- **坑位披露**：`codeSigning (1.3.6.1.5.5.7.3.3)` 不在 c2pa 默认合法 EKU 表
  （`valid_eku_oids.cfg`：emailProtection/documentSigning/timeStamping/OCSPSigning/
  MS-C2PA/C2PA-Signing 六项）——首版 fixture 被签名侧
  `CertificateProfileError(InvalidCertificate)` 拒绝，探针定位后换用 C2PA EKU。
- manifest 定义须含 `c2pa.actions` 首动作 created/opened，否则读回
  `assertion.action.malformed` → state=Invalid（首次自签样本踩中，已修正）。
- 篡改用例的翻转位选择：JPEG 布局 = JUMBF 盒在前、扫描数据贴 EOI；**清单盒内
  字节属 dataHash 排除区**（翻转后仍 Valid），故翻转位锚定 `len-100`（扫描区）。
  位置扫描探针证据：300→AssertionDecoding、500→Invalid、800→ClaimDecoding、
  1689..12517→Valid（排除区）、13017/13417→Invalid。

## 4. 门禁状态（T05 时点）

| 门禁 | 结果 |
|---|---|
| cargo fmt --all --check | ✅ |
| cargo clippy（ai/graph/gateway --all-targets -D warnings） | ✅ |
| cargo test -p partisync-ai | ✅ 44/44（含 wp05 8/8） |
| cargo test -p partisync-graph / -p partisync-gateway | ✅（gateway asset_read 断言含 c2pa） |
| cargo deny 四段 | ✅（advisories 本地缓存） |

## 5. 开放项

| 优先级 | 项 |
|---|---|
| P1 | 真实世界 C2PA 样本冒烟（多 ingredient/Adobe/CAI 官方样本）——当前仅自签样本覆盖 |
| P2 | verify_trust 可配置化（信任列表接入 → Trusted 判定） |
| P2 | dataset_export 输出含 c2pa 清单字段（SPEC 非目标，M5 评估） |
| P3 | blob 通道与列双写的存储裁剪（清单仅留列即可） |
