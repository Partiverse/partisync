# P2 加固批 L2 证据 — A2-01…A2-06（上传链路资源/类型安全 + 就绪探针 + 超时）

- **日期**: 2026-09-13
- **执行者**: 主 Agent（strong 档）——**实施者自证，不构成独立复审**
- **任务卡**: `docs/P2-HARDENING-TASK-CARD.md`
- **ADAC 风险分级**: **C**（关键路径：真实文件摄入 / 资源生命周期 / 临时文件；含安全面子项）
- **上游输入**: `docs/evidence/audit-t6-prime.md` §3（A2-01…A2-09）、§12 残留项
- **结论**: **L1 全绿 / L2 53 项断言全通过 / L3 回归 8/8 PASS**；**独立复审（AC-11）未执行**，本批不得据此判定门禁放行。

---

## 1. 变更清单（finding → 机制）

| finding | 严重度 | 实现 | 文件 |
|---|---|---|---|
| A2-01 multipart 大文件先落容器 `/tmp`，绕开资产卷配额 | medium | 弃用 `ParseMultipartForm`，改 `r.MultipartReader()` 流式取 `file` 部分，直接喂给 `storage.Save`（写进资产卷内 `.tmp`→内容寻址终路径）；其余字段读尽丢弃 | `internal/api/handlers.go` |
| A2-02 `/healthz` 是纯 liveness，依赖故障无观测 | medium | 新增 `GET /readyz`：逐项探测 postgres / meilisearch / storage，任一不可达 → 503；`/healthz` 保持纯存活语义（依赖故障不触发重启）；SPA 静态托管下 `/readyz` 与 `/api` 同优先级 | `internal/api/handlers.go`、`cmd/server/main.go`、`internal/search/meili.go`（新增 `Health`）、`internal/storage/storage.go`（新增 `Healthy`） |
| A2-03 请求体上限硬编码，`MAX_UPLOAD_BYTES` 调小不生效 | low | 上限改为 `store.MaxBytes() + maxUploadOverheadBytes(1MiB)` | `internal/api/handlers.go` |
| A2-04 白名单只约束后缀；预览按 DB 声明 MIME 内联返回 | low | 预览前用 `storage.MatchedContentType` 核对内容与声明，不一致 → 415；响应补 `Content-Security-Policy: default-src 'none'; …; sandbox`（原有 `nosniff` 保留） | `internal/api/handlers.go`、`internal/storage/image.go` |
| A2-04 **误拒修复（本轮新发现）** | — | 初版按逐字相等比较，`.md`/`.csv`/`.json`（嗅探均为 `text/plain`）会被误拒；改为文本家族收敛判定：声明属 `text/*`／`application/json` 且嗅探结果为文本类型即放行，其余仍须一致 | `internal/storage/image.go` |
| A2-05 落盘成功但入库/回读失败留孤儿文件 | low | 失败路径调用 `storage.Discard`（幂等、经 `Resolve` 越界校验）；去重命中对象不删（非本次请求所写） | `internal/api/handlers.go`、`internal/storage/storage.go` |
| A2-06 `ReadTimeout=15s` 截断慢速大文件上传并误报 400 | low | 服务端级超时保持严格（读 15s／头 15s／写 30s），由上传端点用 `http.ResponseController` 放宽本次请求的读（5min）与写（5min+30s）期限；**同时消除 `WriteTimeout=30s` 对慢上传的第二道墙** | `internal/api/handlers.go`、`cmd/server/main.go` |
| 工程卫生 | — | `gofmt` 修 `cmd/bench/main.go`、`internal/worker/worker.go` | 同上 |
| 边界行为对齐（自查补修） | — | 流式改造后「全空白文件名」原本会落到 415（旧实现是 400）；补 `errUploadNoFileName` 守卫，保持与原状态码一致，并加单测 | `internal/api/handlers.go`、`internal/api/upload_test.go` |

