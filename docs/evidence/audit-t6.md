# T6 独立架构与安全审计报告 — partisync MCD（T1–T5 全量产出）

- **日期**: 2026-09-12
- **任务**: T6（独立架构与安全审查）；DAG：T5 → **T6** → T7
- **审计范围**: `internal/**`、`cmd/**`、`docker-compose.yml`、`deploy/postgres/init.sql`、`playground/**`、`scripts/**`、`docs/evidence/**`（T1–T5 证据）、`docs/decisions/ADR-001-mcd-architecture.md`
- **模型档位**: strong = `vectide/glm-5.3`（运行时校验：本会话即运行于该模型；无降档）
- **结论**: **verdict = fail**（1 blocker + 4 high，门禁不通过）

---

## 1. 审计方法与独立性声明

| 审计面 | 执行者 | 独立性 | 结论 |
|---|---|---|---|
| 安全面 | 独立审计员 A（fresh context，strong 档） | **独立** | `pass`（0 blocker/high，3 medium + 1 info） |
| 并发与健壮性面 | 主 Agent（strong 档） | **非独立**（审计员连续 2 次派发失败，见 §7） | 1 medium×4 + low×3，无 blocker/high |
| 证据与交付物真实性面 | 主 Agent（strong 档） | **非独立**（同上） | 1 blocker + 4 high |

**工具偏差声明**: 本会话工具集不提供 `agent_teams_*`（AgentTeams）；`workflow` 亦两次未产出。审计改用 `subagent` 并行委派，部分面由主 Agent 亲自完成。因此本报告**不是**完全独立的第三方审计：安全面有独立结论，其余两面为自查。**建议在 T7 之前补一次独立复审（T6′）**，覆盖 T6-01…T6-05 修复后的代码。

---

## 2. 一手环境事实（可直接复现）

| # | 命令 | 真实输出（节选） | 含义 |
|---|---|---|---|
| E1 | `docker compose config --services` | `postgres`<br>`meilisearch` | 编排中**无 `server` 服务** |
| E2 | `curl -H 'Authorization: Bearer …' localhost:7700/stats` | `"assets":{"numberOfDocuments":500000}`，`databaseSize` 804 MB | 索引实际 **50 万**，非 100 万 |
| E3 | `docker inspect partisync-meilisearch --format '{{json .State.Health}}'` | `"Status":"unhealthy","FailingStreak":3316`；Log: `wget: can't connect to remote host: Connection refused` | 健康检查持续失败 |
| E4 | `docker exec partisync-meilisearch wget -q -O - http://127.0.0.1:7700/health` → `{"status":"available"}`；`http://localhost:7700/health` 与 `http://[::1]:7700/health` → `Connection refused` | 见左 | 根因：容器内 `localhost` 解析出 `::1`，Meili 仅监听 IPv4 → **healthcheck 假阴性** |
| E5 | `docker inspect partisync-postgres` | `"Status":"healthy"` | PG 健康检查正常 |
| E6 | `go build ./... && go vet ./... && go test -race -count=1 ./...` | 全绿；仅 `internal/api` 有测试，其余包 `[no test files]` | 无竞态，但测试面极窄 |
| E7 | `ls scripts/` | 空 | `scripts/benchmark-1m.sh` 不存在 |
| E8 | grep `multipart\|FormFile\|io.Copy\|os.WriteFile\|thumbnail\|extract`（`*.go`） | 仅 6 处 `"storage backend unavailable"` 字符串 | **无任何文件字节摄入/落盘/元数据提取** |
| E9 | grep 前端 `FormData\|type="file"`、`virtual\|masonry\|debounce\|filter` | 无命中 | 前端无文件上传、无虚拟滚动、无防抖、无属性过滤 |
| E10 | `ls docs/` | 无 `shadcn-design-spec.html`（实际为 `shadcn-design-guidelines.md` + `ui-design-spec.html`） | `docs/SESSION.md:188` 引用失效 |
| E11 | `internal/store/store.go:29-31` | `SetMaxOpenConns(20)`、`SetMaxIdleConns(10)`、`SetConnMaxLifetime(30m)` | 连接池已配置，非耗尽风险 |
| E12 | `internal/api/handlers_test.go` | 11 个 `Test*`，断言状态码与响应体（真实断言） | handler 层有效覆盖；store/worker 零覆盖 |

