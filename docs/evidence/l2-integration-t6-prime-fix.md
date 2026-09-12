# T6′ 阻断项修复与回归证据（C2-1 / C2-2 / C2-3 / C2-4 / C2-6）

- **日期**: 2026-09-12
- **上游输入**: `docs/evidence/audit-t6-prime.md`（T6′ 独立复审，verdict = **fail**：2 blocker + 4 high）
- **修复范围**: 前端与检索契约面的 blocker/high 项，外加同一缺陷族的 C2-6（列表命中字段不完整）
- **证据分级**: **L1 已跑 + L2 已实测 + L3 级浏览器回归已跑**（真实容器 + 真实 Chromium）
- **模型档位**: 主 Agent `vectide/glm-5.3`（strong 档）；**本文件是实施者自证，不是独立复审**

---

## 1. 修复清单

| # | finding | 变更 | 文件 |
|---|---|---|---|
| 1 | **C2-1** blocker<br>响应契约失配 | 前端改读 `results`；类型同步为 `results`。后端字段名保持不变（`asset`/`job`/`jobs` 等端点本就一致，全端点扫描确认只有此处错位） | `playground/src/lib/api.ts`、`playground/src/pages/assets.tsx` |
| 2 | **C2-2** blocker<br>`sort` 必然 502 | `EnsureIndex` 补 `sortableAttributes`；新增 `SortableFields` 白名单常量，设置项与白名单共用同一来源 | `internal/search/meili.go` |
| 3 | **C2-3** high<br>仅 `sort` 被静默丢弃 | `sort` 纳入 Meili 分支条件；新增 `ValidateSort()` 白名单校验（字段+方向），非法值 **400**（此前是"静默忽略"或"502"两种错误行为） | `internal/api/handlers.go`、`internal/search/meili.go` |
| 4 | **C2-4** high<br>PG 路径缺 `total` | 新增 `Store.CountAssets()`；PG 路径响应补 `total`，两条路径响应形状统一为 `results/total/limit/offset` | `internal/store/store.go`、`internal/api/handlers.go` |
| 5 | **C2-6** medium<br>命中字段不完整 | 索引文档补 `path` 与 `created_at`（仅展示用，不参与检索）；前端日期渲染改为缺失即 `—`；详情弹窗回源 `GET /assets/{id}` 取权威记录 | `internal/search/meili.go`、`playground/src/pages/assets.tsx` |
| 6 | C2-8 medium<br>单测零断言力 | 重写/新增有断言力的测试：断言**真实发出的请求体**与**响应键**，不再使用恒返 200 的哑 mock | `internal/search/meili_test.go`、`internal/api/list_assets_test.go`（新增） |

**新增测试清单**（8 个用例 + 1 个表驱动）：

| 测试 | 覆盖 finding |
|---|---|
| `TestSearchOptionsFormatting`（升级：断言 filter/sort/q 请求体） | C2-2/C2-8 |
| `TestSearchOmitsSortWhenEmpty` | C2-2 |
| `TestEnsureIndexConfiguresSortableAttributes`（断言 PATCH settings 含 sortableAttributes 且与白名单一致） | **C2-2 根因** |
| `TestValidateSort`（11 例表驱动，含 `created_at:desc` 必须被拒） | C2-3 |
| `TestListAssetsMeiliContract`（断言响应键 `results`/`total`，且不再返回 `assets`） | **C2-1** |
| `TestListAssetsSortOnlyReachesMeili`（仅 `sort` 必须抵达检索后端） | C2-3 |
| `TestListAssetsRejectsInvalidSort`（400 且不触达检索后端） | C2-3 |
| `TestListAssetsWithoutDepsFailsCleanly`（保持 503 降级语义） | 回归保护 |

---

## 2. L1 证据

```
$ export GOCACHE=/tmp/go-cache-fix2            # 默认 ~/.cache/go-build 会被文件沙箱拒绝 → 假绿
$ gofmt -l internal/search/meili.go internal/api/handlers.go \
          internal/api/list_assets_test.go internal/store/store.go internal/search/meili_test.go
（无输出 = 本次改动文件均已格式化）
$ go build ./...    → exit 0
$ go vet ./...      → exit 0
$ go test -race -count=1 ./...
ok  partisync/server/internal/api      1.022s
ok  partisync/server/internal/search   1.015s
ok  partisync/server/internal/storage  1.021s

$ cd playground && npx tsc --noEmit   → exit 0
$ cd playground && npm run build      → ✓ built in 1.57s
```