**接口语义变化**：新增 `GET /readyz`（additive）；`POST /api/v1/assets/upload` 请求体上限跟随 `MAX_UPLOAD_BYTES`（默认值不变）；`GET /api/v1/assets/{id}/preview` 新增 415（有意的行为收紧）。**回滚**：单提交 `git revert` + 重跑 `docker compose build server`；无数据迁移、无卷内容变更。

---

## 2. L1 证据

```text
$ gofmt -l cmd internal          # 空输出

$ GOCACHE=/tmp/go-cache-resume GOFLAGS=-mod=vendor go build ./...   → build=0
$ GOCACHE=/tmp/go-cache-resume GOFLAGS=-mod=vendor go vet ./...     → vet=0
$ GOCACHE=/tmp/go-cache-resume GOFLAGS=-mod=vendor go test -race -count=1 ./...
?   partisync/server/cmd/bench      [no test files]
?   partisync/server/internal/models [no test files]
?   partisync/server/internal/store  [no test files]
?   partisync/server/internal/worker [no test files]
ok  partisync/server/cmd/server     1.014s
ok  partisync/server/internal/api   1.266s
ok  partisync/server/internal/search 1.017s
ok  partisync/server/internal/storage 1.023s
test=0
```

> `GOCACHE` 显式指向 `/tmp`：本环境默认 `~/.cache/go-build` 曾被沙箱拒绝并产生**假绿**（审计 §6 教训），故取真实退出码（文件重定向，不经管道）。

**新增用例（15 个，全部有断言力；本批 `75fd604`）**

| 文件 | 用例 | 断言对象 |
|---|---|---|
| `internal/api/readyz_test.go` | `TestReadyWithStorageOnly` / `TestReadyNotReadyWhenMeiliUnreachable` / `TestReadyNotReadyWhenStorageBroken` / `TestHealthzStaysAliveWhenStorageBroken` | `/readyz` 逐依赖聚合与 503、`/healthz` 语义分离 |
| `internal/api/upload_test.go` | `TestUploadRequestBodyCapFollowsStoreLimit` / `TestSaveUploadedFileStreamsWithoutTempSpill` / `TestSaveUploadedFileKeepsFirstFilePart` / `TestSaveUploadedFileRejectsNonMultipart` / `TestSaveUploadedFileRejectsBlankFileName` / `TestUploadBudgetsExceedServerDefaults` | 请求体上限推导、零 /tmp 落盘、重复部件、非 multipart 400、空白文件名 400（与旧实现一致）、超时预算口径 |
| `internal/storage/image_test.go` | `TestMatchedContentType`（14 例）/ `TestMatchedContentTypeIgnoresMimeParams` / `TestSniffMimeNormalizesParams` | 内容-声明相容判定（含伪装图片拒绝与文本家族放行） |
| `internal/storage/storage_test.go` | `TestDiscardRemovesObjectIdempotently` / `TestHealthyDetectsMissingTmpDir` | 孤儿清理幂等与越界拒绝、就绪探测 |
| `cmd/server/main_test.go` | `TestHTTPServerTimeouts` / `TestUploadLimitFromEnv` | 服务端级超时不得放宽、`MAX_UPLOAD_BYTES` 解析口径 |

---

## 3. 断言力（反向验证 / falsification）

把旧实现临时还原到 `/tmp/partisync-falsify` 副本（不动仓库），确认新测试确实抓得住回归：

