# P2 加固批独立安全审计（fresh-context）

- 审计员：独立 fresh-context 安全审计子代理（无历史会话，仅依据仓库现状）
- 审计对象：`75fd604`（A2-01…A2-06）与 `a6a6702`（F6 修复），基线 `b03ff25`
- 审计时间：2026-09-13 00:42–00:46 UTC（live 环境：compose 三容器 healthy，server @ 127.0.0.1:8080）
- 环境注意：审计期间观察到**并行测试流量**（见「盲区」）；仓库工作区在审计结束时仍为 `HEAD=8644b55`、`git status` 除既有未跟踪 `.audit-tmp/` 外干净，未观察到仓库文件被并发改动。

---

## Verdict

**pass（有保留）** —— 四个重点核查项的主干均成立：上传确为真流式、F6 解析期清理有效且经对照式单测验证、415 内容-类型强制校验实测生效、大小上限/扩展名白名单/路径防护完整。但发现 1 个 medium 级缺陷（A2-05 回读失败路径会删除**已被新 DB 行引用**的对象，制造悬空行）与 1 个 medium 级 DoS 面（multipart 部件头解析无独立上限），建议合入后续小批修复。

---

## 逐项核查记录

### 1. A2-01 流式上传

**结论：确为流式，无全量缓冲残留。**

- `internal/api/handlers.go:663` `r.MultipartReader()`（不经 `ParseMultipartForm`，无 `/tmp` 副本）；`handlers.go:692` 文件部件直接 `s.content.Save(part, clientName)`。
- `internal/storage/storage.go:148-156`：`bufio.NewReaderSize(r, 64KiB)` + 流式 `io.Copy(io.MultiWriter(tmp, hasher), io.LimitReader(br, maxBytes+1))`，写 `Root/.tmp/upload-*` 后 rename，全程只有 64KiB 前缀驻留内存。
- 非文件部件 `handlers.go:678-685` `io.Copy(io.Discard, part)` 顺序读尽丢弃，无驻留。
- 部件头伪造 Content-Type 无效：`Save` 忽略部件头，MIME 全部来自 `storage.sniffMime`（`storage.go:177,248-257`）。
- 实测：正常上传 33B → 201 + `file_deduped:false`；同文件再传 → `file_deduped:true, existing:true` 返回既有资产（内容寻址去重正确）。
- 实测超多部件/超长部件头：构造「file 部件 + 带 8MB 自定义头的后续部件 + 终止边界」共 8,388,749 字节，42ms 内 201 落盘，`/data/assets/.tmp` 无新增残留。**但该结果同时证明部件头解析不受 1MiB 的 net/http 请求头限制约束**（见 Finding S3）。
- 中途断连实测：30MB `--limit-rate 8M` 3 秒后掐断 → `.tmp` 无新增、`assets` 表无 `original_name='big.bin'` 行（清理链路：`Save` 的 copyErr → defer 删 temp；`saveUploadedFile` 的 saved==nil）。

### 2. A2-04 预览 MIME 一致性

**结论：415 强制校验实测生效；文本家族放行判定实现正确、未见逐字节误拒回归；但 HTML 可经 .md 上传入库并内联预览，唯一脚本防线是 CSP sandbox（Finding S5）。**

