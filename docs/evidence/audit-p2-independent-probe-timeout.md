# P2 加固批独立复审报告 — 就绪探针 / 请求体上限 / 上传超时（fresh-context）

- **审计对象**：commit `75fd604`（A2-01…A2-06 实现）+ commit `a6a6702`（复审发现的解析期孤儿对象泄漏 F6 修复）
- **审计员**：fresh-context 独立审计（无历史会话，仅依据仓库现状 + 实测）
- **审计面**：就绪探针（`/readyz`、`/healthz`）/ 请求体上限（`MAX_UPLOAD_BYTES`）/ 上传超时（`ResponseController`）/ 配置面一致性
- **审计时间**：在仓库 commit `8644b55` 状态下审查 `75fd604`/`a6a6702` 的实际语义；运行 live server (127.0.0.1:8080) 做对照实测
- **结论**：**pass（A2-06 实测成立，A2-03 数学与运行时成立；A2-02 实现有 1 项语义薄弱点 + 1 项文档不一致；F6 修复有效）**

---

## 0. 验证基线

```text
$ go build ./...                                  → 0
$ go vet ./...                                    → 0
$ go test -race -count=1 ./...                    → 全部 PASS（4 包 ok）
$ curl http://127.0.0.1:8080/healthz              → 200 {"status":"ok"}
$ curl http://127.0.0.1:8080/readyz               → 200 {"ready":true,"dependencies":[{"name":"postgres","ok":true},{"name":"meilisearch","ok":true},{"name":"storage","ok":true}]}
$ docker inspect partisync-server --format '{{range .Config.Env}}{{println .}}{{end}}'
  → MEILI_KEY / STORAGE_ROOT=/data/assets / ADDR=:8080 / PG_DSN=postgres://…@postgres:5432/partisync / MEILI_URL=http://meilisearch:7700 / WEB_DIST=/app/web
  → （**未设置** MAX_UPLOAD_BYTES，故 default = 100 MiB；body cap = 100 MiB + 1 MiB = ~101 MiB）
```

---

## 1. 逐项核查

### 1.1 A2-02 `/readyz` 探测口径

#### 1.1.1 三项探测各自的判定真实性

**Postgres** — `internal/api/handlers.go:113`
```go
if _, err := s.store.CountAssets(ctx); err != nil { … }
```
`internal/store/store.go:126-132`：
```go
func (s *Store) CountAssets(ctx context.Context) (int64, error) {
    var n int64
    if err := s.db.QueryRowContext(ctx, "SELECT count(*) FROM assets").Scan(&n); err != nil { … }
    return n, nil
}
```
**真实查询**：执行 `SELECT count(*) FROM assets`，带 3s context timeout。✅ 不是 TCP-only 假探针——会真实拉一条 PG 往返。

**Meilisearch** — `internal/api/handlers.go:123`
```go
if err := s.meili.Health(ctx); err != nil { … }
```
`internal/search/meili.go:172-190`：
```go
func (c *Client) Health(ctx context.Context) error {
    ctx, cancel := context.WithTimeout(ctx, 3*time.Second)
    defer cancel()
    raw, err := c.do(ctx, http.MethodGet, "/health", nil)
    …
    if resp.Status != "available" { return fmt.Errorf("meilisearch status %q", resp.Status) }
    return nil
}
```
**真实 HTTP 调用** Meili 的 `/health`，解析 `{"status":"available"}`。✅ 不仅 TCP 连通，且验证 Meili 自身处于可用状态。

**Storage** — `internal/api/handlers.go:131`
```go
if err := s.content.Healthy(); err != nil { … }
```
`internal/storage/storage.go:85-91`：
```go
func (s *Store) Healthy() error {
    if _, err := os.Stat(filepath.Join(s.root, tmpDirName)); err != nil { … }
    return nil
}
```
**仅 `os.Stat` 检查 `.tmp` 目录存在性**。⚠️ 详见 §1.1.3。

#### 1.1.2 探测超时与整体响应超时

每个 dep 独立 3s context timeout；handler 串行调用（PG → Meili → storage），全部失败的极端情况 ≈ 9s 总时长，在 server-level `ReadTimeout=15s`（`cmd/server/main.go:30`）之内。✅