| 断言 | 反向实验 | 观察结果 |
|---|---|---|
| 文本家族放行（A2-04 误拒修复） | 还原为逐字相等比较 | `TestMatchedContentType` FAIL：`md@text/markdown` / `csv@text/csv` / `json@application/json` / `md-with-html@text/markdown` 四例 want true got false；`TestMatchedContentTypeIgnoresMimeParams` FAIL |
| 请求体上限跟随配置（A2-03） | 还原为 `storage.DefaultMaxBytes+1MiB` | `TestUploadRequestBodyCapFollowsStoreLimit` FAIL：`want 413, got 400`（1MiB 垃圾字段被放行后落到"缺少 file 部件"） |
| 无 `/tmp` 落盘（A2-01） | 独立小程序复现旧路径（`ParseMultipartForm(8MiB)` + `TMPDIR=/tmp/oldspill-dir`，12MiB 部件） | 输出：`TMPDIR=/tmp/oldspill-dir entries=1` → `spill: multipart-3399161390 (12582912 bytes)`。即：`TestSaveUploadedFileStreamsWithoutTempSpill` 断言的"该目录为空"在旧实现下必然失败 |
| 慢速上传不被截断（A2-06） | 行为面由 S7 提供（41s 上传，超过旧 15s 读超时与 30s 写超时） | 201（见 §4） |

---

## 4. L2 证据（真实容器 + 真实 HTTP）

**脚本**：`scripts/l2-p2-hardening.sh`（可复现；夹具每次运行唯一化，避免命中内容去重与历史残留 sha）。
**运行**：`bash scripts/l2-p2-hardening.sh`（前置：`docker compose up -d` 三容器 healthy；镜像 `partisync-server:local` 已构建）
**结果**：`PASS=53 FAIL=0 → RESULT=PASS`（退出码 0）

```text

=== 前置检查：容器与镜像 ===
PASS  container partisync-server healthy
PASS  container partisync-postgres healthy
PASS  container partisync-meilisearch healthy
PASS  image partisync-server:local present

=== S1 探针基线（AC-06） ===
PASS  /healthz → 200
PASS  /healthz payload status=ok
PASS  /readyz（全依赖可达） → 200
PASS  /readyz 依赖 postgres ok = true
PASS  /readyz 依赖 meilisearch ok = true
PASS  /readyz 依赖 storage ok = true

=== S2 A2-04 内容-声明一致性（AC-01） ===
PASS  上传 HTML 伪装 .png → 201
PASS  入库 MIME 为嗅探结果（未回退扩展名） = text/html
PASS  预览该资产（声明 text/html 与内容一致） → 200
PASS  预览响应含 X-Content-Type-Options: nosniff
PASS  预览响应含 CSP sandbox
PASS  登记声明 image/png 的伪装资产 → 201
PASS  伪装资产声明 MIME = image/png
PASS  预览伪装资产（内容与声明不一致） → 415

=== S3 A2-04 文本家族误拒回归（AC-02） ===
PASS  上传 notes.md → 201
PASS  登记声明 text/markdown 的孪生资产（notes.md 的真实字节） → 201
PASS  预览 声明=text/markdown 内容=notes.md → 200
PASS  预览 notes.md 返回非空字节（53 bytes）
PASS  上传 data.csv → 201
PASS  登记声明 text/csv 的孪生资产（data.csv 的真实字节） → 201
PASS  预览 声明=text/csv 内容=data.csv → 200
PASS  预览 data.csv 返回非空字节（15 bytes）
PASS  上传 config.json → 201
PASS  登记声明 application/json 的孪生资产（config.json 的真实字节） → 201
PASS  预览 声明=application/json 内容=config.json → 200
PASS  预览 config.json 返回非空字节（43 bytes）
PASS  上传 photo.png → 201
PASS  登记声明 image/png 的孪生资产（photo.png 的真实字节） → 201
PASS  预览 声明=image/png 内容=photo.png → 200
PASS  预览 photo.png 返回非空字节（98 bytes）

=== S4 A2-01 大文件不落容器 /tmp（AC-03） ===
PASS  上传 12MiB 文件 → 201
INFO  可写层 before=4198 bytes after=4198 bytes delta=0 bytes
PASS  容器可写层增量 < 4MiB（12MiB 未落在 /tmp）
PASS  容器 /tmp 条目数 = 0

=== S5 A2-03 请求体上限跟随 MAX_UPLOAD_BYTES（AC-04） ===
PASS  一次性容器（MAX_UPLOAD_BYTES=1MiB）就绪
PASS  1MiB 上限下上传 2MiB → 413
PASS  1MiB 上限下上传 512KiB → 201

=== S6 A2-05 孤儿文件清理 + A2-02 探针语义分离（AC-05/AC-06） ===
PASS  /readyz 在 PG 停机后转为 503
PASS  /readyz 标记 postgres 未就绪 = false
PASS  /readyz 标记 meilisearch 仍然就绪 = true
PASS  /healthz 在依赖故障时仍 200（纯存活语义） → 200
PASS  PG 停机时上传（入库必失败） → 500
PASS  孤儿文件已清理（/data/assets/a4/c7b7578b3de4022dd8c90b45eb2b6efa7d65c0d7ffe3a38c16f4102bd45327 不存在）
PASS  /readyz 在 PG 恢复后回到 200
PASS  /readyz 在 Meili 停机后转为 503
PASS  /readyz 标记 meilisearch 未就绪 = false
PASS  /healthz 在 Meili 故障时仍 200 → 200
PASS  /readyz 在 Meili 恢复后回到 200

=== S7 A2-06 慢速大文件上传（AC-07） ===
INFO  慢速上传状态=201 用时=40.972351s（服务端常规读/写超时 15s/30s）
PASS  4MiB @100KB/s 慢速上传 → 201
PASS  上传耗时 > 35s（超过旧 15s 读超时与 30s 写超时）

=== 汇总 ===
PASS=53 FAIL=0
RESULT=PASS
```

