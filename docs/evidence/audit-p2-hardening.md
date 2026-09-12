# P2 加固批复审报告（2026-09-13）

> **独立性声明（先读）**：本报告 **不是独立复审**。
> 原计划派发 fresh-context 独立审计员，**三次派发全部未产出**（§1）。
> 按本环境既定纪律（连续 2 次无产出即判工具级不可用）切换为主 Agent（实施者、同档模型）
> 自查，因此全部结论均为**实施者自证**，**任务卡 AC-11（独立复审）仍然未满足**。

- **审计对象**：`75fd604`（A2-01…A2-06 实现）+ 复审后修复 `a6a6702`（§7）
- **审计面**：安全/类型一致性、探针与配置、超时语义、证据真实性
- **审计方式**：主 Agent 自查（真实容器 + 真实 HTTP + 真实浏览器），全部命令可复现（§6）
- **结论**：**pass（实施者自查口径）**；6 项 findings —— 1 medium 已修（F6）、1 info 已修（F1）、
  1 low 明确接受（F2）、2 low 待办（F3/F4）、1 info 接受（F5）

---

## 1. 恢复事件（独立审计派发全部失败）

| # | 子代理 id | 审计面 | 结果 | 现象 |
|---|-----------|--------|------|------|
| 1 | `651add05` | 安全/类型一致性 | 失败，无产出 | closing message 只剩注入的状态栏文字（"⏵ 精读上传/预览/就绪处理实现"），无任何审计结论 |
| 2 | `3d1ad820` | 探针/配置/超时/证据真实性 | 失败，无产出 | closing message 是一段关于"工具参数被反复序列化成嵌套 `arguments`"的自我循环文本；子代理卡在工具调用序列化层面，未产出报告 |
| 3 | `4921f90d` | 安全/类型一致性（收紧提示词后重派） | 失败，无产出 | closing message 为空 |

**处置**：按「连续 2 次无产出即视为工具级不可用」的既定策略，改为主 Agent 亲自完成上述四个面，
并在本报告与 SESSION 中显式标注非独立、建议后续补一次独立复审。
自 3 号失败后未再派发（本环境本次共 3 派 0 中，与既往 2/4 失败率一致）。

---

## 2. 逐面结论

| 审计面 | verdict | 依据（实测） |
|---|---|---|
| 安全 / 类型一致性（A2-04 面） | **pass** | 伪装图片声明全部 415；文本家族放行但脚本执行被浏览器实测拦下（§4） |
| 上传解析与生命周期（A2-01/A2-05 面） | **pass（1 medium 已修）** | 畸形/截断输入无 panic、存储零残留；发现并修复 F6 |
| 就绪探针与配置（A2-02/A2-03 面） | **pass** | PG/Meili 双向停机 → 503 且故障项 `ok:false`，`/healthz` 恒 200；`MAX_UPLOAD_BYTES` 真实生效并正确回退非法值 |
| 超时语义（A2-06 面） | **pass** | 自跑 4MiB @100KB/s → 201（41.0s）；不完整请求头仍在 **15.0s** 被关闭（服务端级超时未被放宽） |
| 证据真实性 | **pass（1 处声明原先无实测，已补测）** | L2 脚本在 `75fd604` 上复跑 **53/53 PASS**（与文档一致）；`去重命中对象不删` 原先只有代码级说明，本次已实测 |

**断言判别力**（证据真实性的一部分）：对两条最关键的断言做了**反向验证**——把实现还原成旧写法后，
新增单测确实失败（`orphan objects left after parse failure: 1 files`），
证据文档中原有的「旧实现必然落 `multipart-*` 12,582,912 bytes」亦为反向实验所得。

---

## 3. findings