> 遗留：`internal/models/models.go`、`internal/worker/worker.go`、`cmd/bench/main.go` 在**本次改动之前**就不满足 `gofmt`。不在本次范围，未顺手重排（避免无关 churn），登记为低优先残留。

---

## 3. L2 证据（真实容器 + 真实 HTTP）

容器已用修复后的代码重建：`docker compose up -d --build server` → 三容器 `Up (healthy)`，`GET /healthz` = `{"status":"ok"}`。

### 3.1 C2-2 / C2-3：排序链路（修复前 502 / 静默忽略）

Meili 设置重建任务：`task 227 settingsUpdate` → `succeeded`，细节 `sortableAttributes: ['name','size_bytes']`；设置复查：

```
$ curl -s -H "Authorization: Bearer …" 127.0.0.1:7700/indexes/assets/settings
filterable: ['mime_type', 'resource_type']
sortable:   ['name', 'size_bytes']
```

| 请求 | 修复前 | 修复后 |
|---|---|---|
| `?resource_type=image&sort=size_bytes:desc&limit=4` | **502** `search backend unavailable` | **200**，sizes `[100000, 13058, 13057, 4104]`（降序 ✅），`total=1000` |
| `?sort=size_bytes:asc&limit=4` | 200 但**顺序与不排序一致**（参数被丢弃） | **200**，sizes `[9, 22, 31, 45]`（升序 ✅） |
| `?sort=name:desc&limit=3` | 同上 | **200**，names `['small100k.png','regress.txt','readme.txt']`（降序 ✅） |
| `?sort=bogus:zzz` | 200 静默忽略 | **400** `sort field "bogus" is not sortable (allowed: name, size_bytes)` |
| `?sort=size_bytes` | 200 静默忽略 | **400** `sort must look like "field:asc" or "field:desc"` |
| `?sort=size_bytes:up` | 200 静默忽略 | **400** `sort direction must be asc or desc` |

### 3.2 C2-1 / C2-4：响应契约（两条路径统一）

```
$ curl '127.0.0.1:8080/api/v1/assets?q=hero&limit=2'
HTTP 200  keys=['limit','offset','results','total']  total=1   n=1        # Meili 路径
$ curl '127.0.0.1:8080/api/v1/assets?limit=3'
HTTP 200  keys=['limit','offset','results','total']  total=15  n=3        # PG 路径（修复前无 total）
```

### 3.3 C2-6：索引字段

```
$ curl -F "file=@/tmp/pw-smoke.png" 127.0.0.1:8080/api/v1/assets/upload     # 全新文件
$ curl '127.0.0.1:8080/api/v1/assets?q=pw-smoke&limit=1'
fields: ['created_at','id','mime_type','name','path','resource_type','sha256','size_bytes']
created_at: 2026-09-12T13:09:33.395415Z
path:       7c/7a765b4e…d88f
```

> 对照：修复前 App 上传资产的命中只有 `[id, mime_type, name, resource_type, sha256, size_bytes]`（无 `created_at`/`path`），而压测语料 `bench-*` 文档因生成器写全字段而"看起来正常"——这正是 C2-6 在两个文档来源上表现不一致的原因。

---

## 4. L3 级浏览器回归（真实 Chromium，可复现）

- 脚本（已入库）：`scripts/browser-regression.mjs`
- 运行：`cd playground && npm run dev -- --port 5201 --strictPort`，然后
  `L3_BASE=http://127.0.0.1:5201/ L3_SHOTS=/tmp/l3-final node scripts/browser-regression.mjs`
- 截图：`docs/verification/t6-prime-fix/`（本会话产物；T7 将另建目录）

```
script exit=0
PASS | 列表渲染非空（C2-1）              | cards=15
PASS | 无空态（C2-1）                    | emptyState=0
PASS | 总数非 0（C2-4）                  | 共 15 个资产
PASS | 搜索命中收敛                      | cards=1
PASS | resource_type=image 过滤生效      | cards=24
PASS | 表格视图渲染行                    | rows=24
PASS | 详情弹窗保持打开                  | 资产详情 / bench-000000000
PASS | 详情无 Invalid Date（C2-6）       | —
PASS | App 上传资产卡片无 Invalid Date（C2-6） | IMAGE | 12.8 KB | hero.png | image/png | —
PASS | App 上传资产详情回源到权威路径（C2-6）  | 路径 | 88/1ffbcaafc212e4…
PASS | 无 console 错误                   | []
PASS | 无 4xx/5xx 请求                   | []
```

**修复前**的浏览器实测（同一会话，作为对照）：卡片文本为 `IMAGE | 12.8 KB | hero.png | image/png | Invalid Date`，详情"路径"为空。

---

