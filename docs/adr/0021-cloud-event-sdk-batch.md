# ADR-0021: M5-WP04 云事件流 SDK 批次：aws-sdk-sqs / webhook HMAC，Kafka 延期

版本: 1.0 · 状态: **批准（2026-09-25 用户指令「推进 SDK ADR」）** ·
关联: M5-WP04（云事件流摄取引擎，规格 §裁定 6/契约 3 与 §风险「SQS/Kafka
SDK 引入」）、ADR-0016（依赖豁免先例）、deny.toml
负责人: @lead · 起草日期: 2026-09-25

## 背景

M5-WP04 交付了统一 `EventSource` 抽象 + Webhook 通道（axum，已闭环）+
SQS/Kafka stub。正式 SDK 属新增顶层依赖，铁律 8 要求 ADR + cargo deny。
本 ADR 补齐该决策。

## 备选与决策

### 1. SQS：`aws-sdk-sqs`（采纳，Phase 1）

| 备选 | 评估 |
|------|------|
| **aws-sdk-sqs（官方 AWS SDK for Rust）** | 纯 Rust、tokio 原生、无 C 原生依赖；依赖树大（~100 crates）但 licenses 全过（Apache-2.0）；credential/重试/区域装配成熟 |
| 手写 SIGv4 + GetQueueUrl/ReceiveMessage/DeleteMessage | 依赖面最小，但签名/重试/可见性超时自研面大——重复造轮子 |
| 不接 SQS，仅 webhook | webhook 需要可达的反向入口（NAT/防火墙），SQS 拉模式对家庭/私有部署是唯一通路 |

**采纳 aws-sdk-sqs**，feature 门控 `event-sqs` 默认关（CI 不背 AWS 构建
链；启用方显式 `--features event-sqs`）。

### 2. 版本锚定与 MSRV 上限

```
aws-sdk-sqs  = "=1.102.0"   # 1.112+ 要求 Rust 1.94.1 > rust-toolchain 锁定的 1.94.0
aws-config   = "=1.8.18"    # 凭证/区域装配（同链 MSRV 约束）
```

MSRV 漂移防护：exact pin；升级需同步评估 rust-toolchain.toml（改它本身
需 ADR）。

### 3. webhook HMAC：`hmac` + `sha2`（采纳，随本批次）

RustCrypto 纯 Rust、依赖树极小，不门控。用于 MinIO `X-Amz-Signature` /
S3 事件签名校验（WP04 报告 §5.2 遗留）。**校验算法实现归接线卡**
（constant-time 比较等，D2 OSCP 进场重审）。

### 4. Kafka：`rdkafka` **延期**（不采纳于本批次）

rdkafka 绑定 librdkafka C 库——cmake 构建链 + 平台二进制 + deny sources
面扩大，与「无聊依赖」原则冲突；WP04 验收已由 mock + webhook 覆盖。
Kafka 需求真实出现时另立 ADR（备选：纯 Rust 的 `kafka`/`rskafka` 生态
成熟度届时重评）。

## cargo deny 结果（本 ADR 落地时点）

- licenses/bans/sources：**ok**（aws 链 Apache-2.0；RustCrypto MIT OR Apache-2.0）。
- advisories：aws-smithy-http-client 1.4.2 **双栈 HTTP 客户端**拉 hyper 0.14
  → h2 0.3.27、rustls 0.21 → webpki 0.101.7，四条新公告（RUSTSEC-2026-0258
  [Low]、0098/0099/0104）**在旧 major 线无修复版本**。按 ADR-0014/0016
  先例登记 ignore + 撤销条件（`aws-smithy-http-client` 弃用 hyper 0.14
  双栈）。可达性论证见 deny.toml 注释：本仓库仅作出站客户端（SQS 拉
  模式），h2 空帧排队是对端行为不可达；webpki 校验对象限 AWS 信任根。

## 实现记录（2026-09-25）

钉版 aws-sdk-sqs 1.102.0 后发现其传递链仍解析到 MSRV 1.94.1 版本
（sqs 1.102 req http-client ^1.4.2 → smithy-runtime ^1.15 需要 1.94.1；
aws-config 1.8.18 req sts ^1.106 同）。逐包 `--precise` 降级被 req 死锁
挡死。正解 = **MSRV-aware resolver**：`.cargo/config.toml` 配置
`[resolver] incompatible-rust-versions = "fallback"` + Cargo.lock 全量
重生成——解析器自动选 1.94.0 兼容链（sqs 1.102.0 / runtime 1.7.5 /
http-client 1.1.13 / sts 1.107.0），`--features event-sqs` 编译通过。
验证：cargo deny 四项全绿；全 workspace 回归 446 passed / 0 failed。

## 后果

- `partisync-sync` 新增 feature `event-sqs`（默认关）+ 直接依赖
  hmac/sha2（默认开，轻量）。
- SQS 适配器实装（`poll_batch` = ReceiveMessage、`commit_cursor` =
  DeleteMessageBatch）归接线卡，contract 已由 WP04 `EventSource` trait
  钉死。
- deny ignore 4 条 = 新增审计面：D2 OSCP 进场时把 SQS 凭证面与 webhook
  HMAC 一并复审。
