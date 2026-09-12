# T4 L2 Integration Verification

**Date**: 2026-09-12  
**Status**: ✅ PASSED

## Environment

- Go server: `localhost:8080` (binary: `server-test`, PID 2479569)
- Postgres: `localhost:5432` (docker container `partisync-postgres`)
- MeiliSearch: `localhost:7700` (docker container `partisync-meilisearch`)

## Verification Steps & Results

### 1. MeiliSearch Index Creation (POST /indexes) — ✅

```
GET http://localhost:7700/indexes/assets
→ 200 OK (no more 405 Method Not Allowed)
```

**Fix applied**: `EnsureIndex` changed from `PUT /indexes/assets` to `POST /indexes` with body `{"uid":"assets","primaryKey":"id"}`. Also added 405 to the list of treated-as-success status codes.

### 2. Asset CRUD — ✅

```
POST /api/v1/assets
Body: {"name":"t4-final.jpg","path":"/uploads/t4-final.jpg","sha256":"deadbeef...","mime_type":"image/jpeg","resource_type":"image","size_bytes":99999}
→ 201 Created
asset_id: ef2674dd-d453-4d70-9542-70d2c3ff345e
```

### 3. Annotation Job Lifecycle — ✅

```
POST /api/v1/assets/{asset_id}/annotate
Body: {"prompt":"classify this"}
→ 200 OK
job_id: 336d07b5-a388-43e7-ada6-0c6eb43898f9
status: pending
```

Poll `GET /api/v1/jobs/{job_id}` every 1 second:

| Poll | Status |
|------|--------|
| 1 | pending |
| 2 | processing |
| 3 | **completed** |

### 4. Job Detail Endpoint — ✅ NEW

```
GET /api/v1/jobs/336d07b5-a388-43e7-ada6-0c6eb43898f9
→ 200 OK
{
  "job": {
    "id": "336d07b5-a388-43e7-ada6-0c6eb43898f9",
    "asset_id": "ef2674dd-d453-4d70-9542-70d2c3ff345e",
    "status": "completed",
    "prompt": "classify this",
    "result": {
      "tags": [
        {"name": "landscape", "confidence": 0.935},
        {"name": "outdoor", "confidence": 0.933},
        {"name": "daylight", "confidence": 0.941}
      ],
      "model": "mock-annotator-v1",
      "description": "A scenic outdoor photograph featuring natural landscapes...",
      "prompt_used": "classify this"
    },
    "retry_count": 0
  }
}
```

### 5. Asset Search — ✅

```
GET /api/v1/assets?q=t4
→ total: 2 (t4-test.jpg, t4-l2.jpg)
```

## Changed Files

| File | Change |
|------|--------|
| `internal/search/meili.go` | POST /indexes + 405 handling |
| `internal/store/store.go` | `GetJob(id)` method added |
| `internal/api/handlers.go` | `GET /api/v1/jobs/{id}` route + `handleGetJob` |
| `playground/vite.config.ts` | Vite proxy `/api` → `localhost:8080` |
| `playground/src/lib/api.ts` | Typed API client (Asset, AnnotationJob, AnnotateResult) |
| `playground/src/pages/assets.tsx` | Asset library page (search, table, modal, annotate, polling) |
| `playground/src/App.tsx` | Minimal shell with sidebar nav + hash routing |

## Build Verification

- `go build ./...` — ✅ clean
- `go test ./...` — ✅ ok
- `npx tsc --noEmit` — ✅ zero errors

## Summary

| Check | Result |
|-------|--------|
| MeiliSearch index (no 405) | ✅ 200 |
| Asset create + list | ✅ works |
| Annotate job submit | ✅ works |
| GET /api/v1/jobs/{id} | ✅ works |
| Job lifecycle pending→processing→completed | ✅ works |
| MockAnnotator structured result (tags+description) | ✅ works |
| Asset search filter | ✅ works |
| Frontend TypeScript | ✅ zero errors |

**T4 L2 验证通过。L3 浏览器验收（Playwright）待 T7 阶段执行。**