#### 1.1.3 ⚠️ `storage.Healthy()` 是**弱探针**

**实测**（独立写 `.audit-tmp/healthy_probe.go` 用 `go run` 执行）：
```
1) baseline Healthy:                              <nil>          ← 正常
2) after removing .tmp Healthy:                   ERROR (no such file or directory)
3) with read-only .tmp (chmod 555) Healthy:       <nil>          ← 假阳性！
4) with .tmp being a regular file Healthy:        <nil>          ← 假阳性！
5) with root removed Healthy:                     ERROR (no such file or directory)
```

**结论**：storage 健康度只检存在性，**不验证写权限、不验证磁盘空间**。注释 `"not writable-ready"` 名实不符。生产环境若资产卷被设为只读（运维误操作/容量已满被内核置 RO），**`/readyz` 会假阳性 200**，上传时再 ERR_FAIL。

代码/注释不一致的修复建议：要么改为 `os.Stat` 后追加真实写小文件 + 删除，要么把注释改成"目录存在"。详细见 §3 finding 1。

#### 1.1.4 docker-compose healthcheck 是否用对了探针

`docker-compose.yml:64-68`：
```yaml
healthcheck:
  test: ["CMD-SHELL", "wget -q -O - http://127.0.0.1:8080/healthz | grep -q 'ok' || exit 1"]
  interval: 5s
  timeout: 5s
  retries: 5
```

**用 `/healthz`（liveness）是正确的**——Docker 用它决定容器是否重启；用 `/readyz` 会让 Docker 在依赖故障时**重启 server 容器**，这恰恰违背 liveness/readiness 分离的设计意图（A2-02 注释明确写"依赖故障不应触发进程重启"）。

但 `depends_on.condition: service_healthy` 对 PG/Meili 已确保 server 启动时依赖就绪——server 启动之后若 PG 挂掉，**没有任何编排器摘流量**：docker-compose 不支持 readiness，server `/readyz` 存在但无人调用。**这是文档声明与编排能力的落差**——`/readyz` 是为 K8s readinessProbe 准备的，compose 这条路径是孤儿。

✅ 探针选型正确；⚠️ 编排端未消费 `/readyz`，存在信息盲区（详见 §3 finding 2）。

#### 1.1.5 F2（内部拓扑泄露）独立判断

`/readyz` 在 dep 不可达时把原始错误回写到响应体：
```json
{"dependencies":[{"name":"postgres","ok":false,"error":"count assets: dial tcp: lookup postgres on 127.0.0.11:53: server misbehaving"}, …]}
```
实测环境 PG 故障时（来自 `docker logs`：插入时报 `dial tcp: lookup postgres on 127.0.0.11:53: server misbehaving`），同样的字符串会出现在 `/readyz` 响应里。**泄露面**：
- 容器编排 DNS（`127.0.0.11:53`，Docker bridge 嵌入式 DNS 的内网地址）
- 服务主机名（`postgres`、`meilisearch`）
- Meili URL 与 HTTP 路径
- **不含凭据、不含文件路径、不含用户数据**

**独立判断**（与先前主 Agent 自查 F2 一致）：在仅 127.0.0.1:8080 绑定、不对外暴露的前提下，**真实风险低**——这些拓扑信息本就对该主机可观测者可见（`docker network inspect` 即可拿到）。**接受**。一旦上反向代理对外，必须改成 `{name, ok}` 二元 + 服务端日志留存详情（这是 K8s readiness 的标准做法）。

---

### 1.2 A2-03 请求体上限 = `store.MaxBytes() + 1MiB`

#### 1.2.1 限值计算

`internal/api/handlers.go:569`：
```go
r.Body = http.MaxBytesReader(w, r.Body, s.content.MaxBytes()+maxUploadOverheadBytes)
```
`maxUploadOverheadBytes = 1 << 20 = 1 MiB`（handlers.go:523）。

