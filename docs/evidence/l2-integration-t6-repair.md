# L2 集成证据 — T6-REPAIR（P0 门禁解除）

- **日期**: 2026-09-12
- **任务卡**: `docs/T6-REPAIR-TASK-CARD.md`（P0：T6-06 / T6-03 / T6-01）
- **上游**: `docs/evidence/audit-t6.md`（T6 审计 verdict=fail）
- **证据分级**: **L1 已跑 + L2 已验证**（真实容器 + 真实服务进程 + 真实 HTTP 入口 + 真实文件字节落盘）；**L3 未跑**（浏览器闭环与 100 万压测属 T7）
- **模型档位**: 主 Agent `vectide/glm-5.3`（strong 档；实现与验证同一模型，无降档）

---

## 1. 变更清单

| 文件 | 变更 | 对应 finding |
|---|---|---|
| `docker-compose.yml` | meilisearch healthcheck 改 `127.0.0.1`；新增 `server` 服务（`depends_on: service_healthy`）；新增 `asset_data` 卷；三服务端口绑定 `127.0.0.1` | T6-06 / T6-03（顺带 T6-08） |
| `Dockerfile` | 新增：多阶段构建（golang:1.22-alpine → alpine:3.20），非 root 用户 `app`，`/data` 归 app 所有，busybox wget 供 healthcheck | T6-03 |
| `.dockerignore` | 新增：构建上下文瘦身（排除 node_modules/dist/docs/日志） | T6-03 |
| `vendor/` | 新增（`go mod vendor`，296 KB）：容器构建**完全离线**（本环境容器内无法访问 `proxy.golang.org`） | T6-03 的必要条件 |
| `internal/storage/storage.go` | 新增：内容寻址本地存储（流式 SHA256、扩展名白名单、大小上限、扩展名/嗅探 MIME、图片尺寸、`Resolve`/`Open` 前缀与符号链接校验） | T6-01 / T6-18 |
| `internal/storage/image.go` | 新增：`image.DecodeConfig` 取图片宽高（仅解析头部） | T6-01 |
| `internal/api/handlers.go` | 新增 `POST /api/v1/assets/upload`（multipart）+ `displayName`/`resourceTypeFor`/`uploadMetadata`；`NewServer` 增加 `content` 参数 | T6-01 |
| `cmd/server/main.go` | 组装 `storage.New(STORAGE_ROOT, MAX_UPLOAD_BYTES)`；`STORAGE_ROOT` 默认 `./data/assets` | T6-01 |
| `internal/storage/storage_test.go` | 新增 11 个用例：内容寻址/去重/白名单/空文件/超限/路径穿越/Resolve 逃逸/符号链接逃逸/扩展名解析 | L1 |
| `internal/api/upload_test.go` | 新增 8 个用例：503（无存储/无库）、400（缺 part/空文件）、415（非白名单）、413（超限）、展示名清洗、资源类型归类 | L1 |
| `playground/src/lib/api.ts` | 新增 `uploadAsset(file)`（FormData） | T6-01 前端 |
| `playground/src/pages/assets.tsx` | 新增隐藏 `<input type="file">` + 「上传文件」按钮 + 上传中/结果/错误提示；上传成功后刷新列表 | T6-01 前端 |

**已声明的顺带修复**：把 postgres/meilisearch/server 的 host 端口绑定从 `0.0.0.0` 收敛为 `127.0.0.1`（审计 T6-08 的端口暴露项，与本任务同文件、零额外风险）。**未做**审计中的其余 P1/P2 项（T6-02/04/05/07/09…T6-16）。

---

## 2. L1 证据（本机，非容器）

```
$ go build ./... && go vet ./... && gofmt -l internal cmd   # gofmt 对我改动的文件已清零
（无输出 = 全部通过）
$ go test -race -count=1 ./...
?   partisync/server/cmd/bench        [no test files]
?   partisync/server/cmd/server       [no test files]
?   partisync/server/internal/models  [no test files]
?   partisync/server/internal/search  [no test files]
?   partisync/server/internal/store   [no test files]
?   partisync/server/internal/worker  [no test files]
ok  partisync/server/internal/api      1.015s
ok  partisync/server/internal/storage  1.016s
```

