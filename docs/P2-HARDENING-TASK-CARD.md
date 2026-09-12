# TASK-CARD: partisync P2 加固批收尾（A2-01…A2-06 + 工程卫生）

> 上游输入：`docs/evidence/audit-t6-prime.md` §3（A2-01…A2-09 安全面 findings）与 §12 残留项。
> 前置：T6′/T6″ 修复已 pass（C2-1…C2-6/C2-8 关闭）、C2-5 已修、T7 L3 浏览器闭环 8/8 通过（`docs/evidence/l3-mcd-acceptance.md`）。
> 本阶段目标：把 A2 安全批**全部关闭**（含未提交的在途改动 A2-02/04/05/06 + 未做的 A2-01/A2-03），并产出可复现 L2 证据。

## 1. 任务基础信息

- **任务名称**: P2 加固批收尾 — 上传链路资源与类型安全 + 就绪探针 + 工程卫生
- **执行模式**: 智能模式（执行级硬门）
- **ADAC 风险分级**: **C**（关键路径：上传摄入、资源生命周期/临时文件；含安全面子项，安全子项按 D 的口径要求证据与门禁）
- **高风险标识**: 中（改动落在唯一的真实文件摄入路径上，必须零功能回归）

## 2. 范围

| ID | 项 | 来源 | 本阶段处置 |
|---|---|---|---|
| A2-01 | multipart 大文件先落容器 `/tmp`，绕开资产卷配额 | 安全 medium | 改为流式解析（`MultipartReader`），文件部分直接流进 `storage.Save`，不再经 `ParseMultipartForm` 落盘 |
| A2-02 | `/healthz` 是纯 liveness，依赖故障无观测 | 安全 medium | 保持 `/healthz` 为纯 liveness；新增 `GET /readyz`（PG/Meili/存储逐项 + 503） |
| A2-03 | 请求体上限硬编码，`MAX_UPLOAD_BYTES` 调小不生效 | 安全 low | 上限改为 `store.MaxBytes() + multipart 余量` |
| A2-04 | 白名单只约束后缀，预览按 DB mime 内联返回 | 安全 low | 预览前做内容-声明一致性校验，不一致 415；补 `X-Content-Type-Options` + CSP `sandbox` |
| A2-05 | 落盘成功但入库/回读失败留孤儿文件 | 安全 low | 失败路径 `storage.Discard` 清理（去重命中对象不删） |
| A2-06 | `ReadTimeout=15s` 导致慢速大文件上传 400 | 安全 low | `ReadHeaderTimeout=15s` + `ReadTimeout=5min` 分离 |
| — | **A2-04 误拒修复（本轮新发现）** | 本轮 | 现存实现按逐字相等比较，`.md`/`.csv`/`.json` 被嗅探为 `text/plain` 会误拒 415；改为按"文本家族"收敛判定 |
| — | 工程卫生 | §12 残留 | `gofmt` 修 `cmd/bench/main.go`、`internal/worker/worker.go` |

## 3. 非目标（本阶段不做）

- C2-7（MIME 过滤 UI）、C2-9（排序 UI）——属新增 UI 面，未获批不动；
- B-01/B-02/B-06（证据文档口径修正，含 8080 产品链路基准）——独立小任务；
- T6-05 虚拟滚动/懒加载网格；A2-07…A2-09（info 级）；认证/RBAC；
- 不触碰生产/凭据/外部发布；不删既有数据与容器卷内容。

## 4. 变更的接口与语义

- **新增** `GET /readyz`（additive，无破坏性）：200 `{ready:true,dependencies:[...]}`；任一依赖不可达 → 503。
- `POST /api/v1/assets/upload`：请求体上限语义由"硬编码 100MiB+1MiB"改为"跟随 `MAX_UPLOAD_BYTES`（默认值不变）"；**不再产生容器 `/tmp` 落盘**。
- `GET /api/v1/assets/{id}/preview`：新增 415（内容与声明 MIME 不一致）——**有意的行为收紧**。
- `cmd/server`：`ReadTimeout` 15s → 5min（`ReadHeaderTimeout` 承担防慢速攻击）。

**回滚**：单提交 `git revert`；镜像可回退（重跑 `docker compose build server`）；无数据迁移，无卷内容变更。

## 5. Acceptance Matrix