**溢出**：int64 加 1 MiB 在 `MaxBytes` 取任何正 int64 值时都不溢出（`math.MaxInt64 ≈ 9.2e18 >> 1MiB`）。
**下界**：`MaxBytes() >= 1`（因 `storage.New` 在 `maxBytes <= 0` 时回退 `DefaultMaxBytes`），所以 body cap 至少 `1 + 1MiB ≈ 1MiB`，永不为 0 或负。`MaxBytesReader` 对 `n < 0` 视为 0，此处不会触发。
**HTTP 层**：未做向上取整；`MaxBytes() = 1` 时 body cap = `1 + 1048576 = 1048577` bytes。这个上限对单文件 1 byte 的场景是宽松的（多 1MiB 余量给 multipart envelope），符合"小文件 + 余量"的语义。✅

#### 1.2.2 413 响应路径

`handlers.go:706-723` 把 `errors.As(&maxErr)` 映射为 `http.StatusRequestEntityTooLarge`（413），并由 `uploadErrorMessage`（handlers.go:729-733）区分两种 413：
- `storage.ErrTooLarge` → `"file exceeds maximum size"`
- 其他（含 `*http.MaxBytesError`）→ `"upload exceeds maximum request size"`

**实测（live server，default 100 MiB cap）**：
- 102 MiB 文件 → `413 file exceeds maximum size`（storage cap 先触发，因为 102 > 100）
- 99 MiB 文件 + 3 MiB junk field → `413 upload exceeds maximum request size`（body cap 触发，multipart 总长 > 101 MiB）
✅ 两条 413 路径都正确。

#### 1.2.3 multipart 流式解析与上限的交互

`saveUploadedFile`（handlers.go:653-703）逐 part 流式读：
1. file 部件先被 `Save` 落盘（成功返回 saved，err=nil）
2. 后续部件若触发 body cap → `mr.NextPart()` 返回 wrap 后的 `*http.MaxBytesError` → `return saved, clientName, partErr`（保留 saved）
3. **defer 触发清理**：`saved != nil && !saved.Deduped && err != nil` → `Discard(saved.RelPath)`

**实测验证（F6 修复在 live 流量上成立）**：
- 步骤 A：上传 99 MiB file + 3 MiB junk → `413 upload exceeds maximum request size`（body cap 触发）
- 步骤 B：`GET /api/v1/assets?limit=2` 不见 sha=61943e3a… 的 DB 行
- 步骤 C：用相同 99 MiB 文件、不带 junk 重传 → `201 Created file_deduped:false`（说明步骤 A 时磁盘对象已被清理）

✅ F6 修复在线上行为正确。

#### 1.2.4 `MAX_UPLOAD_BYTES` 环境变量入口

`cmd/server/main.go:59-70` `uploadLimitFromEnv`：空 → 0；非法 → WARN + 0；`<=0` → 0。`storage.New`（storage.go:76-78）把 `<=0` 替换为 `DefaultMaxBytes` = 100 MiB。
`MAX_UPLOAD_BYTES=1MiB` 实测（先前主 Agent L2 脚本 `S5`）→ 2 MiB 413、512 KiB 201。✅

#### 1.2.5 ⚠️ docker-compose 未声明 `MAX_UPLOAD_BYTES`

`docker-compose.yml:54-60` 的 server 环境块没有 `MAX_UPLOAD_BYTES`，仅暴露 `ADDR / PG_DSN / MEILI_URL / MEILI_KEY / STORAGE_ROOT`。开发/演示栈跑在默认 100 MiB 上限，运维若想调小必须修改 compose 文件而非 .env。文档（`docs/P2-HARDENING-TASK-CARD.md`）将 A2-03 列为完成项，但运维可调性"未在 compose 落地"。详见 §3 finding 3。

---

### 1.3 A2-06 上传端点超时放宽

#### 1.3.1 全局超时组合自洽性

`cmd/server/main.go:30-33`：
```go
readTimeout       = 15 * time.Second // 常规请求的读期限（含请求体）；上传端点自行放宽
readHeaderTimeout = 15 * time.Second // 请求头读期限：慢速攻击防线
writeTimeout      = 30 * time.Second
idleTimeout       = 60 * time.Second
```