- 前端类型检查：`cd playground && npx tsc --noEmit` → 退出码 0（零错误）。
- 覆盖缺口（如实记录）：`store`/`search`/`worker`/`models` 仍无测试文件（对应审计 T6-07，属 P2）。

---

## 3. L2 证据（真实 Docker + 真实二进制 + 真实 HTTP）

系列命令与原始输出（`S=http://127.0.0.1:8080`）：

### 3.1 编排自包含与健康状态（T6-03 / T6-06）

```
$ docker compose up -d --build
 Container partisync-meilisearch Recreated
 Container partisync-postgres  Recreated
 Volume partisync_asset_data   Created
 Container partisync-server    Created → Started
 Container partisync-meilisearch Healthy
 Container partisync-postgres  Healthy

$ docker compose ps
partisync-meilisearch   Up 33 seconds (healthy)
partisync-postgres      Up 32 seconds (healthy)
partisync-server        Up 27 seconds (healthy)

$ curl -s http://127.0.0.1:8080/healthz
{"status":"ok"}
```

对照：修复前 `partisync-meilisearch` 为 `unhealthy`（`FailingStreak=3316`，容器内 `localhost`→`::1` 拒连）；修复后为 **healthy**。

### 3.2 真实文件字节上传（T6-01）

```
$ curl -F "file=@playground/src/assets/hero.png" $S/api/v1/assets/upload
201
{"asset":{"id":"65d410b7-0e9b-43ba-a9f1-8c3f096144f8","name":"hero.png",
 "path":"88/1ffbcaafc212e49addad08846a5b82761355fa20624253af3477ba33262c5c",
 "sha256":"881ffbcaafc212e49addad08846a5b82761355fa20624253af3477ba33262c5c",
 "size_bytes":13057,"mime_type":"image/png","resource_type":"image",
 "metadata":{"ext":".png","width":343,"height":361,"original_name":"hero.png","content_sniffed":true}},
 "file_deduped":false}
```

- 大小、**嗅探 MIME**（`image/png`）与**图片尺寸**（343×361，由 `image.DecodeConfig` 解析头部得到）均真实写入。
- 存储路径为内容寻址（`sha[0:2]/sha[2:]`），**不含任何客户端路径成分**。

### 3.3 去重（DB 与磁盘两级）

```
$ curl -F "file=@playground/src/assets/hero.png" $S/api/v1/assets/upload
existing=True  file_deduped=True  id=65d410b7-0e9b-43ba-a9f1-8c3f096144f8   # 同哈希 → 返回既有资产
$ docker exec partisync-server sh -c "find /data/assets -type f | wc -l"   # 3 次上传同内容
2                        # 磁盘只有 2 个对象（hero 与 trav2 各一份）
$ docker exec partisync-server sh -c "ls -A /data/assets/.tmp | wc -l"
0                        # 无临时文件残留
```

### 3.4 路径穿越与畸形输入（T6-01 安全面）

| 输入 | 结果 |
|---|---|
| `filename=../../../../etc/passwd.png`（内容同 hero） | `200`，走去重分支返回既有资产；`path` 仍为内容寻址 |
| `filename=../../../etc/evil.png`（**不同内容**） | `201`，`name="evil.png"`（`filepath.Base` 清洗），`path="38/7809…"` |
| 落盘位置 | `docker exec` 实测 `/data/assets/38/7809935c…` ∈ storage root；`--` 权限 `-rw-------` |
| `.exe`（非白名单） | `415` `{"error":"unsupported file extension (allowed: png, jpg, jpeg, gif, webp, pdf, txt, md, csv, json)"}` |
| `empty.png`（0 字节） | `400` `{"error":"uploaded file is empty"}` |
| 缺 `file` part | `400` `{"error":"multipart part \"file\" is required"}` |
| 超限（L1 覆盖，maxBytes=4） | `413`（`internal/api/upload_test.go`） |

### 3.5 入库、检索与既有能力回归