| ID | 严重度 | 位置 | 问题与证据 | 影响 | 状态 |
|----|--------|------|------------|------|------|
| **F6** | **medium** | `internal/api/handlers.go` `saveUploadedFile` | file 部件**已合法落盘**后，后续部件触发请求体上限/读取错误时直接返回错误，**已落盘对象不清理**。一次性容器（`MAX_UPLOAD_BYTES=4096`）实测：file 部件 4096B + junk 部件 2MiB → HTTP **413**，但 `/data/assets/26/b7e40be0bcf3e6667020b3acf6e07faa17585b21b2936305dd6c9ad3860b15` **存在且无 DB 行** | 无认证下可重复请求累积占盘（与 A2-01 同类风险面）；A2-05 只覆盖了"入库失败"路径 | **已修（`a6a6702`）**：`saveUploadedFile` 改具名返回 + `defer` 清理本次已提交对象（去重命中对象不删）；修复后同场景重跑 → `NO_ORPHAN`（残留文件 0、`.tmp` 0）；单测 + L2 S8 常驻回归 |
| F1 | info | `internal/api/handlers.go` `uploadErrorMessage` | 非 multipart 的 400 文案回显包装后的内部错误串：`{"error":"request is not multipart/form-data: request Content-Type isn't multipart/form-data"}` | 客户端可见文案冗余、暴露 Go 内部措辞 | **已修（`a6a6702`）**：只回 sentinel 文案，并加单测断言 |
| F2 | low（接受） | `internal/api/handlers.go` `handleReady` | `/readyz` 的 `dependencies[].error` 回显内部错误原文，含服务名/端口与 Docker DNS：`count assets: dial tcp: lookup postgres on 127.0.0.11:53: server misbehaving`、`GET /health: Get "http://meilisearch:7700/health": …` | 泄露内部拓扑（不含凭据）。本部署只绑定回环，且就绪探针需要可诊断性 | **接受并记录**：一旦经反向代理对外暴露，须改为通用文案 + 服务端日志留存详情 |
| F3 | low（待办） | `internal/api/handlers.go` `handleCreateAsset` | 元数据端点接受任意 `path` 并落库：`../../etc/passwd`、`/etc/passwd` 均登记 201（只做长度/格式校验） | 读取面已中性化（预览 400 `invalid asset path`、不存在对象 404），故**无路径穿越**；但库内可存在畸形 path 行，且缺失"对象是否存在"校验 | **待办**（先于本批存在，不在 P2 范围）：建议创建时校验 path 形状 + 存在性，或收敛为服务端生成 |
| F4 | low（待办） | `internal/api/handlers.go`（A2-05 引入） | 并发上传**同一新内容**时，若一方 `InsertAsset` 成功、另一方在瞬时错误下走清理分支，清理可能删掉另一方已引用的同内容对象 | 需"同内容并发 + 单边 PG 瞬时失败"窗口，概率低；后果是既有资产预览 500 | **待办**：建议改为不删（留待对账/GC 回收），或删除前二次确认无引用。本次未实测（需故障注入） |
| F5 | info（接受） | `internal/api/handlers.go` `allowSlowUpload` | `SetReadDeadline`/`SetWriteDeadline` 的错误被忽略 | 非标准 ResponseWriter 下静默退回服务端级超时（生产 `net/http` 支持该控制，实际不影响） | 接受（注释已说明） |

---

## 4. 证伪尝试清单（含未果尝试）

**类型一致性与预览（A2-04）**

| 尝试 | 结果 |
|---|---|
| HTML 字节登记为 `image/png` | 预览 **415**（符合预期，未绕过） |
| 同上，声明 `image/svg+xml` / `application/xhtml+xml` | 预览 **415**（`image/svg+xml` 不被 Go 嗅探识别，且非文本家族，故拒） |
| 同上，声明 `IMAGE/PNG`（大小写） | 预览 **415**（大小写不敏感比较未造成误放行） |
| 同上，声明 `text/html` / `text/plain` / `text/xml` / `application/json` | 预览 **200**（文本家族放行），响应带 `X-Content-Type-Options: nosniff` + `Content-Security-Policy: default-src 'none'; style-src 'none'; sandbox` |
| **"放行的 text/html 能否执行脚本"**（真实 Chromium） | **不能**：对照组 `setContent` 同一段 HTML → `window.__xss=1`、`title=EXECUTED`（探针有效）；预览端点 → `httpStatus=200, executed=null, title=CTRL`，控制台原文 `Blocked script execution … because the document's frame is sandboxed and the 'allow-scripts' permission is not set` |
| `mime_type` 带参数 `image/png;charset=utf-8` | 登记阶段即 **400**（`mime_type must look like "type/subtype"`），该形态不经 API 可达；`normalizeMime` 的去参数能力为纵深防御 |
| PNG 字节 + 大小写声明 `IMAGE/PNG`、`Image/Png` | 预览 **200**（相等判定大小写不敏感）；`image/jpeg` 对 PNG 字节 → **415**（内容不符） |
| 越界 `path`（`../../etc/passwd`、`/etc/passwd`、`../data/assets/xx`） | 预览 **400 `invalid asset path`**，未穿越、未泄露内容；形状合法但不存在的路径 → **404** |