---

## 3. Findings 汇总

| ID | 严重度 | 标题 | 影响 | 责任任务 |
|---|---|---|---|---|
| **T6-01** | **blocker** | 无真实文件摄入：MCD「上传/导入 → 落盘 → 元数据提取」整体缺失 | MCD 目标闭环 + 交付物第 2 项 | T2 / T4 |
| **T6-02** | **high** | `scripts/benchmark-1m.sh` 未交付 | 交付物第 4 项 | T5 |
| **T6-03** | **high** | docker-compose 缺 `server` 服务 | 交付物第 1 项 | T1 |
| **T6-04** | **high** | 100 万资产 p95<100ms 未证实（实测 50 万；线性外推不成立） | L3 验收口径 | T5 |
| **T6-05** | **high** | 前端 MCD 交付物未实现（虚拟滚动/瀑布流网格、多维属性过滤） | 交付物第 3 项 | T4 |
| T6-06 | medium | MeiliSearch healthcheck 假阴性；T1「全绿」不可复现 | T1 证据 | T1 |
| T6-07 | medium | 测试覆盖缺口：`store`/`search`/`worker`/`models` 零测试 | L1 口径 | T2/T3 |
| T6-08 | medium | 基础设施端口按 `0.0.0.0` 暴露（`5432`/`7700`）+ 默认凭据无生产熔断 | 安全基线 | T1 |
| T6-09 | medium | HTTP Server 未设 `MaxHeaderBytes`，无 CORS/Origin 策略 | 安全基线 | T2 |
| T6-10 | medium | 优雅停机未等待 Worker 退出即 `st.Close()` | 健壮性 | T3 |
| T6-11 | medium | 租约超时（5 min）对超时任务重复入队，无心跳 | 队列正确性 | T3 |
| T6-12 | medium | 双写一致性无对账/回填（Meili 失败仅告警仍返回 201） | 数据一致性 | T2 |
| T6-13 | low | `retry_count` 快照漂移（Worker 用出队快照判定上限，回收路径也会递增） | 队列语义 | T3 |
| T6-14 | low | `docs/SESSION.md:188` 引用不存在的文件 | 文档一致性 | 路线文档 |
| T6-15 | low | `internal/store/store.go:205` `_ = tx.Rollback()` 忽略回滚错误 | 错误处理 | T2 |
| T6-16 | low | `MockAnnotator.RNG`（`math/rand.Rand`）非并发安全（单 Worker 下未触发） | 潜在竞态 | T3 |
| T6-17 | info | 无认证/RBAC = ADR-001 与 SESSION.md 明确定界的 MCD 边界，**非缺陷** | — | — |
| T6-18 | info | 因无文件写路径，当前**不存在** Path Traversal/filepath 逃逸面 | — | — |

---

## 4. 关键 findings 详述

### T6-01 (blocker) 无真实文件摄入

- **证据**: `internal/api/handlers.go:86 handleCreateAsset` 仅 `decodeJSON` 元数据（name/path/sha256/size/mime/resource_type）；E8 全仓无文件字节处理、无存储适配器、无缩略图/文本抽取；E9 前端无 `type="file"`/`FormData`；`playground/src/lib/api.ts:57` 仅 `createAsset(body: object)`。
- **判定**: 任务卡 §3 交付物第 2 项要求「资产上传 API（单文件/批量上传，SHA256 去重与**存储落盘**）」与「元数据提取」；`docs/SESSION.md:91-101` 的 MCD 闭环首两步为「上传或导入图像/文档 → 提取基础元数据并建立索引」；`SESSION.md:107` 要求「至少一个真实文件摄入路径」。当前实现是**元数据登记**，`path` 为客户端自报字符串。
- **处置（二选一，需用户决策）**:
  1. **补交付（推荐）**: 新增 `POST /api/v1/assets/upload`（multipart），文件名 `filepath.Base` + 扩展名白名单 + 服务端生成存储名；落盘前 `filepath.Abs` 校验目标在 storage root 内（或 `os.OpenRoot`），流式 SHA256 去重；补最小元数据提取（size/mime/图片尺寸）；前端补文件选择与进度。
  2. **改口径**: 若有意把「摄入」降级为元数据登记（后续由 connector/桌面端承担文件），必须先改 `SESSION.md` §5 与任务卡 §3，使口径与实现一致。