| ID | 类目 | 可验收准则（不变式） | 验证方法 | 证据 | 状态 |
|---|---|---|---|---|---|
| AC-01 | security | 声明 `image/png` 而内容是 HTML/JS 的对象，预览返回 415；真实 PNG/PDF 预览 200 | L1 单测（`MatchedContentType`）+ L2 curl 实测 | 单测输出、curl 状态码 | **pass** |
| AC-02 | security | `.md`/`.csv`/`.json`/`.txt` 资产预览不被误拒（200） | L2 curl 实测 | curl 状态码 + 字节数 | **pass** |
| AC-03 | resource | 上传大文件时容器 `/tmp` 零增长；请求 body 全程流式（无 `ParseMultipartForm`） | L2 `docker ps --size` 前后对比 + `ls /tmp` + 反向实验 | 可写层 delta=0 bytes、/tmp 条目 0（规模取 12MiB，超旧 8MiB 落盘阈值） | **pass** |
| AC-04 | config | `MAX_UPLOAD_BYTES=1MiB` 时上传 2MiB → 413；512KiB → 201 | L2 一次性容器覆盖 env 实测 | curl 状态码 | **pass** |
| AC-05 | lifecycle | 入库失败（PG 停机）时落盘文件被清理，资产卷内无该 sha 对象 | L2 停机 postgres 实测 + 容器内 `test -e` 核对 | 对象路径不存在（单测覆盖幂等与越界拒绝；"去重对象不删"由代码分支 + 单测覆盖） | **pass** |
| AC-06 | observability | `/readyz` 三依赖可达 → 200；Meili/PG 停机 → 503 且故障项 `ok:false`；`/healthz` 始终 200 | L2 停机/恢复实测 | curl JSON | **pass** |
| AC-07 | regression | 慢速上传（限速，用时 > 服务端 15s/30s 常规超时）成功 201，不再 400 | L2 限速上传实测 | 4MiB @100KB/s = 40.97s → 201 | **pass** |
| AC-08 | regression | T7 L3 浏览器闭环复跑 8/8 PASS，consoleErrors=0，badResponses=0 | `scripts/l3-mcd-acceptance.mjs`（静态托管 :8080） | `docs/verification/p2-hardening-run.json` + 截图 | **pass** |
| AC-09 | hygiene | `gofmt -l cmd internal` 为空；`go build/vet/test -race` 全绿 | L1 | 命令输出 | **pass** |
| AC-10 | evidence | 证据文档 `docs/evidence/l2-integration-p2-hardening.md` + SESSION.md 状态更新 + git 提交 | 人工核对 | 文档 + commit | **pass** |
| AC-11 | gate | 独立复审（fresh-context 审计员）复核本批；未执行则显式声明非独立 | 派发审计员 | 审计报告 | **pending（未执行，已在证据文档显式声明）** |

## 6. User Verification Scenarios

| ID | 关联 | 场景 | 步骤 | 期望可见结果 |
|---|---|---|---|---|
| UV-01 | AC-01/AC-02 | 伪装图片 vs 合法文档 | 用元数据端点把一段 HTML 字节登记为 `image/png` 资产，并在 `notes.md` 的孪生资产之间对比详情预览 | 前者预览区不渲染（服务端 415，前端 `img onError` 隐藏）；后者正常显示内容 |
| UV-02 | AC-04 | 上传上限可配 | 以 `MAX_UPLOAD_BYTES=1048576` 起服务，上传 2MiB PNG | 提示「文件超过上限」（413） |
| UV-03 | AC-06 | 就绪探针 | 依次 `docker stop partisync-meilisearch` / `partisync-postgres`，`curl /readyz` 与 `/healthz` | `/readyz` = 503 且列出故障依赖；`/healthz` 仍 200 |
| UV-04 | AC-08 | 浏览器闭环无回归 | 跑 `node scripts/l3-mcd-acceptance.mjs` | 8/8 PASS，截图落 `docs/verification/t7-l3/` |

## 7. 门禁

- **L1**：`go build ./... && go vet ./... && go test -race -count=1 ./...` 全绿（`GOCACHE` 显式指定，避免沙箱假绿）；`gofmt -l` 为空。
- **L2**：AC-01…AC-07 全部实测通过，证据落 `docs/evidence/l2-integration-p2-hardening.md`。
- **L3 回归**：AC-08 复跑 T7 脚本 8/8。
- **独立复审**：AC-11 由 fresh-context 审计员执行；**本轮实施者自证不构成独立复审**，未执行时门禁状态显式标注"未复审"。
