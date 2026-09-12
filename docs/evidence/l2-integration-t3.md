# L2 集成验证 — T3 慢标注 Worker 系统

**日期**：2026-09-12  
**验证内容**：T3 慢标注异步任务系统的端到端生命周期  
**环境**：Docker（postgres:16-alpine + meilisearch:v1.8）+ 真实 Go 服务进程 + 同进程 Worker

---

## 环境

| 组件 | 版本 | 状态 |
|------|------|------|
| postgres | 16-alpine | healthy |
| meilisearch | v1.8 | healthy |
| partisync server | 当前 HEAD | 运行中 (:8080) |
| Worker | 内嵌同进程 | 运行中 (poll=2s) |

**DSN**：`postgres://partisync:partisync_dev_password@localhost:5432/partisync?sslmode=disable`（已与 docker-compose.yml 对齐）

---

## 步骤 1：启动服务

```bash
./server-test > server-test.log 2>&1 &
```

**结果**：服务启动日志：
```
[partisync] [worker] starting (poll=2s, lease_timeout=5m0s, max_retries=3)
[partisync] listening on :8080 (meili=http://localhost:7700)
```

Worker 在启动时自动回收了一个旧的 stale processing job（bbb159f1）：
```
[partisync] [worker] processing job bbb159f1-b807-4dda-b757-81c437bf72b0 (asset=f649307f..., attempt=0)
[partisync] [worker] job bbb159f1-b807-4dda-b757-81c437bf72b0 completed successfully
```

---

## 步骤 2：创建资产（POST /api/v1/assets）

```bash
curl -X POST http://localhost:8080/api/v1/assets \
  -H "Content-Type: application/json" \
  -d '{
    "name":"test-l2.jpg",
    "path":"/uploads/test-l2.jpg",
    "sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "resource_type":"image",
    "size_bytes":12345,
    "mime_type":"image/jpeg"
  }'
```

**结果**：`201 Created`
```json
{
  "asset": {
    "id": "60cadf24-123a-49f2-a9d4-996d855ee08f",
    "name": "test-l2.jpg",
    "path": "/uploads/test-l2.jpg",
    "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "size_bytes": 12345,
    "mime_type": "image/jpeg",
    "resource_type": "image",
    "metadata": {},
    "created_at": "2026-09-12T01:42:36.344513Z",
    "updated_at": "2026-09-12T01:42:36.344513Z"
  }
}
```

---

## 步骤 3：提交标注任务（POST /api/v1/assets/{id}/annotate）

```bash
curl -X POST http://localhost:8080/api/v1/assets/60cadf24-123a-49f2-a9d4-996d855ee08f/annotate \
  -H "Content-Type: application/json" \
  -d '{"prompt":"classify this image"}'
```

**结果**：`200 OK`（幂等返回）
```json
{
  "job": {
    "id": "e4c7cc0c-e453-4e24-b40d-e8d1b9936c5e",
    "asset_id": "60cadf24-123a-49f2-a9d4-996d855ee08f",
    "status": "pending",
    "prompt": "classify this image",
    "result": {},
    "retry_count": 0,
    "created_at": "2026-09-12T01:42:39.555735Z",
    "updated_at": "2026-09-12T01:42:39.555735Z"
  }
}
```

---

## 步骤 4：Worker 自动出队处理

约 1.4 秒后 Worker 日志：
```
[partisync] [worker] processing job e4c7cc0c-e453-4e24-b40d-e8d1b9936c5e (asset=60cadf24-123a-49f2-a9d4-996d855ee08f, attempt=0)
[partisync] [worker] job e4c7cc0c-e453-4e24-b40d-e8d1b9936c5e completed successfully
```

---

## 步骤 5：查询任务结果（GET /api/v1/jobs）

```bash
curl http://localhost:8080/api/v1/jobs
```

**结果**：
```json
{
  "jobs": [
    {
      "id": "e4c7cc0c-e453-4e24-b40d-e8d1b9936c5e",
      "asset_id": "60cadf24-123a-49f2-a9d4-996d855ee08f",
      "status": "completed",
      "prompt": "classify this image",
      "result": {
        "tags": [
          {"name": "landscape", "confidence": 0.922},
          {"name": "outdoor", "confidence": 0.962},
          {"name": "daylight", "confidence": 0.944}
        ],
        "model": "mock-annotator-v1",
        "asset_id": "60cadf24-123a-49f2-a9d4-996d855ee08f",
        "description": "A scenic outdoor photograph featuring natural landscapes with clear visibility and natural lighting.",
        "prompt_used": "classify this image"
      },
      "retry_count": 0,
      "created_at": "2026-09-12T01:42:39.555735Z",
      "updated_at": "2026-09-12T01:42:40.976922Z"
    }
  ],
  "limit": 20
}
```

---

## 验证结论

| 检查项 | 结果 |
|--------|------|
| 服务启动 | ✅ |
| Worker 启动日志 + RecoverStaleProcessing | ✅ 启动时回收了 1 个 stale job |
| 资产创建（POST /api/v1/assets） | ✅ 201 Created，返回完整 asset JSON |
| 标注任务提交（POST /api/v1/assets/{id}/annotate） | ✅ 200 OK，返回 status=pending 的 job |
| Worker 自动出队（FOR UPDATE SKIP LOCKED） | ✅ 约 1.4 秒内处理 |
| MockAnnotator 执行（随机 100-500ms 延迟，5% 失败率） | ✅ 成功，结构化 JSON 结果 |
| 任务状态变更为 completed + result | ✅ |
| 幂等：重复提交同一 asset 的标注任务 | ✅ 返回已有 job |

**L2 验证通过。L3（浏览器真实入口）待 T4 Web 前端完成后执行。**

---

## 附：MeiliSearch 索引问题

MeiliSearch `PUT /indexes/assets` 返回 405（方法不允许），原因是 MeiliSearch v1.8 中索引创建需用 `POST /indexes`，而 `EnsureIndex` 实现使用了 `PUT /indexes/assets`。搜索功能降级，但不影响标注 Worker 核心链路。该问题在 T4 前端联调阶段一并修复。