**关键性质**：
- `ReadHeaderTimeout=15s` 是慢速攻击主防线——客户端必须 15s 内发完 headers（不含 body）。与 `ReadTimeout=15s` 分离后，慢速大文件上传不会因为 headers 慢而失败；快速 headers + 慢 body 的合法客户端依然受 15s body 读期限（被上传端点放宽到 5min）。
- `WriteTimeout=30s` 仅在 handler 返回后通过 `defer c.rwc.SetWriteDeadline(time.Now().Add(30s))` 设置（`net/http/server.go:992-996`）。在 handler 内被 `allowSlowUpload` 的 `SetWriteDeadline(now+5min30s)` 覆盖。
- `IdleTimeout=60s` 限制 keep-alive 连接的空闲时间，对单请求超时无影响。
- `ResponseController.SetReadDeadline` 的语义（`net/http/responsecontroller.go:80-97`）："Setting the read deadline after it has been exceeded will not extend it." — 即在 handler 启动时如果已经超过 15s，`SetReadDeadline(now+5min)` **不会恢复**。但 handler 启动时机 = headers 已读完，常规场景下远小于 15s，所以实际上不会触发这个限制。

**慢速攻击面的放大风险评估**：当 `allowSlowUpload` 成功（生产 net/http 标准库支持），单上传最坏占用连接 5min + 处理时间。一次只能允许少量并发慢连接（受 GOMAXPROCS / FD limits 约束），不是"放大面"。但若上传端点被并发触发 N 次，会占用 N 个 conn × 5min — 这是**设计意图**（业务需要慢上传）而非"放大攻击"。对 127.0.0.1 only 部署，攻击者必须先获得本地访问。✅

#### 1.3.2 `ResponseController` 用法是否正确

`internal/api/handlers.go:542-547`：
```go
func allowSlowUpload(w http.ResponseWriter) {
    rc := http.NewResponseController(w)
    now := time.Now()
    _ = rc.SetReadDeadline(now.Add(uploadReadBudget))   // 5min
    _ = rc.SetWriteDeadline(now.Add(uploadWriteBudget)) // 5min+30s
}
```

- ✅ `http.NewResponseController(w)` 接受 `http.ResponseWriter` 接口
- ✅ handler 内调用（在 `handleUploadAsset` 第 566 行）
- ✅ 错误被忽略——注释明确说明"非标准 ResponseWriter（如测试用 recorder）会返回错误，忽略即可，此时仍有服务端级超时兜底"——但这有一个语义漏洞（详见 §1.3.4）
- ✅ `now` 在 handler 启动时取，所以 deadline = `handler_start + 5min`，headers 解析时间不计入

#### 1.3.3 非 upload 端点是否仍受 15s 保护

✅ 是。只有 `handleUploadAsset` 调用 `allowSlowUpload`。所有其他端点（list / get / tag / search / readyz / healthz / preview / annotate / create / confirm）不受影响。

单元测试 `TestUploadBudgetsExceedServerDefaults`（upload_test.go:322-332）+ `TestHTTPServerTimeouts`（main_test.go:12-34）锁定口径——后者断言 server-level timeouts ≤ 15s，防"为了修慢上传把全局放大"的回归。

#### 1.3.4 ⚠️ `allowSlowUpload` 静默吞错

```go
_ = rc.SetReadDeadline(now.Add(uploadReadBudget))
_ = rc.SetWriteDeadline(now.Add(uploadWriteBudget))
```

如果未来引入非标准 `ResponseWriter`（自定义 middleware 包装、第三方 logger），`SetReadDeadline` 会返回 `errNotSupported`。**目前没有任何日志或错误暴露**。生产 net/http 的 `*response` 实现 `SetReadDeadline`（server.go:495），所以这不是当前问题，但属于"防御深度"缺口——若有人将来加 middleware 改变 `ResponseWriter` 链，行为会从"放宽"静默退化为"服务端 15s/30s 截断"，且无任何日志提示。

建议：记一次 `log.Printf` 而非静默 `err`（handlers.go:543-546）。详见 §3 finding 4。

#### 1.3.5 实测：5 MiB @ 30 KB/s = 170s → 201 ✅

