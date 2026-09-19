# ADR-0008: 设备网络层以 iroh 1.x 为底座

状态: 提议 · 日期: 2026-09-19 · 决策人: @lead（AI 代理起草，人工签核待补）
关联: SPEC M2-WP04、调研方案 §5.8（设备↔设备通道）

## 背景
M2-WP04 需要设备网络层：手机/NAS/工作站直连，移动网络切换友好，QUIC 打洞 +
中继回退。调研方案 §6 既定 iroh；本次定案版本与配对机制。

## 决策
1. `iroh 1.x`（具体小版本待引入时 crates.io 核验——版本漂移风险，锁定 minor + 隔离层）。
2. 助记词配对：12 词英语词表（BIP-39 子集 256 词够用）；派生 Ed25519 密钥对 +
   ECDH（X25519）；挑战 = 服务端 nonce + ECDH(配对码, 服务端公钥) hash；
   客户端 ECDH(配对码, 客户端私钥) 验证 — 双方共享 secret ⇒ hash 比对通过。
3. 设备注册表：本地 `device` 表扩 `endpoint / pubkey / last_endpoint_update_ns`
   （现有 device 表：id/name/slug/pubkey/capabilities/last_seen——规格 wp01 规格
   已有 pubkey 与 last_seen 字段，需补 endpoint 列）。
4. 协议隔离层：`partisync-sync::transport::iroh::IrohTransport` 实现内部 trait，
   与同进程双 Store 模拟对端共存——bisync / reconcile 走「对端句柄」抽象面，
   同进程实现归 M2-WP03 路径，iroh 实现归本 WP，调用方无感。

## 备选
- libp2p：API 较繁琐，配对协议需自实现更多面积（Yamux/Mplex/Noise 各面），
  不如 iroh 的「少样板 + QUIC 默认」贴合调研方案 §5.8 ；
- 自写 QUIC（quinn）：能控但工作量大，与本期进度不符，否决；
- ZeroTier/Tailscale：依赖外部 daemon，资产自包含目标冲突，否决。

## 后果
- iroh 0.x→1.x API 漂移已被调研方案 §6.3 风险节点名（与 ADR-0005 同模式）；
  隔离层 + 锁定 minor 是缓解策略；
- 移动网络切换友好（连接迁移）原生地好——比自研 QUIC 省至少一季工作量；
- 助记词配对的"凭据在线"路径将依赖新增 Ed25519 + X25519 + 哈希库（见 ADR-0010 配生）。

## 重新评估条件
- iroh 1.x 停更或破坏性变更 >1 季度 → 评估 libp2p 或自写 quinn；
- 移动端壳 M6 引入时实测打洞失败率 >10% → 评估 Tailscale 兜底路径。