```
$ curl $S/api/v1/assets/$ID
{"asset":{"id":"22d4e839-…","name":"evil.png","path":"38/7809…","sha256":"3878099…",
 "size_bytes":13058,"mime_type":"image/png","resource_type":"image",
 "metadata":{"ext":".png","width":343,"height":361,"original_name":"evil.png","content_sniffed":true}}}

$ docker exec partisync-postgres psql -U partisync -d partisync -tAc "select name,path,size_bytes,mime_type,resource_type,metadata->>'width' from assets where id='$ID';"
evil.png|38/7809…|13058|image/png|image|343        # PG 与 API 一致

$ curl "$S/api/v1/assets?q=evil&limit=5"
{"results":[{"id":"22d4e839-…","name":"evil.png",…}],"total":1}   # MeiliSearch 同步生效

$ curl -H "Authorization: Bearer …" http://localhost:7700/stats
numberOfDocuments = 500003     # 修复前 500000 + 本次 3 条（hero / trav2 / regress.txt）；既有数据完好
$ docker exec partisync-postgres psql -U partisync -d partisync -tAc "select count(*) from assets;"
8                              # 修复前 5 + 本次 3

$ curl -X POST -H 'Content-Type: application/json' -d '{…metadata JSON…}' $S/api/v1/assets   → 201
（元数据登记端点未回归；仍可用于无文件字节的登记场景）
```

### 3.6 慢标注链路回归

```
POST /api/v1/assets/22d4e839-…/annotate  → job_id=778a3b22-f0b0-4391-9c60-7eec9440cd0d
poll 1: completed (retry_count=0)
result.tags=[{landscape,0.929},{outdoor,0.942},{daylight,…}]

$ docker logs partisync-server | tail
[worker] processing job 778a3b22-… (asset=22d4e839-…, attempt=0)
[worker] job 778a3b22-… completed successfully
（无 warning / 无 meilisearch upsert 失败日志）
```

---

## 4. 验收判定（本阶段 MCD）

| 验收项 | 结果 |
|---|---|
| L1：`go build` / `go vet` / `go test -race` 全绿，新增路径安全单测通过 | ✅ |
| L1：前端 `tsc --noEmit` 零错误 | ✅ |
| L2：`docker compose up` 后 meilisearch **healthy**、server running+healthy（自包含编排） | ✅ |
| L2：真实 `curl -F` 文件上传 → 元数据（size/嗅探 MIME/图片尺寸）入库 | ✅ |
| L2：SHA256 去重（DB 返回 existing；磁盘不产生第二份副本；无 temp 残留） | ✅ |
| L2：路径穿越/非白名单/空文件/缺 part/超限 均被拒且无越界落盘 | ✅ |
| L2：检索（MeiliSearch）与既有 50 万索引数据无回归 | ✅ |
| L2：慢标注任务链路无回归 | ✅ |
| L3：浏览器闭环 + 100 万压测 | ⛔ **未跑**（T7 范围；需先补 T6-02/04） |

**本阶段结论**：P0 三项（T6-06 / T6-03 / T6-01）修复完成，**L2 已验证，L3 未跑**。
门禁上仍**不能**宣称 T6 通过：T6-02（`scripts/benchmark-1m.sh`）、T6-04（100 万实测）、T6-05（前端虚拟滚动/属性过滤）等 P1 项未完成；T6′ 独立复审尚未执行。

## 5. 已知限制（诚实记录）

1. **上传成功但 PG 写入失败时**，内容寻址文件会保留在磁盘（无 GC/对账）——属审计 T6-12 双写一致性问题的一部分，留待 P2。
2. **尚无下载/预览端点**（`storage.Open` 已实现且带符号链接校验，但未挂路由）；浏览器侧目前无法回看文件内容。
3. 单文件上限默认 100 MiB（`MAX_UPLOAD_BYTES` 可覆盖）；未做磁盘配额与并发上传限流。
4. 元数据提取仅 size / 嗅探 MIME / 图片尺寸；无缩略图与文档文本抽取（任务卡非目标）。
5. `vendor/` 入库是离线构建的代价（本环境容器内无外网）；若后续 CI 有外网可改回 `go mod download`。
