# ADR-0006: Provider SPI 以 OpenDAL 0.59 为底座

状态: 已接受 · 日期: 2026-09-19 · 决策人: @lead（AI 代理起草，人工签核待补）
关联: SPEC M1-WP00/WP01、ADR-0002、调研方案 §5.5/§6

## 背景
M1 需要消费 50+ 云存储。调研方案 §6 既定 OpenDAL；本次定案版本与集成方式。

## 决策
1. `opendal 0.59.2`（crates.io 核验），workspace 关闭 default features，
   按 feature 启用后端：`services-s3 / services-webdav / services-fs`（后续按需加）。
2. 0.59 架构变化（源码核验）：服务拆分为独立 crate（opendal-core + opendal-service-*），
   `opendal::services::S3` 即 S3Builder；`Operator::new(builder)` 统一构造；
   配置字段（bucket/endpoint/region/access_key_id/secret_access_key/root）为 builder setter。
3. Provider SPI 包薄适配层：`Provider::from_config(ProviderConfig{scheme, params})`，
   caps 能力位来自 partisync-core::caps（Default 保守，P9 前提）。
4. `Operator::from_iter`（kv 迭代器构造）用于 JSON 参数到 builder 的通用映射——
   避免为每个后端手写参数分支（调研方案 §5.5 caps 协商设计的配套）。

## 备选
- 手写各云 SDK（aws-sdk-s3 直连保留为 S3 特化路径，通用面走 OpenDAL）；
-自写存储抽象：重复造轮子，否决。

## 后果
- 特性裁剪后编译面可控；新增后端 = Cargo feature + 配置行，无代码变更；
- OpenDAL 0.x API 漂移风险由 SPI 内部隔离（下游只见 Provider trait 面）。

## 重新评估条件
- OpenDAL 0.59 停更或破坏性变更 >1 季度 → 锁定 vendor 或评估手写 S3/WebDAV 双协议。
