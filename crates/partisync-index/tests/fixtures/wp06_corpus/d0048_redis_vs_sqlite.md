title: Redis vs SQLite 选择
filename: redis_vs_sqlite_choice.md
tags: [redis, sqlite, database, choice, engineering]
updated_ns: 1704067200000000000

# Redis vs SQLite 选择

## 何时 SQLite

- 本地/嵌入式
- 关系型数据
- 强一致性
- 单机
- 读多写少
- 复杂查询

## 何时 Redis

- 缓存
- 队列
- Pub/Sub
- 计数器
- Session
- 实时分析

## 对比

| 维度 | SQLite | Redis |
|---|---|---|
| 类型 | 嵌入式关系型 | 内存 KV |
| 持久化 | 文件 | 可选 |
| 查询 | SQL | 简单命令 |
| 一致性 | 强 | 最终（默认） |
| 性能 | 百万行秒级 | 十万 op/s |
| 部署 | 零 | 服务 |

## PartiSync 选 SQLite

理由：
- 本地优先（同步无需服务器）
- 关系型（content/entry/tag/sidecar_items）
- 强一致性（多端同步最终一致，DB 局部一致）
- 嵌入式（无外部依赖）

## 用 Redis 的场景

- PartiSync Hub（服务端）缓存
- 实时在线设备列表
- 会话 / token 缓存

## 设计原则

- DB 走 SQLite（强一致）
- 缓存走 Redis（最终一致）
- 不要用 Redis 当主存

## 部署复杂度

- SQLite = 零（无服务）
- Redis = 单 binary + 配置文件 + 端口

## 故障恢复

- SQLite：文件备份
- Redis：AOF + RDB 双策略

## 迁移路径

从 SQLite 升级到 Postgres：
- 接口层抽象（sqlx 已抽象）
- 迁移工具：Litestream / LiteFS

## PartiSync 现状

- graph.db = SQLite（本地）
- tantivy index = 文件系统
- usearch = 文件系统
- 无 Redis（M3+ 可能引入）