```text
$ curl --limit-rate 30k -X POST -F "file=@5MiB.txt" http://127.0.0.1:8080/api/v1/assets/upload
HTTP/1.1 201 Created
Content-Length: 480
{"asset":{... "sha256":"74b7dbd…","size_bytes":5242880 ...},"file_deduped":false}
elapsed: 170s
```

慢速上传在 170s（远超 server-level 15s/30s）内成功 201，证明 `allowSlowUpload` 在真实流量上有效（与主 Agent L2 S7 4MiB@100KB/s=41s 一致，仅速率不同）。

#### 1.3.6 反例：python 脚本 1 MiB @ 100 KB/s 失败

```text
$ python3 (4KB chunks + delay, Connection: close)
sent 1049600 bytes in 10.39s
response: HTTP/1.1 500 Internal Server Error {"error":"failed to store file"}
server log: upload save failed: storage: write temp: unexpected EOF
```

`unexpected EOF` 来自 `mime/multipart.Part.Read`（`net/mime/multipart/multipart.go:207`），含义是 body 流在 multipart 关闭边界到达前返回 EOF。可能原因：python 用 `Connection: close` + 块式 send，触发某种服务端关闭行为（未深查——可能与 idle conn 处理或 c.CloseNotify 时序有关）。**这不是 A2-06 的实现 bug**，而是 python 客户端的关闭策略问题。curl（带 `Expect: 100-continue`）在相同速率下成功。**结论**：A2-06 实现正确，覆盖了真实 HTTP 客户端场景。

---

### 1.4 配置面一致性

| 项 | main.go 默认 | docker-compose | 文档 |
|---|---|---|---|
| 监听地址 | `:8080` | `:8080` | 一致 |
| PG DSN | `postgres://partisync:partisync_dev_password@localhost:5432/partisync?sslmode=disable` | `postgres://…@postgres:5432/…`（覆盖到容器名） | 一致（compose 覆盖合理） |
| Meili URL | `http://localhost:7700` | `http://meilisearch:7700`（覆盖到容器名） | 一致 |
| Meili Key | 32字符硬编码 | 同 | 一致 |
| STORAGE_ROOT | `./data/assets` | `/data/assets`（覆盖） | 一致 |
| `MAX_UPLOAD_BYTES` | 未声明（默认 100 MiB） | **未声明** | 文档主张"可调"，但 compose 没暴露 |
| `ReadHeaderTimeout` | 15s | (server 级) | P2-HARDENING-TASK-CARD.md:23 行 **写错了**（声称 `ReadTimeout=5min`，实际代码 15s，正确的语义是"上传端点放宽到 5min"） |
| `ReadTimeout` | 15s | (server 级) | 同上行 |
| `WriteTimeout` | 30s | (server 级) | docs/evidence/l2-integration-p2-hardening.md 与 audit-t6-prime.md 描述一致 |

**文档不一致**：`docs/P2-HARDENING-TASK-CARD.md:23` + `:39` 写"`ReadTimeout 15s → 5min`"和"`ReadTimeout` 15s → 5min（`ReadHeaderTimeout` 承担防慢速攻击）"，**与实际代码不符**——代码仍是 15s，仅由 `ResponseController` 按请求放宽到 5min。其他证据文档（`audit-t6-prime.md`、`l2-integration-p2-hardening.md`）描述是正确的。详见 §3 finding 5。

---

## 2. Findings 分类

