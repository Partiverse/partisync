title: 家庭 K8s 集群搭建
filename: k8s_personal_cluster.md
tags: [kubernetes, k8s, k3s, homelab, self-hosting]
updated_ns: 1715817600000000000

# 家庭 K8s 集群搭建

## 硬件

- 3 节点 Raspberry Pi 5（8GB）
- 1TB NVMe SSD（USB 3.0）
- 千兆交换机
- UPS（防断电）

## 系统选择

K3s（轻量 K8s）：
- 单 binary <100MB
- 内存 <512MB
- 内置 etcd（SQLite 模式）
- ARM64 原生支持

## 部署

```bash
curl -sfL https://get.k3s.io | sh -s - \
  --write-kubeconfig-mode 644 \
  --disable=traefik \
  --node-name=picluster-01
```

## 关键服务

- **Longhorn**：分布式存储（替代 NFS）
- **MetalLB**：LoadBalancer（家庭网段）
- **cert-manager**：自动 TLS（Let's Encrypt）
- **Authentik**：身份认证
- **Gitea**：自托管 Git
- **Jellyfin**：家庭媒体
- **Nextcloud**：私有云盘

## 网络

- Tailscale（异地组网）
- Cloudflare Tunnel（家庭 NAT 后公网暴露）
- 内网 DNS（Pi-hole）

## 备份

- Velero → Backblaze B2
- Restic → 加密本地备份
- K8s manifests 入 Git（ArgoCD GitOps）

## 监控

- Prometheus + Grafana
- Loki（日志聚合）
- Alertmanager（钉钉/邮件告警）

## 学习

- CKA 认证
- kubebuilder（Operator 开发）
- Argo Rollouts（渐进式发布）

## 经验

- ARM64 镜像注意（部分镜像无 arm64 tag）
- SSD 通过 USB 3.0 接入性能可接受
- Pi 5 单节点可跑 50 pod 不卡

## 与 PartiSync 集成

- K8s 部署 PartiSync Hub demo（演示场景）
- asset_search 检索家庭媒体