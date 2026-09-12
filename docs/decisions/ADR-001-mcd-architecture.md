# ADR-001: partisync MCD 基础架构与技术选型

## Status
Accepted

## Date
2025-09-06

## Context
partisync 重新定位为**面向 AI/ML 团队的自托管数据资产管理（DAM）基础软件**。
MCD 阶段需要满足以下关键工程约束：
1. **高性能检索**：实现跨 100 万资产元数据/全文搜索 p95 < 100ms；
2. **高吞吐数据摄入**：支持批量图片/文档上传，支持元数据并发提取与哈希去重；
3. **后台慢标注降本**：低成本、异步可排队的 AI 标注作业体系；
4. **极简自托管**：单机 Docker Compose 一键拉起，尽量降低外挂组件运维复杂度；
5. **架构纯粹性**：MCD 阶段避免多语言混编，杜绝过早优化。

## Decision

### 1. 服务端核心语言：Go (Golang)
- **原因**：具备工业级并发 I/O 调度能力（Goroutine/Netpoll）、极速构建、静态编译单二进制（`CGO_ENABLED=0`），非常适合构建精简的 Docker Scratch/Alpine 镜像；已在 Immich、rclone、MinIO 等同类工业级工具中得到充分验证。
- **排除 Rust**：Rust 仅在后续阶段针对密集图像转换、SIMD 硬件加速和零知识加密 Vault 等高风险/极高 CPU 场景按需作为独立 Worker 引入，不在 MCD 增加工具链复杂性。

### 2. 搜索底座：Meilisearch (独立容器)
- **原因**：纯 Rust 编写，具备原生内存映射与高效倒排索引，开箱即用支持复杂过滤、分词和拼写纠错；在商业级百万资产管理（如 Scenario）中验证具备亚秒级甚至十毫秒级响应能力；通过官方 Go SDK 进行轻量 HTTP 通信。
- **排除自研/嵌入式 Tantivy**：避免 Go 通过 Cgo 绑定 Rust 带来的动态链接与交叉编译地狱。
- **排除纯关系库 FTS**：Postgres/SQLite 原生全文检索在 100 万复合排序/多维过滤下达到 p95 < 100ms 的调优成本过高。

### 3. 持久化与任务系统：PostgreSQL + DB-based 队列
- **主数据库**：PostgreSQL 16（支撑资产元数据、标签、集合、审计日志与版本控制）。
- **任务队列**：采用 PostgreSQL `FOR UPDATE SKIP LOCKED` 机制实现轻量高可靠工作队列，承载慢标注批处理与重试逻辑，**首版无需外挂 Redis**。

### 4. 存储层：本地文件系统 + 虚拟对象路径映射
- 资产文件物理落盘于 Docker Volume（`/data/assets/`），按分片哈希结构（如 `/data/assets/ab/cd/abcdef12...`）存储，服务端维护逻辑映射。

## Consequences
- 部署拓扑为经典的 3 容器组合：`partisync-server` (Go) + `partisync-db` (Postgres) + `partisync-search` (Meilisearch) + 静态资源。
- 保证构建流水线清晰，本地调试与 CI 均可在纯 Go 与 Docker 环境中一键运行。
- 检索 SLA 明确量化，必须提供可复现的 100 万资产基准压测脚本作为 L3 验收依据。