**AC-01 的威胁模型构造说明**：上传端点存储的是**嗅探结果**，因此"声明与内容不一致"只能由元数据端点 `POST /api/v1/assets` 构造——该端点接受客户端自报的 `path` / `sha256` / `mime_type`（仅做格式与长度校验，无存在性/一致性校验）。脚本据此把同一份 HTML 字节登记成 `image/png` 资产，预览即 415；而真实 `text/html` 资产预览 200。两者对照说明判定的是**声明与内容是否相容**，而非内容本身。

---

## 5. L3 回归（AC-08）

同一批改动后，复跑 T7 浏览器闭环（对象为**静态托管的构建产物** `127.0.0.1:8080`，非 Vite dev server）：

```text
$ L3_BASE=http://127.0.0.1:8080/ L3_SHOTS=docs/verification/p2-hardening node scripts/l3-mcd-acceptance.mjs
{"ok": true, "checks": [8 项全 true], "failed": [], "consoleErrors": [], "badResponses": []}
```

| 断言 | 结果 | 细节 |
|---|---|---|
| 1.1 列表渲染非空 | PASS | cards=24 |
| 2.1 上传/去重提示可见 | PASS | notice shown |
| 3.1 搜索命中上传资产 | PASS | cards=1 |
| 4.1 预览图加载成功 | PASS | HTTP 200、82 bytes |
| 5.1 人工标签写入并展示 | PASS | badge visible |
| 6.1 标注任务完成 | PASS | pending → completed |
| 7.1 确认成功提示可见 | PASS | confirm notice |
| 7.2 AI 标签写入并展示 | PASS | ai badges=3 |

产物：`docs/verification/p2-hardening-run.json`、截图 `docs/verification/p2-hardening/`（7 张）。

---

## 6. Acceptance Matrix 状态