| ID | 严重度 | 项 | 位置 | 摘要 |
|---|---|---|---|---|
| F-I1 | **medium** | `storage.Healthy()` 是弱探针 | `internal/storage/storage.go:85-91` | 仅 `os.Stat`，不验证写权限/磁盘空间；read-only `.tmp` 或被替换为 regular file 时返回 healthy；注释 `"not writable-ready"` 名实不符。实测 5 个 case 中 2 个假阳性。 |
| F-I2 | low | `/readyz` 在 compose 部署中无人消费 | `docker-compose.yml`、`internal/api/handlers.go:64` | docker-compose 没有 readiness 概念；server healthcheck 用 `/healthz` 是正确的，但 PG/Meili 中途故障时**没有任何编排器摘流量**——server 仍然 healthy 但功能不可用。`/readyz` 存在但只对 K8s readinessProbe 有用。 |
| F-I3 | low | `MAX_UPLOAD_BYTES` 未在 docker-compose 暴露 | `docker-compose.yml:54-60` | A2-03 让环境变量可调，但 compose 不暴露，运维要调必须改 compose 文件。 |
| F-I4 | low | `allowSlowUpload` 静默吞 `SetReadDeadline`/`SetWriteDeadline` 错误 | `internal/api/handlers.go:542-547` | `ResponseController` 在非标准 `ResponseWriter` 上返回 `errNotSupported`。当前生产 net/http 链无问题，但若中间件改变 `ResponseWriter`，行为从"放宽"静默退化为"服务端级超时截断"，无任何日志提示。 |
| F-I5 | low（文档） | P2-HARDENING-TASK-CARD.md A2-06 描述与代码不符 | `docs/P2-HARDENING-TASK-CARD.md:23, 39` | 声称"ReadTimeout 15s → 5min"，实际代码保持 15s server-level，仅由上传端点按请求放宽。其他文档（audit/l2 报告）描述准确。 |
| F-I6 | info | `TestReady*` 缺 PG 失败路径的单测 | `internal/api/readyz_test.go` | 只测了 `store: nil`、`storage` 路径；PG 失败需 mock 或 live DB。无直接单测覆盖 `s.store.CountAssets` 错误返回路径。 |
| F-I7 | info | `/readyz` 错误串泄露内部拓扑（接受） | `internal/api/handlers.go:113-128` | 与主 Agent 自查 F2 一致；127.0.0.1 only 部署可接受；若经反向代理对外，须改为二元 `{name, ok}` + 服务端日志。 |
| F-I8 | info | docstring 名实不符 | `internal/storage/storage.go:88` | `"not writable-ready"` 暗示会测写但只 Stat；与 F-I1 同根。 |

**blocker**：无。
**high**：无。

---

## 3. 复现/修复建议

### F-I1（medium）

**复现**：
```go
// .audit-tmp/healthy_probe.go（已删除；保留命令）
st, _ := storage.New("/tmp/foo", 0)
os.Chmod("/tmp/foo/.tmp", 0o555)
fmt.Println(st.Healthy()) // → <nil>  （假阳性）
```

**建议**（任选其一）：
- 改名 `Healthy` 为 `RootExists`，匹配真实语义；
- 改为 `os.Stat` 后追加 `tmp, _ := os.CreateTemp(...); tmp.Close(); os.Remove(tmp.Name())` 真写测试；
- 把注释从 `"not writable-ready"` 改为 `"storage root reachable"`。

最小侵入修复：注释校正 + 测试补强（写权限失败 / tmp 是普通文件两种假阳性都需要 explicit 测试）。

### F-I4（low）

**复现**：构造一个返回不实现 `SetReadDeadline` 的 `ResponseWriter` 的中间件，运行上传，验证 `allowSlowUpload` 静默退化。

**建议**（最小侵入）：
```go
func allowSlowUpload(w http.ResponseWriter) {
    rc := http.NewResponseController(w)
    now := time.Now()
    if err := rc.SetReadDeadline(now.Add(uploadReadBudget)); err != nil {
        log.Printf("allowSlowUpload: SetReadDeadline unsupported: %v", err)
    }
    if err := rc.SetWriteDeadline(now.Add(uploadWriteBudget)); err != nil {
        log.Printf("allowSlowUpload: SetWriteDeadline unsupported: %v", err)
    }
}
```

### F-I5（low）

**建议**：把 `docs/P2-HARDENING-TASK-CARD.md:23` 改为：
> A2-06 | `ReadTimeout=15s` 导致慢速大文件上传 400 | 安全 low | 服务端级 `ReadTimeout=15s` 保持，由上传端点用 `http.ResponseController` 按请求放宽到 5min（写 = 5min+30s）；`ReadHeaderTimeout=15s` 单独承担慢速头攻击防线 |

