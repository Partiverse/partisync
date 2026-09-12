# TASK-CARD: partisync T6-REPAIR（P0 修复阶段）

> 上游输入：`docs/evidence/audit-t6.md`（T6 审计，verdict=fail：1 blocker + 4 high）
> 本阶段只做 **P0**：解除门禁阻塞项，使 T7（L3 真实入口验收）具备可验收前提。P1/P2 另立阶段。

## 1. 任务基础信息

- **任务名称**: T6-REPAIR — P0 门禁解除（真实文件摄入 + 单机自包含编排）
- **关联**: `docs/SESSION.md`（v2 基线 §5 MCD 闭环、§9 进度）、`docs/decisions/ADR-001-mcd-architecture.md`
- **执行模式**: 智能模式（执行级硬门）
- **高风险标识**: **是**（新增文件落盘路径 → 路径遍历/符号链接/磁盘配额风险面；API 变更；compose 编排变更）
- **工具限制声明**: 本会话无 `agent_teams_*`；审计类 subagent 在本环境多次无产出失败（见 `docs/evidence/audit-t6.md` §7）→ 实现以主 Agent 为主，能独立委派的部分再委派

## 2. 模型档位（显式指定）

| 角色 | Provider / Model | 档位 | 依据 |
|---|---|---|---|
| Lead / 验收 | `vectide/glm-5.3` | strong | 跨模块改动 + 安全敏感（文件落盘） |
| Backend 实现 | `vectide/glm-5.3` | standard | 同一模型（当前 catalog 下 standard 与 strong 同模型） |
| Frontend 实现 | `vectide/glm-5.3` | standard | 同上 |
| T6′ 独立复审（下一会话） | `vectide/glm-5.3` | strong | 审计必须 strong，且要求独立上下文 |

fast 档（`minimax-cn/MiniMax-M2.7-highspeed`）仅可用于纯机械改动；本阶段无此类任务。

## 3. 范围（P0 三项）

1. **T6-06** compose healthcheck 假阴性修复：`wget http://127.0.0.1:7700/health`（或 `curl -f`）。
2. **T6-03** 单机自包含编排：新增 `server` 服务（多阶段 Dockerfile）+ `depends_on: service_healthy`。
3. **T6-01** 真实文件摄入最小闭环：
   - 后端 `POST /api/v1/assets/upload`（`multipart/form-data`）；
   - 文件名 `filepath.Base` + 扩展名白名单；服务端生成存储名（UUID）；最终落盘路径必须经 `filepath.Abs` 校验位于 `STORAGE_ROOT` 内；
   - 流式 SHA256（边写边算），同哈希**不重复落盘**（去重）；
   - 最小元数据提取：文件大小、MIME（嗅探）、图片宽高（png/jpeg/gif via `image.DecodeConfig`）；
   - 复用既有 `assets` 表（不新增迁移；`path` 字段记录存储相对路径）；
   - 前端：文件选择 + 上传 + 进度/结果反馈。

### 非目标（本阶段显式不做）

- ❌ 对象存储/WebDAV/SaaS connector；❌ 缩略图生成与文档文本抽取（仅记录 size/mime/图片尺寸）；
- ❌ 多文件并发批量的 UI 体验打磨；❌ 认证/RBAC（MCD 已声明边界）；
- ❌ P1（`scripts/benchmark-1m.sh` + 100 万实测）与 P2（T6-05 前端虚拟滚动、T6-07…T6-16）；
- ❌ 任何删除操作、生产部署、外部发布。

## 4. 验收（本阶段 MCD，可独立验收）

| 级别 | 验收方式 | 证据 |
|---|---|---|
| L1 | `go build ./... && go vet ./... && go test -race -count=1 ./...` 全绿；**新增** `internal/storage` 路径安全单测（`..`、绝对路径、符号链接、空名、超长名、非法扩展名）与 upload handler 测试 | 测试输出 + 用例清单 |
| L2 | `docker compose up -d --build` 后：`docker compose ps` 显示 meilisearch **healthy**、server **running**；用真实 `curl -F file=@…` 上传→`GET /api/v1/assets?q=` 命中→`GET /api/v1/assets/{id}` 返回含 size/mime 的记录；重复上传同文件返回 existing 且磁盘只有一份 | `docs/evidence/l2-integration-t6-repair.md` |
| L3 | 本阶段**不承诺**（T7 范围）：浏览器闭环 + 100 万压测留待 T7 | 如实标注 |

**门禁**: 本阶段完成后由 **T6′**（新会话，独立上下文，strong 档）复审 T6-01/03/06 的修复，verdict=pass 方可进入 T7。

## 5. 风险与回滚

- **路径遍历/符号链接**: 以 `STORAGE_ROOT` 为根，落盘前 `filepath.Abs` + 前缀校验；拒绝非白名单扩展名；不使用客户端文件名作为存储名。
- **磁盘耗尽/大文件**: 单文件大小上限（默认 100 MiB，可用 env 覆盖）+ 流式写入（不整文件进内存）。
- **去重竞态**: 先写临时文件算哈希 → 若哈希已存在则删除临时文件并返回既有资产；否则 `rename` 就位。
- **回滚**: 本阶段仅新增文件 + compose 追加服务；`git` 未初始化（仓库无 VCS），回滚 = 删除新增文件/还原被改文件（`docker-compose.yml` 变更前内容见本文档 §6）。
- **权限边界**: 不删除任何既有数据；不触碰容器卷内既有 50 万索引数据；不安装新插件。

## 6. docker-compose.yml 变更前状态（回滚参照）

```yaml
services:
  postgres:  # image postgres:16-alpine, ports "5432:5432", healthcheck pg_isready
  meilisearch:  # image getmeili/meilisearch:v1.8, ports "7700:7700",
                # healthcheck: wget -q -O - http://localhost:7700/health | grep -q 'available' || exit 1
volumes: postgres_data, meili_data
```

## 7. 交付物清单

- `Dockerfile`（多阶段，静态二进制 + alpine 运行层）
- `docker-compose.yml`（healthcheck 修复 + `server` 服务 + `asset_data` 卷 + `STORAGE_ROOT`）
- `internal/storage/storage.go` + `internal/storage/storage_test.go`
- `internal/api/handlers.go`（新增 `handleUploadAsset` + 路由）、`internal/api/handlers_test.go`（新增用例）
- `playground/src/lib/api.ts`（`uploadAsset`）、`playground/src/pages/assets.tsx`（文件选择 + 上传）
- `docs/evidence/l2-integration-t6-repair.md`（L2 证据）

## 8. 所用技能（加载 + 是否真应用）

| 技能 | 应用点 |
|---|---|
| `smart-mode-protocol` | 分诊、恢复预算、L1/L2/L3 证据分级（已注入并执行） |
| `defensive-patterns` | 文件 IO/生命周期/清理路径（tmp 文件、错误聚合、defer） |
| `testing-tiers` | 单测覆盖真实失败路径（路径逃逸、非法扩展名、超大文件）而非 happy path |
| `security-and-hardening` | 路径遍历/符号链接/大小上限/嗅探校验 |
| `minimal-evidence-checks` | 只跑与改动面匹配的检查集（build/vet/race + 目标包测试），不盲跑全量 |