### T6-04 (high) 100 万指标未证实

- **证据**: E2（500000 文档）；`benchmark-t5.md:22,95` 自述 500k 并以「p95 随文档数线性外推」论证；`SESSION.md:20,114` 口径为百万资产。
- **方法学复核（主 Agent）**: `cmd/bench/main.go:31` 默认 `--count=1_000_000`（工具支持 100 万，本次以 500000 运行）；`:112` 按 `reqNum%len(searchTerms)` 轮换 15 词（无单查询缓存假象）；`:340-352` `percentile` 为 nearest-rank，实现正确；`:113-115` 客户端端到端计时，口径可接受。
- **保留意见**: 15 词在 5,000 请求中重复约 333 次，直方图呈**双峰**（0.3–3.8ms 计 1,405 次 vs 42.6–46.2ms 计 2,470 次），提示缓存/热度影响显著；线性外推不成立（倒排检索通常次线性）。
- **建议**: 补齐 100 万实测（并修正 delete-queue 阻塞），查询集加入低频/长尾词；或经用户同意把口径正式改为 50 万并留痕。

### T6-05 (high) 前端交付物缺项

- **证据**: `playground/src/pages/assets.tsx`（302 行）为搜索 + 表格 + 详情模态框 + 2s 轮询（`:117 setInterval(poll, 2000)`）；E9 无 `virtual/masonry/debounce/filter` 命中；`vite.config.ts:46` proxy `/api → localhost:8080` 存在；`l2-integration-t4.md:92` 亦自述为「table」。
- **判定**: 任务卡 §3 前端交付物要求「资产库瀑布流 / 虚拟滚动网格」与「多维属性过滤」→ 未实现（拼写容错由 Meili 服务端默认提供，算部分满足）。
- **建议**: 要么实现虚拟滚动网格与 filter 参数链路（后端 Meili `filter` + 前端筛选器），要么按 §3 修订任务卡口径。

### T6-11 (medium) 租约超时导致重复执行

- **证据**: `internal/store/store.go:271-288` 对 `status='processing' AND updated_at < NOW() - lease` 直接 `retry_count+1` 并置回 `pending`；`internal/worker/worker.go:27` `LeaseTimeout=5m`，执行期间不刷新租约。
- **风险**: 真实 AI 标注 >5 min 会被误判为 worker 崩溃 → 重复执行、重复计费。
- **建议**: 执行期心跳（周期性 `updated_at = NOW()`）或按任务超时主动失败。

### T6-12 (medium) 双写无补偿

- **证据**: `internal/api/handlers.go:183-189` Meili upsert 失败仅 `log.Printf` + 返回 `warning`，仍 201；无重试队列/对账任务。
- **风险**: 「库里有、检索不到」的静默不一致，且无检测手段。
- **建议**: 落库后置 `index_state='pending'`，由后台任务重试同步并提供对账视图。

### T6-10 (medium) 优雅停机顺序

- **证据**: `cmd/server/main.go:72` `go w.Run(ctx)`；`:78-92` `cancel()` → `httpSrv.Shutdown()` → `st.Close()`，未等待 Worker 退出。
- **建议**: `sync.WaitGroup`/done channel 等待 Worker，再关连接池。

### T6-06 (medium) healthcheck 假阴性

- **证据**: E3+E4；`docker-compose.yml` healthcheck 用 `wget http://localhost:7700/health`。
- **建议**: 改 `http://127.0.0.1:7700/health`；修好后任何基于 `service_healthy` 的编排依赖（含 T6-03 的 `server`）才能成立。

---

## 5. 独立审计员 A（安全面）结论摘要