| ID | 准则 | 状态 | 证据 |
|---|---|---|---|
| AC-01 | 内容与声明不一致 → 预览 415；一致 → 200 | **pass** | 单测 15 例（`75fd604`）+ L2 S2（415/200 对照） |
| AC-02 | 文本家族不误拒 | **pass** | 单测 + L2 S3（md/csv/json/png 孪生 200） |
| AC-03 | 12MiB 上传零 /tmp 落盘、可写层零增长 | **pass** | L2 S4（delta=0 bytes、/tmp 条目 0）+ 反向实验 |
| AC-04 | `MAX_UPLOAD_BYTES` 生效 | **pass** | L2 S5（1MiB→2MiB 413、512KiB 201） |
| AC-05 | 入库失败不留孤儿、去重对象不误删 | **pass** | L2 S6（PG 停机上传 500 + 对象不存在） |
| AC-06 | `/readyz` 依赖故障 503、`/healthz` 保持 200 | **pass** | L2 S1/S6（PG、Meili 双向停机-恢复） |
| AC-07 | 慢速上传不被截断 | **pass** | L2 S7（40.97s → 201） |
| AC-08 | T7 L3 闭环无回归 | **pass** | §5（8/8、consoleErrors=0、badResponses=0） |
| AC-09 | gofmt 空 + L1 三绿 | **pass** | §2 |
| AC-10 | 证据文档 + SESSION.md + 提交 | **pass** | 本文 + `docs/SESSION.md` §13 + git 提交 |
| AC-11 | 独立复审 | **pending** | **未执行**：本批为实施者自证，建议下轮派 fresh-context 审计员 |

---

## 7. 盲区 / 未覆盖 / 残留（诚实清单）

1. **无独立复审**：A2-04 的"文本家族"放行规则与 A2-01 的流式解析是本轮**新增实现**，只有实施者自证；建议复审员重点证伪"文本家族放行是否引入新的类型混淆面"。
2. **前端对 415 无文案**：预览 `<img>` 的 `onError` 只隐藏预览容器，用户看不到"预览不可用"的原因（任务卡 UV-01 的期望值据此调整为"预览区不渲染"）。属 UX 残留（P3），未在本批修。
3. **上传端点无认证**：A2-07…A2-09（info）仍打开；无认证下"写满磁盘"的并发面未压测。
4. **`/tmp` 落盘的规模边界**：证明只在 12MiB（超过旧实现 8MiB 内存阈值）上做，未做 100MiB 量级；`/readyz` 的存储检查只验 `.tmp` 目录可达，未做写入探测。
5. **慢速上传测到 41s**：未验证 5min 预算的边界与超时后的错误呈现（旧实现 15s 截断表现为 400，新预算用尽时的表现未观测）。
6. **`POST /api/v1/assets` 仍接受客户端自报 `path`/`mime_type`**：本批只在**预览**这一内联返回面加了一致性校验；DB 中仍允许存在"声明与内容不一致"的行（设计现状，非本批目标）。预览是唯一内联返回文件字节的端点，其余端点只返回元数据 JSON。
7. **测试残留未清理**：L2 脚本会向库中写入 `p2-*` 测试资产（含 12MiB/4MiB 对象）；库中另有历史残留（`t4-l2.jpg`、T7/L3 与 A2 审计的 `evil.png`/`passwd.png` 等）。清理需删行 + 删卷内对象，属数据变更，**未执行**，待用户批准。
8. **L2 脚本副作用**：会短暂停止/启动 `partisync-postgres` 与 `partisync-meilisearch`（已实测可恢复、幂等）；一次性容器数据目录经同镜像容器删除（宿主 `rm` 对 uid 10001 文件无权限）。

---

## 8. 恢复事件

| 事件 | 级别 | 情况 | 处置 |
|---|---|---|---|
| P2-L2-1 | L1 | 首轮 L2 `PASS=49 FAIL=4`：失败均为**夹具问题**而非实现缺陷——① 伪装资产用了固定 `sha256=deadbeef…`，撞上 T4 时期残留行（`t4-l2.jpg`）→ 返回 200 existing；② `photo.png` 固定字节与 T7/L3 脚本用图完全一致 → 命中内容去重返回 200；③ 第二轮孪生资产用确定性 sha → 撞上一轮残留行 | 夹具唯一化（随机 sha / 追加随机字节 / 内容含 nonce），第三轮 `53/53 PASS` |
| P2-L2-2 | L1 | 一次性容器数据目录宿主 `rm -rf` 失败（文件属容器内 uid 10001） | 改为用同镜像容器删除后再 `rmdir`；脚本内 `cleanup_cap_data` 同时挂到 `trap` |
| P2-L2-3 | L1 | 初版 A2-04 实现（逐字相等）会把 `.md`/`.csv`/`.json` 预览误拒为 415 | 在 L1 阶段以表格单测发现并改为文本家族收敛判定；L2 S3 以"孪生资产"实测覆盖 |