## 5. 修复过程中发现的自身回归（诚实记录）

第一版 C2-6 修复让详情弹窗无条件回源 `GET /api/v1/assets/{id}`，结果在**压测语料**上暴露新缺陷：

- `bench-000000000` 等 id **不是 UUID**，且不存在于 PG → 回源得到 **400 `invalid asset id`**；
- 且 `res.asset` 为 `undefined` 时会把 `selectedAsset` 置空 → **弹窗被关闭**（比修复前更糟）。

浏览器回归脚本捕获了它（`badResponses` 记录到该 400、`consoleErrors` 出现一条错误）。处置：仅对 UUID 形态 id 回源，且仅在确实取到记录时覆盖状态（`assets.tsx` 的 `UUID_RE` + 条件覆盖），非 UUID 资产直接使用列表项字段。修复后上表 12 项全 PASS，`badResponses` 与 `consoleErrors` 均为空。

> 教训：**"列表项字段不全 → 回源"这类修复必须用真实数据源（含非 UUID 语料）回归**；仅用 App 上传资产测试会漏掉它。

---

## 6. 尚未处置的残留（需用户决策，本阶段未动手）

| ID | 严重度 | 项 | 建议 |
|---|---|---|---|
| C2-5 | high | Meili `pagination.maxTotalHits=1000` 硬墙：`offset≥1000` 命中 0，`total` 被截到 1000 | 需权衡内存/延迟后设置更大 `maxTotalHits`，或改前端分页口径（游标/分段）。**与"百万资产可检索"口径直接相关** |
| C2-7 | medium | MIME 过滤维度未实现（前端只有 4 个 resource_type 按钮） | 补 MIME 下拉，或按任务卡 §3 修口径 |
| C2-9 | low | 排序无前端入口（`sort` 仅 API 可用） | 补排序 UI（后端已可用） |
| A2-01…A2-06 | medium/low | multipart 暂存绕开资产卷、`/healthz` 纯 liveness、上限硬编码、后缀白名单不校验内容、孤儿文件无 GC、慢速上传 15s 超时 | 见 `audit-t6-prime.md` §3 |
| B-01/B-02/B-06 | medium/low | 报告 p95 含 ~40ms 客户端传输伪影；证据自称"端到端"实为直连 7700；文档数字过期（LMDB 1408MB vs 实测 1755MB）、标题超出证据 | **修正证据文档口径**（独立小任务，不得静默改判） |

**T7 前提缺口（决策待定，`docs/SESSION.md` §5 闭环）**：人工分类/文本标签、人工确认 AI 建议并写入标注、文件预览三项在代码中不存在（`tags`/`asset_tags` 实测 0 行）。

---

## 7. 门禁状态

- 本文件证明：T6′ 报告的 **2 个 blocker（C2-1/C2-2）与 3 个 high（C2-3/C2-4、C2-5 除外）已修复并有 L1+L2+L3 级回归证据**，C2-6 一并修复，C2-8 的测试空洞已补。
- **T6″ 独立复核已执行：verdict = pass（6/6 项成立）**，复核员自行复现了排序真序、契约键、400 校验与测试断言力，并尝试证伪未果。完整裁决见 `docs/evidence/audit-t6-prime.md` §11。
- **仍未通过门禁**：
  1. **C2-5（high）仍打开**——Meili 路径 `total` 被截到 1000，`offset>1000` 静默返回空列表 + HTTP 200（T6″ 独立确认）。需用户决策（调高 `maxTotalHits` / 改口径 / 游标分页）；
  2. **T7 的 L3 闭环前提缺口未决**——「人工分类/文本标签」「人工确认 AI 建议并写入标注」「预览」在代码中不存在。
- 因此当前状态记为：**T6′ 阻断项修复完成且经独立复核通过；门禁因 C2-5 与 T7 前提缺口不放行**（原始 `verdict=fail` 不变）。

### 7.1 独立复核之后的改动声明

T6″ 复核返回后，`internal/search/meili.go` 中 `SortableFields` 的**注释**按其发现（T6″-2，注释与实现不符）被改写为准确表述：说明 `created_at` 已写入文档但因时间串格式不统一而不开放排序，故由 `ValidateSort` 明确拒绝（400）。

- 该改动**仅注释，无行为变化**；改动后已重新执行 `go build` / `go vet` / `go test -race`（`internal/search`、`internal/api` 均 ok，exit 0）与 `gofmt -l`（无输出）。
- 如实声明：这一行改动**发生在 T6″ 复核快照之后**，未被该次独立复核覆盖（行为等价，风险可忽略，但记录在案以备下一次复审核账）。