**上传解析与生命周期（A2-01/A2-05）**

| 尝试 | 结果 |
|---|---|
| 无 `file` 部件 | **400** `multipart part "file" is required` |
| 两个 `file` 部件 | 取首个，正常返回（第二个被读尽丢弃，未产生第二个对象） |
| 非 multipart（`application/json`） | **400**，服务端存活 |
| 文件名全空白 | **400**（与旧实现一致，未漂移到 415） |
| 截断请求体（socket 发 1000B 后直接关闭，声明 Content-Length 20 万） | 服务端存活（随后 `GET /healthz` → 200）、无 panic、容器存储 `.tmp` 残留 **0** |
| 12MiB 上传前后容器可写层 | `before=4198 bytes after=4198 bytes delta=0 bytes`；容器内 `/tmp` 条目 **0** |
| **PG 停机 + 上传全新内容** | **500** `failed to store asset`，且对象 **ABSENT**（已清理，无孤儿） |
| **PG 停机 + 重复上传同内容（去重命中）** | **500**，但既有对象 **EXISTS**（未误删）；PG 恢复后该资产预览 **200** |
| **file 部件落盘后 junk 部件超限**（F6） | 修复前：**413 + 孤儿对象存在**（缺陷）；修复后：**413 + 无孤儿** |

**探针、配置、超时**

| 尝试 | 结果 |
|---|---|
| `/readyz` 基线 | 200，三项 `ok:true` |
| 停 PG | `/readyz` **503**，`postgres ok:false`（含错误原文）、其余 `ok:true`；`/healthz` **200** |
| 停 Meili | `/readyz` **503**，`meilisearch ok:false`；`/healthz` **200** |
| 恢复两者 | `/readyz` **200**（两次均恢复） |
| `MAX_UPLOAD_BYTES=1MiB` | 2MiB → **413** `file exceeds maximum size`；512KiB → **201** |
| `MAX_UPLOAD_BYTES=abc`（非法） | 服务端 `WARN: ignoring invalid MAX_UPLOAD_BYTES="abc", using default`，5MiB → **201**（回退默认） |
| `MAX_UPLOAD_BYTES=0` | 5MiB → **200**（命中同 sha 已登记资产；证明未被当成 0 上限拒绝） |
| 自跑慢速上传 4MiB @100KB/s | **201**，用时 **41.0s**（远超服务端常规 15s/30s） |
| 不完整请求头（socket 只发一半、不发结尾 CRLF） | 服务端 **15.0s** 后关闭连接（`ReadHeaderTimeout` 未被放宽） |
| 超大请求头字段（200KB） | **15.0s** 内结束 |

**回归抽查**

| 检查 | 结果 |
|---|---|
| SPA 深链接 `/assets` | **200**，`Content-Type: text/html`，`<!doctype html>`（回退正常） |
| `/api/nope` | **404** |
| `GET /api/v1/assets` 键 | `limit / offset / results / total`（与本批改动前一致） |
| `GET /api/v1/assets/{id}` 键 | `asset` |
| T7 L3 浏览器闭环 | **8/8 PASS**，`consoleErrors=[]`、`badResponses=[]` |
| L2 脚本复跑 | `75fd604` 上 **53/53 PASS**（与文档一致）；修复后含新 S8 段 **60/60 PASS** |

**未果/无判别力尝试**：未能构造出绕过 `MatchedContentType` 的伪装组合；
`image/svg+xml` 内联路径未打通（登记声明与嗅探结果必不一致 → 415）；
F4 的并发窗口未做故障注入，故**未实测**（仅代码推演）。