- `verdict = pass`，无 blocker/high。
- 一手证据：`go test -v -count=1 -race ./internal/api` 11/11 PASS；`docker inspect` 两容器健康状态；`curl -i http://localhost:8080/healthz` 连接拒绝（服务当时未常驻）。
- findings：F-SEC-01 medium（MaxHeaderBytes/CORS ＝ T6-09）、F-SEC-02 medium（0.0.0.0 暴露 ＝ T6-08）、F-SEC-03 low（默认凭据 ＝ T6-08）、F-SEC-04 info（＝ T6-17/T6-18）。
- 认可结论：SQL 全参数化（无字符串拼接）、Meili 查询经 JSON 结构体传输（无 filter 注入面）、请求体 `io.LimitReader(1MB)`、超时传播完整；T1/T2 四项 medium（M1 输入校验 / M2 UUIDv7-8 / M3 租约回收 / M4 ctx 传播）均已在代码中落地，主 Agent 独立复核一致。

---

## 6. 门禁判定

任务卡 §6：`verdict=pass` 且无 open blocker/high 方可交付。

- **fail**：1 blocker（T6-01）+ 4 high（T6-02/03/04/05）。
- 性质说明：5 项均属**交付物与验收口径缺口**，不是已交付代码中的可利用漏洞；已交付代码在安全面与竞态面无 blocker/high。
- **对 T7 的影响**: T7（L3 浏览器闭环 + 发布）在 T6 未修复前不具备可验收前提（浏览器闭环首步「上传图像/文档」在实现中不存在）。

### 建议修复顺序（P0 → P2）

| 优先级 | 项 | 说明 |
|---|---|---|
| P0 | T6-06 → T6-03 | 先修 healthcheck（否则 `service_healthy` 门控被假阴性阻塞），再补 `server` 服务 |
| P0 | T6-01 | 真实文件摄入最小闭环（multipart + 安全路径 + 流式 SHA256 + 落盘 + 最小元数据提取）+ 前端文件选择 |
| P1 | T6-04 + T6-02 | `scripts/benchmark-1m.sh` + 100 万实测，或经同意修订口径 |
| P1 | T6-05 | 前端虚拟滚动网格 + 属性过滤链路，或修订任务卡口径 |
| P2 | T6-07…T6-16 | 测试补齐（store/worker）、端口收敛、停机顺序、租约心跳、双写对账、文档修正 |

---

## 7. 恢复事件记录（L1/L2 预算）

| 事件 | 级别 | 情况 | 处置 |
|---|---|---|---|
| L1-1 | L1 | `workflow` 脚本解析失败（模板字面量内含反引号） | 改写脚本 |
| L1-2 | L1 | `workflow` 运行被取消，无产出 | 切换等价工具 `subagent` |
| L1-3 | L1 | 审计员 B 第 1 次失败（无产出） | 重派 |
| L1-4 | L1 | 审计员 C 第 1 次失败（无产出；因主 Agent 审计中途写报告触发其复现性质疑） | 重派并声明 `audit-t6.md` 非审计对象 |
| L1-5 | L1 | 审计员 B 第 2 次失败（无产出） | 见 L2-1 |
| L1-6 | L1 | 审计员 C 第 2 次失败（无产出） | 见 L2-2 |
| L2-1 | L2 | 并发面审计工具连续失败 | 由主 Agent 亲自完成该面，报告标注非独立 |
| L2-2 | L2 | 证据真实性面审计工具连续失败 | 同上 |

**经验教训（写入 Engramory 候选）**: subagent 连续失败时切换为主 Agent 自查是可行兜底；但审计任务应尽量保留至少一路独立结论；审计运行期间主 Agent 不应写仓库文件（会干扰审计员对可复现性的判断）。

---

## 8. 未验证项

1. 真实网络边界（反向代理后）的慢速攻击耐受性（本地无代理）。
2. 生产密钥管理（KMS/Vault）——MCD 范围外。
3. 100 万资产实测（见 T6-04）。
4. 浏览器真实入口闭环（L3）——属 T7 范围，且依赖 T6-01 修复。
5. 100 万 delete-queue 阻塞的根因（T5 遗留，未复现）。