把 `:39` 改为：
> - `cmd/server`：**服务端级** `ReadTimeout` 保持 15s；**上传端点**用 `ResponseController` 按请求放宽到 5min；`ReadHeaderTimeout=15s` 承担防慢速攻击。

### F-I2 / F-I3（low）

**建议**：在 `docker-compose.yml` 的 server service env 块添加 `MAX_UPLOAD_BYTES: ${MAX_UPLOAD_BYTES:-}`（默认空字符串，行为不变；用户可在 .env 设置）。或在文档 README/部署说明中明确 compose 不消费 `/readyz`，建议 K8s 部署时启用 readinessProbe。

### F-I6（info）

**建议**：补一个 `internal/api/readyz_test.go` 用例覆盖 PG `CountAssets` 失败路径。可注入一个返回错误的 `*store.Store` 替身（需要 store 包暴露接口或引入最小 mock 接口）；或者用 `httptest.Server` 模拟一个返回错误的 PG。

---

## 4. 证伪尝试清单（不限于本文档已采纳的证据）

### 4.1 A2-06 慢速上传会不会被服务端级超时截断？

| 尝试 | 结果 |
|---|---|
| `curl --limit-rate 30k` 5 MiB（约 170s） | **201 Created** ✅ 放宽有效 |
| `curl --limit-rate 100k` 5 MiB（约 51s） | **200 OK**（deduped，与前一次同内容）✅ |
| `curl --limit-rate 100k` 5 MiB 文件 + 重复文件名再传 | 200 deduped ✅ |
| python socket `Connection: close` + 4 KB chunks + delay | **500 unexpected EOF**（python 客户端关闭策略，非 A2-06 bug；curl 同速率下成功） |

### 4.2 A2-03 `MAX_UPLOAD_BYTES` 调小真实生效？

| 尝试 | 结果 |
|---|---|
| 102 MiB 文件 vs default 100 MiB cap | **413 `file exceeds maximum size`**（storage cap 先触发）✅ |
| 99 MiB 文件 + 3 MiB junk field vs default 101 MiB body cap | **413 `upload exceeds maximum request size`**（body cap 触发）✅ |
| 99 MiB 文件 + 5 MiB junk field（body ≈ 104 MiB） | 413（同上）✅ |
| F6 闭环：99 MiB+3MiB junk 失败后，再单独上传 99 MiB 同内容 | **201 `file_deduped:false`**（说明前一次的孤儿文件确实被清理）✅ |

### 4.3 A2-02 `/readyz` 真实查询？

| 尝试 | 结果 |
|---|---|
| `curl /readyz`（live 全部 healthy） | 200，3 项 `ok:true` ✅ |
| `Healthy()` 仅 `os.Stat`：read-only `.tmp` | healthy（**假阳性**）❌ |
| `Healthy()` 仅 `os.Stat`：`.tmp` 被替换为 regular file | healthy（**假阳性**）❌ |
| Meili `Health`：`http://127.0.0.1:1`（无监听） | 503 + `meilisearch ok:false` ✅（unit test） |
| Storage 根被删 `.tmp` | 503 + `storage ok:false` ✅（unit test） |
| PG 未在 unit test 中显式覆盖（live 实测因停止 PG 会破坏容器，无法本审计做） | 见 §5 盲区 |

### 4.4 F2 拓扑泄露

| 尝试 | 结果 |
|---|---|
| `curl /readyz` 200 healthy 时 | body 无任何拓扑信息（只有 `name`） |
| `curl /readyz` 推 测会泄露的失败场景（基于 server log `dial postgres on 127.0.0.11:53`） | 响应会包含 `127.0.0.11:53`、`postgres`、`meilisearch:7700`；**不含凭据**；127.0.0.1 only 部署可接受 |

### 4.5 服务端级超时是否被错误放大？

`TestHTTPServerTimeouts` 断言 `ReadTimeout ≤ 15s` 与 `ReadHeaderTimeout ≤ 15s`——任何人未来把 server-level 放大到 30s+ 都会被该测试阻止。✅

---

## 5. 盲区

