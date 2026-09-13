# L2 集成验证 — P2.1 复审修复批（2026-09-13）

> 对应独立复审 findings（`docs/evidence/audit-p2-independent-{security,probe-timeout,evidence}.md`，三面 verdict 均 pass）：
> 修复 4 个 medium（S1/S2/F-I1/M1）+ 6 个低成本 low（S5、F-I3、F-I4、F-I5、L1 文档口径、L2 脚本 `ok` 判定）。
> S3（.md 内联 HTML 的 CSP 单防线）、S4（并发去重 Discard 竞态）**本批未修**，见文末残留。

## 1. 交付机制

| ID | 修复 | 位置 |
|---|---|---|
| S1 | 回读失败分支改为**先删回 DB 行（`store.DeleteAssetBySHA256`）再 Discard 对象**；删行失败则行+对象均保留（宁留孤儿文件，不留悬空 DB 行）；`Deduped` 对象仍不删 | `internal/api/handlers.go`、`internal/store/store.go` |
| S2 | 上传解析改为自建 `multipart.Reader`（保留 MaxBytesReader 语义）：部件头阶段启用 `headerBudgetReader`（**64KiB/部件**）+ 部件数上限 **32**；超限 → 400 `errPartHeaderTooLarge` / `errTooManyParts`。注：Go 1.22 stdlib 另有 10MiB/部件、10000 条目兜底，但都在本预算之后触发，实际不可达 | `internal/api/handlers.go` |
| F-I1 | `storage.Healthy()` 由 `os.Stat` 弱探针改为**真实写探针**：校验 `.tmp` 存在且为目录 + create/delete 临时文件验证可写；docstring 同步（F-I8） | `internal/storage/storage.go` |
| S5 | 新增 `storage.CleanStaleTemp(maxAge)`，`main.go` 启动时清扫 `.tmp` 内 >1h 的 `upload-*` 崩溃残留 | `internal/storage/storage.go`、`cmd/server/main.go` |
| M1 | L2 脚本 S5/S8 一次性容器由宿主 `/tmp` bind mount 改**命名卷**（镜像内 `/data/assets` 已预建并 chown app，命名卷初始化继承属主）；清理函数改 `docker volume rm -f` | `scripts/l2-p2-hardening.sh` |
| F-I3 | compose `server` 暴露 `MAX_UPLOAD_BYTES: ${MAX_UPLOAD_BYTES:-}` | `docker-compose.yml` |
| F-I4 | `allowSlowUpload` 对 `SetReadDeadline/SetWriteDeadline` 失败记日志（防未来中间件改动导致放宽静默失效） | `internal/api/handlers.go` |
| F-I5 | 任务卡 A2-06 口径修正（服务端级 15s/30s 保持，仅上传端点按请求放宽） | `docs/P2-HARDENING-TASK-CARD.md` |
| L1(证据) | 单测计数统一为「75fd604 15 个 + F6 1 个 = 16」；SESSION.md 「14 个」笔误修正 | `docs/evidence/l2-integration-p2-hardening.md`、`docs/SESSION.md` |
| L2(证据) | L3 脚本 `ok` 判定纳入 `badResponses.length === 0`（AC-08） | `scripts/l3-mcd-acceptance.mjs` |
| S8 幂等 | L2 S8 对照载荷加运行唯一 nonce（否则重跑命中 DB 去重 → 200 ≠ 201），`ctl_sha` 按实际载荷文件计算 | `scripts/l2-p2-hardening.sh` |

## 2. L1 验证（真实退出码，GOCACHE=/tmp/gocache-p21）

- `go build ./...` exit=0；`go vet ./...` exit=0；`go test ./... -race` exit=0（api / cmd/server / search / storage 四包 ok）。
- `gofmt -l cmd internal` 为空。
- **新增测试 9 个**：
  - storage：`TestHealthyDetectsTmpReplacedByRegularFile`、`TestHealthyDetectsReadOnlyTmp`（root 跳过）、`TestCleanStaleTempRemovesOnlyOldUploadFiles`；
  - api：`TestUploadRejectsOversizedPartHeader`、`TestUploadAcceptsNormalPartHeaderUnderBudget`、`TestUploadMapsOversizedHeaderLineToBadRequest`、`TestUploadRejectsTooManyParts`；
  - store（DSN 门控集成）：`TestInsertAndDeleteAssetBySHA256`——对容器内真实 PG 实跑 PASS（`PG_DSN=127.0.0.1:5432`）。
- **反向验证**：把 `maxPartHeaderBytes` 临时改回 `1<<30`（≈旧行为）后，两个 S2 测试**确实失败**；还原后通过。证明测试有断言力。