---

## 9. 声明

- 本批**修复与验证均由主 Agent（实施者）完成**，按门禁口径**不构成独立复审**；`docs/evidence/audit-t6-prime.md` 的原始 `verdict=fail` 与本文件无关，门禁是否放行需由独立复审（AC-11）+ 用户决策。
- 本环境无可视觉判读模型：L3 的"预览正常"依据运行时 DOM/网络断言（HTTP 200 + 字节非空 + `img` 存在），非人眼像素校验；`docs/verification/p2-hardening/` 截图仅作过程留痕。

---

## 11. 复审后修复（提交 `a6a6702`）：解析期孤儿对象

本节由 P2 复审（`docs/evidence/audit-p2-hardening.md`，**非独立**）追加。

- **发现（F6, medium）**：multipart 中 `file` 部件已合法落盘、随后部件触发请求体上限时，
  `saveUploadedFile` 直接返回错误而**不清理已落盘对象** → 无 DB 行的孤儿文件。
  一次性容器（`MAX_UPLOAD_BYTES=4096`）实测：file 部件 4096B + junk 部件 2MiB → HTTP **413**，
  但 `/data/assets/26/b7e40be0bcf3e6667020b3acf6e07faa17585b21b2936305dd6c9ad3860b15` 存在。
  本批 A2-05 只覆盖了"入库失败"路径，未覆盖"落盘成功之后解析失败"。
- **修复**：`saveUploadedFile` 改具名返回 + `defer` 清理本次已提交对象（去重命中对象不删）；
  顺带收口 400 文案（不再回显包装后的内部错误串）。
- **反向验证**：把清理逻辑还原为旧写法后，新单测
  `TestUploadCleansUpObjectWhenBodyCapTripsAfterFilePart` 失败：
  `orphan objects left after parse failure: 1 files`。
- **新增 L2 常驻回归（S8）**：一次性容器（1MiB 上限）对照组 201 + 实验组 413 +
  失败请求对象不存在 + 先前对象未被误删 + 存储残留文件数 + `.tmp` 残留条目。

**修复版复验（本文件 §4 的 53/53 是 `75fd604` 的记录，修复后为 60/60）**：

```text
--- 修复验证（一次性容器重跑 F6 场景）---
  F6 重跑 status=413 body={"error":"upload exceeds maximum request size"}
  RESULT=NO_ORPHAN（已修复）
  卷内文件数=0
  卷内 .tmp 条目=0

=== S8 解析期失败不留孤儿对象（F6 复审回归） ===
PASS  一次性容器（1MiB 上限）就绪
PASS  S8 对照组（无超限字段）上传 → 201
PASS  S8 超限请求（file 部件已落盘后解析失败） → 413
PASS  S8 失败请求的 file 部件对象被清理（无孤儿）
PASS  S8 先前成功提交的对象未被误删
PASS  S8 存储内残留文件数 = 1
PASS  S8 存储 .tmp 残留条目 = 0

=== 汇总 ===
PASS=60 FAIL=0
RESULT=PASS
```

- L1：`build/vet/test -race` 三绿（新用例 16 个 = `75fd604` 15 个 + `a6a6702`/F6 1 个）；`gofmt -l cmd internal` 为空。
- L3：T7 闭环 **8/8 PASS**（截图已用修复版镜像重采），`consoleErrors=[]`、`badResponses=[]`。
- **独立性**：修复与复验同样由实施者完成，**不构成独立复审**。