---

## 5. 盲区与未验证项（诚实清单）

1. **无独立复审**：本报告全部结论出自实施者，AC-11 未满足；关键结论（尤其 F6 的修复与
   "无存储型 XSS"）建议由 fresh-context 审计员复核一次。
2. **F4 未实测**：并发同内容 + 单边 PG 失败的窗口未做故障注入，结论为代码推演。
3. **公网暴露面未评估**：上传/元数据端点无认证（A2-07 仍未关闭），本报告只在本机回环环境验证。
4. **规模边界**：`/tmp` 零落盘的证明规模为 12MiB（超过旧实现 8MiB 落盘阈值）；慢上传最长实测 41s，
   未逼近 5min 读预算上限。
5. **未做**：模糊测试、覆盖率统计、长时间稳定性/并发压测、镜像按 digest 固定（A2-09）。
6. **数据面**：本次审计在开发库/卷内留下测试资产（`selfaudit-*`、`p2-*`、一次性容器遗留的悬空行），
   清理需要用户批准。

---

## 6. 复现命令

```bash
cd "<repo>"   # /home/acme/Documents/deepseek harness agent/partisync

# L1
GOCACHE=/tmp/go-cache GOFLAGS=-mod=vendor go build ./... && go vet ./... && go test -race -count=1 ./...

# L2（含 S8：解析期孤儿回归）
bash scripts/l2-p2-hardening.sh                 # 期望 PASS=60 FAIL=0 RESULT=PASS

# L3 浏览器闭环（静态托管）
L3_BASE=http://127.0.0.1:8080/ L3_SHOTS=docs/verification/p2-hardening node scripts/l3-mcd-acceptance.mjs

# 反向验证（新单测有判别力）：把清理 defer 还原为旧写法后
GOCACHE=/tmp/go-cache GOFLAGS=-mod=vendor go test -count=1 ./internal/api/ \
  -run TestUploadCleansUpObjectWhenBodyCapTripsAfterFilePart   # 期望 FAIL：orphan objects left
```

F6 的最小复现（修复前形态，一次性容器）：

```bash
mkdir -p /tmp/x-data && chmod 777 /tmp/x-data
docker run -d --name x --network partisync_default -p 127.0.0.1:8097:8080 \
  -e MAX_UPLOAD_BYTES=4096 -e STORAGE_ROOT=/data/assets -e WEB_DIST= \
  -v /tmp/x-data:/data partisync-server:local
# 构造 multipart：file 部件 4096B（= 上限，可落盘）+ junk 部件 2MiB（超 body 上限 1MiB+4096）
# POST /api/v1/assets/upload → 413；随后检查 /data/assets/<sha[0:2]>/<sha[2:]> 是否存在
```

---

## 7. 复审后修复（`a6a6702`）

| 项 | 内容 |
|---|---|
| 代码 | `saveUploadedFile` 改具名返回 + `defer` 清理：本次请求出错时 `Discard` 已提交对象；去重命中对象不删。顺带收口 F1 文案 |
| 单测 | `TestUploadCleansUpObjectWhenBodyCapTripsAfterFilePart`（对照组证明"先落盘"、实验组证明"再清理"、断言存储零残留） |
| L2 常驻回归 | 脚本新增 **S8** 段（一次性容器 1MiB 上限）：对照 201、实验 413、失败对象不存在、先前对象未被误删、残留文件数 1、`.tmp` 0 |
| 反向验证 | 还原旧写法 → 新单测 FAIL（`orphan objects left after parse failure: 1 files`） |
| 复验结果 | L1 三绿（16 个新用例）；L2 **60/60 PASS**；L3 **8/8 PASS**（截图用修复版镜像重采）；F6 场景重跑 `NO_ORPHAN` |

**门禁影响**：本批因此修复了 1 个 medium 缺陷，但**独立性缺口未变**——
`75fd604` 与 `a6a6702` 均为实施者自证；`audit-t6-prime.md` 的原始 `verdict=fail`
与 §8 门禁判定不因本报告改变。建议下一步：派 fresh-context 审计员复核本报告（重点 F6 修复面与
"文本家族放行"判定），或先转 C2-7/C2-9 交付。