## 3. L2 验证（`bash scripts/l2-p2-hardening.sh`，新镜像 `partisync-server:local` 10:46 构建）

- 首轮 59/60：唯一 FAIL 为 S8 对照组 200≠201——**脚本对重跑非幂等**（载荷命中 DB 去重），非被测代码缺陷；
  次轮暴露 `ctl_sha` 仍按旧常量计算，随 S8 幂等修复一并解决。
- **终轮 60/60 PASS**（RESULT=PASS），含：A2-01 零 /tmp 落盘、A2-02 探针语义分离（PG/Meili 停机 503 + /healthz 200）、
  A2-03 1MiB 上限 413/512KiB 201、A2-04 415/文本家族 200、A2-05 PG 停机孤儿清理、A2-06 慢上传 40.97s→201、
  F6/S8 无孤儿 + 先前对象不误删 + `.tmp` 残留 0。
- **S2 live 探针**（60/60 之外补测，:8080 新镜像）：150KB 部件头请求 → **400** `{"error":"multipart part header exceeds size limit"}`；
  紧接正常上传 → **201**（预算只约束头部阶段，不影响正常文件）。

## 4. L3 回归

`L3_BASE=http://127.0.0.1:8080/ node scripts/l3-mcd-acceptance.mjs` → **exit=0，8/8 PASS**，`consoleErrors=[]`、
`badResponses=[]`（且 `ok` 判定现已纳入 badResponses，为空态下通过）。截图重采至 `docs/verification/t7-l3/`。

## 5. 残留与盲区（如实声明）

> **P2.2 增补（2026-09-13 同日）**：S3 与 S4 已修复并验证，第 5 节原「未修」清单相应过期（见 §6）。

## 6. P2.2 增补：S3 / S4 修复与验证（2026-09-13）

| ID | 修复 | 位置 |
|---|---|---|
| S3 | 预览响应 MIME 绝不允许 HTML 家族：`previewResponseMime()` 把 `text/html` / `application/xhtml+xml`（含带参数形式）降级为 `text/plain; charset=utf-8`（按源码渲染，浏览器不解析标签）；CSP sandbox + nosniff 保持第二、三道防线 | `internal/api/handlers.go` |
| S4 | `storage.Save` 提交由 Stat→Rename 改为 **link() 原子提交**：并发同内容上传恰好一个请求创建终路径对象（Deduped=false），其余得 ErrExist（Deduped=true）；失败路径 Discard 只可能由创建者执行，不可能误删他方引用的共享对象（Windows 不支持 link，部署目标为 Linux 容器） | `internal/storage/storage.go` |

验证：
- L1 三绿（新增 `TestPreviewResponseMimeDowngradesHTML`、`TestSaveConcurrentSameContentExactlyOneCreator`）；S4 反向验证——把 EEXIST 分支强制置 Deduped=false（≈旧行为）后并发测试确实失败，还原后通过。
- live（:8080 新镜像）：`<script>` 开头的 `.md` 以 `stored_mime=text/html` 入库 → 预览 200 且 `Content-Type: text/plain; charset=utf-8`；8 路并发同内容上传 → DB 恰 1 行、卷恰 1 对象、201×1 + 200×7。
- L2 全量 60/60 PASS（`docs/verification/p2.1/l2-p22-run.log`）；L3 8/8 PASS（`docs/verification/p2.1/l3-p22-run.json`）。


- **未修（~~留待 P2.2~~ → P2.2 已修 S3/S4，见 §6）**：~~S3（`.md` 携带 `<script>` 内联预览仅靠 CSP sandbox 兜底）~~ ✅、~~S4（并发同内容上传 Discard 竞态，窗口小）~~ ✅、
  F-I2（/readyz 在 compose 无人消费，架构性）、F-I6（readyz 缺 PG 失败路径单测）、F-I7（/readyz 错误串回显拓扑，127.0.0.1 部署可接受）。
- **部分实现**：F-I1 的磁盘剩余空间检查需平台 syscall，本批未含（写探针已覆盖只读/文件替换两个假阳性场景）。
- **无法 live 复现**：S1 的回读失败分支需在 insert 与回读之间注入 DB 故障，L2 环境无法安全注入；
  覆盖方式为 store 集成单测（真 PG 实证 `DeleteAssetBySHA256` 语义）+ handler 补偿顺序代码走查。
- 本批修复与验证由实施者（主 Agent）完成；**独立复审面已由三份 fresh-context 审计报告构成（AC-11 已满足）**，
  本批改动如需再审计，建议下轮派发时点名 S1/S2 两处 diff。
