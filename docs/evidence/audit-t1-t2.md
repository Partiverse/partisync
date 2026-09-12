# 审计报告 — T1 基础设施 + T2 Go 服务端骨架

- 审计角色: Audit-Reviewer（独立子代理，档位 strong -> vectide/glm-5.3）
- 日期: 2026-09-12
- 验证证据: `go build ./... && go vet ./... && go test ./...` 三绿（含 -race）

## 结论

**verdict: pass** — 无 blocker/high findings。

## Findings 摘要

### medium（4 项，建议 T3 worker 前修复）

| # | 位置 | 问题 |
|---|------|------|
| M1 | internal/api/handlers.go:96-120 | `size_bytes` 未校验负数/超大值；`name/path/resource_type/prompt` 无长度上限，超长输入在 PG 层报 500 而非 400 |
| M2 | internal/api/handlers.go:60-63 | UUID 正则仅接受 v1–v5，合法 UUIDv7/v8 资产 ID 会被 400 拒绝（兼容性地雷） |
| M3 | internal/store/store.go:200-246 | worker 崩溃后任务永久滞留 `processing`，无租约/超时回收（T3 前必须补） |
| M4 | internal/search/meili.go:50,63,80 | Meili 客户端方法不接收 `context.Context`，请求取消无法传播 |

### low / info（摘要）

- `metadata` 允许任意 JSON 标量入库 jsonb；1MB 请求体截断应返回 413 而非 400
- 连接池未设 `SetConnMaxLifetime`；`log.Fatal` 跳过 defer Close
- 带 q / 不带 q 的列表响应形状不一致（map vs Asset 结构）
- docker-compose 密码硬编码（dev 可接受，严禁进入生产；建议移 .env）
- init.sql 中 `uuid-ossp` 扩展未被使用可删
- store 层无 DB 集成测试（T2 验收为 L1 级别）

## 通过项

- SQL 全参数化，无注入面；无文件系统操作
- `DequeuePendingJob` 事务 + `FOR UPDATE SKIP LOCKED` 语义正确，rollback 路径干净
- 请求路径无 panic 面；nil 依赖 503 守卫有测试覆盖
- 错误响应不泄漏内部信息；HTTP 超时齐备；rows.Close/rows.Err 规范
- Schema 与 Go 结构体逐列一致；测试为真实校验分支非空转
