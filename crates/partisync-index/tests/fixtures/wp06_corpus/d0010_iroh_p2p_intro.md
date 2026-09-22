title: iroh P2P 网络入门
filename: iroh_p2p_intro.md
tags: [iroh, p2p, networking, rust, quic]
updated_ns: 1704412800000000000

# iroh P2P 网络入门

## 什么是 iroh

iroh 是一个 Rust 写的 P2P 网络库，建立在 QUIC 之上：
- 节点 = 公钥（Ed25519）
- 直连优先，无法直连走中继
- 0-RTT 建链
- 内置 blob / gossip / docs 子协议

## 核心概念

| 概念 | 含义 |
|---|---|
| NodeId | 32 字节 Ed25519 公钥 |
| NodeAddr | NodeId + 网络可达地址（直连或中继） |
| Endpoint | iroh 节点入口 |
| Connection | QUIC 连接 |
| Router | 多协议复用 |

## PartiSync 用法

- **数据面**（M4-WP04）：1 GiB 上行实测通过
- **Hub 分片联邦**：按 content_id 前缀路由
- **设备发现**：基于 iroh NodeId + 公开地址发现服务

## 直连 vs 中继

- 直连：双方至少一方有公网 IPv4/IPv6 + 端口可达
- 中继：双方都 NAT 时通过 iroh relay server 中转
- 中继是 P2P 网络基础（非可选）

## 设备侧 UploadAck（M5）

P0 前置：设备侧 push fire-and-forget 语义。
- 设备侧：Ack 收到 = 持久化完成
- Hub：超时未收到 Ack → 重传

详见 `docs/specs/M4-WP06.md` 风险与开放问题。