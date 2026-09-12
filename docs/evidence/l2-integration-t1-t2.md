# L2 集成验证报告 — T1 基础设施 + T2 Go 服务端骨架

- 日期: 2026-09-12
- 环境: docker compose（partisync-postgres 16-alpine healthy、partisync-meilisearch v1.8 healthy）+ 真实编译二进制 `/tmp/partisync-server`（go build ./cmd/server）
- 证据分级: **L2 已验证（真实容器 + 真实服务进程 + 真实 HTTP 入口）；L3（浏览器 UI 走通 + 百万 benchmark）未跑**

## 验证结果（全部通过）

| 步骤 | 入口 | 结果 |
|------|------|------|
| 健康检查 | GET /healthz | `{"status":"ok"}` |
| 资产登记 | POST /api/v1/assets（image + document 各一） | 201，UUID/时间戳正常，metadata JSONB 正确落库 |
| 去重 | 同 sha256 重复上传 | 200 `{"existing":true}`，ON CONFLICT 生效 |
| PG 持久化 | psql 查询 assets | 2 行，字段与 schema 一致 |
| Meilisearch 同步 | GET /indexes/assets/documents | 2 文档，写入异步任务成功 |
| 全文检索 | GET /api/v1/assets?q=img-a | Meili 命中 1 条（238µs 服务端耗时） |
| 无关键词列表 | GET /api/v1/assets | PG ListAssets 返回 2 条，created_at DESC |
| 慢标注提交 | POST /api/v1/assets/{id}/annotate | 202，任务落库 pending |
| 任务队列查询 | GET /api/v1/jobs?status=pending | 返回 1 条 pending |
| 出队 SQL | `SELECT ... FOR UPDATE SKIP LOCKED` 事务实测 | 锁定+回滚正常 |
| 输入校验 | 坏 UUID → 400；坏 sha256 → 400 | 通过 |

## 过程中发现并修复的问题

1. **PG_DSN 默认密码与 docker-compose 不一致**（默认 DSN 密码 `partisync` vs compose 的 `partisync_dev_password`）——首次启动失败 `pq: password authentication failed`。L2 用环境变量覆盖通过；**待修复**：main.go 默认 DSN 应与 compose 对齐（medium，审计 M 系列之外的补充项）。
2. **init.sql 未在容器首启时自动执行**（volume 已存在的旧库）——本环境手动 `-f` 执行验证通过；全新卷部署时 `docker-entrypoint-initdb.d` 会自动执行（首次初始化路径正确）。

## 遗留（来自审计 + 本次验证，T3 前处理）

- 审计 4 项 medium：size_bytes/长度校验、UUIDv7 兼容、processing 任务租约回收、Meili ctx 传播（见 `docs/evidence/audit-t1-t2.md`）
- main.go 默认 PG_DSN 密码与 docker-compose 对齐
- 带 q / 不带 q 的列表响应形状统一