1. **PG/Meili 中途故障的 live 实测**：本审计未做（停止容器会破坏 live server，且任务要求"禁止删除/重建容器"）。A2-02 PG / Meili 不可达的 /readyz 503 行为仅由 `internal/api/readyz_test.go` 的 unit test 覆盖（Meili 用 `127.0.0.1:1`、Storage 用删 `.tmp`），未在 live 上跑 `docker stop` 流程。
2. **网络命名空间 / docker DNS 行为**：F2 拓扑泄露的具体错误文本是基于既有 server log 推断，未实地在 `/readyz` 故障路径上抓取（live 跑不出）。
3. **极端并发**：未压测 N 个并发慢速上传同时占用的窗口（5 min × N conn）。
4. **TLS / HTTP/2**：server 当前 `127.0.0.1:8080` plain HTTP，`SetReadDeadline` 在 HTTP/2 路径上同样实现（h2_bundle.go:6576），未实测。
5. **`MAX_UPLOAD_BYTES` 极端值**：未实测 `MAX_UPLOAD_BYTES` 大于 1 EiB 之类的边界（int64 不溢出但 `io.LimitReader` 可能 wrap）。
6. **/readyz 在 docker swarm / K8s 场景的 readinessProbe 集成**：本仓库未提供 helm chart 或 K8s manifest；编排端消费 `/readyz` 的证据缺失。
7. **python `Connection: close` 触发 `unexpected EOF` 的根因**：未深查；不是 A2-06 bug 但属于"上传端点对恶意/异常客户端的鲁棒性"盲区。

---

## 6. 复现命令

```bash
cd "/home/acme/Documents/deepseek harness agent/partisync"

# L1
export GOCACHE=/tmp/gocache-audit-probe
export GOFLAGS=-mod=vendor
go build ./... && go vet ./... && go test -race -count=1 ./...

# L2 探针
curl -sS http://127.0.0.1:8080/readyz | python3 -m json.tool
curl -sS http://127.0.0.1:8080/healthz

# 慢速上传（A2-06 真实流量验证，170s）
dd if=/dev/urandom of=.audit-tmp/big.txt bs=1024 count=5120
START=$(date +%s); curl -sS --limit-rate 30k -X POST -F "file=@.audit-tmp/big.txt" http://127.0.0.1:8080/api/v1/assets/upload; END=$(date +%s); echo "elapsed: $((END-START))s"

# Body cap 真实生效（A2-03 / F6）
dd if=/dev/urandom of=.audit-tmp/99m.txt bs=1024 count=101376
python3 -c "import requests; r=requests.post('http://127.0.0.1:8080/api/v1/assets/upload', files={'file':('99m.txt', open('.audit-tmp/99m.txt','rb'),'text/plain')}, data={'junk':'X'*(3*1024*1024)}); print(r.status_code, r.text[:200])"
# → 413 upload exceeds maximum request size
# 再单独上传同 99m.txt：
python3 -c "import requests; r=requests.post('http://127.0.0.1:8080/api/v1/assets/upload', files={'file':('99m.txt', open('.audit-tmp/99m.txt','rb'),'text/plain')}); print(r.status_code, r.text[:200])"
# → 201 file_deduped:false  （说明前一次失败时落盘的孤儿对象已被 F6 修复清理）

# Healthy() 弱探针
cat > /tmp/probe.go <<'EOF'
package main
import ("fmt"; "os"; "path/filepath"; "partisync/server/internal/storage")
func main() {
  d,_:=os.MkdirTemp("","x*"); defer os.RemoveAll(d)
  s,_:=storage.New(d,0)
  os.Chmod(filepath.Join(d,".tmp"),0o555)
  fmt.Println("read-only .tmp Healthy:", s.Healthy()) // → <nil> 假阳性
}
EOF
go run /tmp/probe.go
```

---

## 7. 一句话结论

A2-02/A2-03/A2-06 三项机制在 live 流量与单元测试层面均成立；F6 修复在 live 99 MiB+3 MiB junk 场景下验证有效（孤儿对象被清理）。**唯一 medium finding** 是 `storage.Healthy()` 弱探针（不验证写权限），不影响现有部署但削弱 readiness 信号。其余 findings 均为 low/info 级文档/编排/防御深度问题，无 blocker。