- `handlers.go:861-868`：预览先 `io.ReadFull` 64KiB 前缀，`storage.MatchedContentType(head, asset.MimeType)` 不一致即 415（实测复现：用 metadata 路由以全新 sha256 指向纯文本对象、声明 `image/png` → `{"error":"content does not match declared mime type"}` + 415）。
- 文本家族判定 `internal/storage/image.go:39-60`：octet-stream→放行（nosniff+CSP 兜底）、逐字一致→放行、`isTextualMime`（text/* 或 application/json）双方皆文本→放行、其余→拒绝。规则内部自洽；`sniffMime` 与 `MatchedContentType` 共用 `normalizeMime`，双向一致。`image/svg+xml` 声明与 text/xml 嗅探不同家族 → 拒绝，SVG 内联不可达。
- **文本家族可绕过性**：`http.DetectContentType` 对 `<html>`/`<script>` 开头内容返回 `text/html`。实测：上传 `<script>alert("xss")</script>…` 文件命名为 `evil.md`（.md 在白名单）→ 入库 `mime_type:"text/html"` → `GET /preview` **200，Content-Type: text/html 内联**，响应头为 `X-Content-Type-Options: nosniff` + `Content-Security-Policy: default-src 'none'; style-src 'none'; sandbox`。即 A2-04 的「一致性」本身无法拦截——上传侧嗅探结果与 DB 声明天然一致；拦截完全依赖 CSP sandbox 一条头（脚本禁用、opaque origin）。风险降级为沙箱内 HTML 钓鱼页，但纵深只有一层（见 S5）。
- 尝试经 metadata 路由伪造声明绕过：sha256 冲突去重挡住了「同一内容换声明」（ON CONFLICT DO NOTHING 返回 existing）；用全新 sha256 绕过去重后，MatchedContentType 仍按嗅探拒绝/放行，无法让 text 内容以 image/* 内联。
- 上传与 metadata 双路由的 MIME 判定使用同一套嗅探，未见家族判定被构造内容骗过的新路径。

### 3. A2-05 + F6 孤儿对象清理

**结论：F6 修复本身完整且经对照式单测验证；但发现回读失败路径的清理会删除已被引用的对象（S1，medium），以及并发去重竞态可致共享对象被误删（S2，low）。**

- F6 主路径（`handlers.go:653-661` defer + `saved.Deduped` 排除）：file 部件已提交、后续部件触发 `MaxBytesError` → defer `Discard`；去重命中对象不删（正确——不属于本次请求）。单测 `internal/api/upload_test.go:368-428+`：对照组证明「先落盘」、实验组断言 413 + 存储零残留 + 先前对象未误删，测试设计有效。
- 失败路径清单核查：
  - 入库失败（`handlers.go:592-602`）：`!saved.Deduped` 才 Discard ✓；
  - 客户端断连 → r.Context() 取消 → InsertAsset 报错 → 走入库失败路径 ✓（实测断连无残留）；
  - body 上限在文件部件内触发 → `Save` copyErr → temp 由 `Save` 自身 defer 删除（`storage.go:141-146`）✓；
  - `Rename`/`MkdirAll` 失败 → temp 删除 ✓；Discard 幂等（`os.IsNotExist` 视为成功，`storage.go:96-105`）✓。
  - **回读失败（`handlers.go:624-635`）：`inserted==true`（`store.InsertAsset` 以 `RowsAffected>0` 判定，行已提交，`internal/store/store.go:53-71`）之后 `GetAssetBySHA256` 出错或返回 nil 时执行 `Discard(saved.RelPath)` —— 删除的是刚写入 DB 行所引用的对象**，DB 行保留 → 悬空行，预览 404。ctx 为 20s 超时，DB 瞬断/超时即可真实触发。正确做法是删 DB 行或保留文件。→ Finding S1。
  - Meili 失败不清理（行+文件均需保留）✓；响应写失败不清理 ✓。
- 去重命中不误删共享对象：单请求层面正确（`Deduped` 排除）；**并发层面**：`storage.go:180-193` Stat→Rename 存在竞态——并发同内容上传中一方 insert 失败会 Discard 掉另一方（或更早成功行）正引用的对象。→ Finding S2。

### 4. 上传大小上限 / 白名单 / 路径防护

**结论：完整，未发现绕过。**

- 请求体上限 `handlers.go:569` `http.MaxBytesReader(w, r.Body, MaxBytes()+1MiB)`（`maxUploadOverheadBytes` 见 `handlers.go:523`）；`MAX_UPLOAD_BYTES` 经 `cmd/server/main.go:59-70` 生效（当前 compose 未设置 → 默认 100MiB）。`MaxBytesError` 正确映射 413（`handlers.go:706-711`，errors.As 可穿透 fmt.Errorf %w 包装）。
- 文件自身上限 `storage.go:152-165` LimitReader maxBytes+1 → `ErrTooLarge`；空文件 `ErrEmptyFile`→400。
- 扩展名白名单 `storage.go:45-56,131-134`：`ExtOf` 取 base 后缀并 ToLower；实测 `.exe` → 415（文案含完整白名单）。
- 路径防护：存储名由 sha256 派生（`storage.go:167-169`），客户端名仅取扩展名与展示；`Resolve`（`storage.go:196-214`）拒绝绝对路径/`..` 前缀并做 Join 后前缀复核；`Open`（`217-237`）打开后 `EvalSymlinks` 复核不越界；`Discard` 复用 `Resolve`。`displayName`（`handlers.go:754-771`）去目录成分与控制字符；`Content-Disposition` 用 `%q` 转义，无头注入。

### 5. 额外面（自选）

- **A2-02 /readyz**（`handlers.go:101-144`）：PG/Meili/存储逐项 3s 超时、任一失败 503；实测三依赖 ok。小瑕疵：依赖错误串原样回显给匿名调用者（本地绑定，info 级）。
- **A2-06 超时放宽**：`allowSlowUpload`（`handlers.go:539-547`）经 ResponseController 仅放宽本请求读 5min/写 5min30s；服务端常规 15s/15s/30s 保持（`cmd/server/main.go:24-34`）。副作用：上传端点单连接最长可挂 5min（有上界，info）。
- **F1 收口**：非 multipart 上传实测返回固定文案 `request is not multipart/form-data`，无内部错误串回显 ✓。
- **Meilisearch filter 注入**：`internal/search/meili.go:142-151` 用 `%q` 包值；实测注入探针 `text/plain" OR resource_type = "image` 与 `image" OR mime_type != "` 均返回 0 hits（未成功、未报 5xx）。即便注入成功，无认证单租户架构下影响有限。
- **质量门**：`export GOCACHE=/tmp/gocache-audit-sec2 && go build ./... && go vet ./... && go test -race ./...` 全部退出码 0。
- **无认证 API**：全部端点匿名可用（架构既定，非本批引入，info 记录）。

---

## Findings

### S1 · medium —— A2-05 回读失败路径删除已入库对象，制造悬空 DB 行
- 位置：`internal/api/handlers.go:624-635`（配合 `internal/store/store.go:53-71`）。
- 复现（推演 + 代码证据）：上传新内容 → `InsertAsset` 成功（inserted=true，行已提交）→ `GetAssetBySHA256` 因 ctx 20s 超时/DB 瞬断返回 err → `Discard(saved.RelPath)` 删除对象 → `assets` 行仍指向已删对象 → 预览 404、内容永久丢失。与「孤儿文件」相比方向相反但同属一致性缺陷。
- 建议：该路径不得 Discard；改为「回读失败→按 sha256 删除 DB 行（或保留文件、仅告警）」。

### S2 · medium —— multipart 部件头解析无独立上限，头部内存≈整个请求体上限
- 位置：`internal/api/handlers.go:669`（`mr.NextPart` 内部头部解析）；净上限 `handlers.go:569`。
- 复现（实测）：file 部件后随 8MB 自定义头部件 → 201 接受、42ms。默认配置下单请求可令服务端把最多 ≈101MiB 的部件头解析进内存 map（net/http 的 1MiB 请求头限制不覆盖 body 内部件头）；并发请求线性放大 → 内存型 DoS。
- 建议：对每个部件头字节/行数设硬上限（如 64KiB，超出即断开），或限制部件数量；`MAX_UPLOAD_BYTES` 调小时此面自动收窄但默认配置敞开。

### S3 · low —— HTML 嗅探内容可经 .md/.txt 上传并内联预览，脚本防线仅 CSP sandbox 一条头
- 复现（实测）：上传 `<script>…</script>` 命名 `evil.md` → 入库 `text/html` → `GET /assets/{id}/preview` 200 内联 `text/html` + `Content-Security-Policy: … sandbox`。CSP sandbox（无 allow-scripts）按规范禁脚本，实测浏览器行为未验证（盲区）；若该头日后回归丢失即成存储型 XSS。
- 建议：上传侧对文本类扩展名嗅探出 text/html 时拒绝或强制 `Content-Disposition: attachment`；预览侧对 text/html 家族一律 attachment。

### S4 · low —— 并发同内容上传的去重竞态可误删被引用对象
- 位置：`internal/storage/storage.go:180-193`（Stat→Rename 无互斥）× `handlers.go:596-600`。
- 复现（推演）：并发上传同一新内容，两方 Stat 均未命中 → 双双 Rename 提交（内容相同无害）→ 任一方 insert 失败即 Discard 该对象 → 另一方已提交的行悬空。窗口小、触发概率低。
- 建议：insert 失败的清理前按 sha256 重查 DB 行存在性；或对同 sha256 提交加互斥/引用计数。

### S5 · low —— `.tmp` 临时目录无启动/定期清扫
- 位置：`storage.go:136-146` 仅靠单请求内 defer 删除；进程崩溃/kill -9 后残留临时文件永久留存（本审计在 `.tmp` 观察到的增长文件经核实为并行测试的在途上传，非泄漏——但崩溃残留无任何回收机制）。
- 建议：启动时及/或后台周期清理 `.tmp` 中超龄（如 >1h）条目。

### S6 · info —— /readyz 向匿名调用者回显依赖错误串（PG/Meili/存储错误详情）；服务仅绑 127.0.0.1，风险受限。
### S7 · info —— API 全端点无认证（非本批引入，架构既定）。
### S8 · info —— `Discard` 失败仅记日志，无重试与兜底清扫（与 S5 叠加时孤儿无法自愈）。

---

## 证伪尝试清单

1. **试图攻破流式声明（A2-01）**：8MB 部件头 + 多部件 + 伪造部件 Content-Type + 海量字段——未发现全量缓冲/落盘放大；但证实部件头无上限（升级为 S2）。
2. **试图让 text 内容以 image/* 内联（A2-04）**：先被 sha256 去重挡住，用全新 sha256 绕过去重后仍被 MatchedContentType 415 —— 未成功。
3. **试图让 HTML 入库并内联（A2-04 规避）**：`evil.md` 上传**成功**入库 text/html 并 200 内联（CSP sandbox 拦脚本）——攻破一致性判定本身，依赖响应头兜底（S3）。
4. **试图 SVG 内联**：`image/svg+xml` 声明 vs text/xml 嗅探不同家族 → 415；无 .svg 白名单 —— 未成功。
5. **试图 Meili filter 注入**：两种 OR 探针均 0 hits —— 未成功。
6. **试图路径穿越/符号链接**：代码审查 Resolve/Open/Discard 前缀+EvalSymlinks 校验，未发现绕过；未在容器内构造 symlink 实测（只读纪律）。
7. **试图触发断连泄漏**：30MB 中途掐断 → `.tmp` 与 DB 均无残留 —— 未成功（清理有效）。
8. **试图 413-after-file-part 泄漏（F6 场景）在 live 环境复现**：默认上限 101MiB 不便实测，改以单测 `TestUploadCleansUpObjectWhenBodyCapTripsAfterFilePart`（对照组/实验组/零残留断言）验证 —— 修复成立。
9. **试图让去重命中被 Discard 误删（单请求层）**：defer 与 insert 失败路径均有 `!saved.Deduped` 守卫 —— 未成功（并发层除外，S4）。

## 盲区

1. 未验证浏览器对 `CSP sandbox` 的实际拦截行为（无浏览器环境），S3 的兜底强度依据 CSP 规范推断。
2. F6 场景（413-after-file-part）未在容器内以真实网络复现；依赖单测证据。
3. 符号链接 TOCTOU（Open 与 EvalSymlinks 之间）未实测。
4. S1/S4 的并发/瞬断窗口未实测复现，评级基于代码推演（行已提交的证据是确凿的）。
5. **环境活性**：审计期间 `.tmp` 中观察到持续增长的在途上传（主 Agent 并行测试），DB 行数（191）/文件数（106）/distinct path（128）计数不含快照一致性语义；其中 distinct path > 文件数的现象与 metadata 路由允许不存在的 path 有关，不能归因于 S1。所有 live 观测应以时序描述为准。
6. 未审计 worker/Meili 索引写入的内容安全（超出本次四项重点，仅顺带确认 sort 白名单）。
