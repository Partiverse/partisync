# T6′ 独立复审报告 — partisync MCD（T6-REPAIR P0+P1 修复后）

- **日期**: 2026-09-12
- **任务**: T6′（P0+P1 修复后的独立复审，DAG：T6-REPAIR-P0/P1 → **T6′** → T7）
- **审计对象**: T6 审计（`docs/evidence/audit-t6.md`）中 **T6-01 / T6-02 / T6-03 / T6-04 / T6-05 / T6-06** 的修复结果，以及修复产出的证据文档
- **输入证据**: `docs/evidence/l2-integration-t6-repair.md`（P0）、`docs/evidence/l2-integration-p1.md`（P1）、`docs/evidence/benchmark-1m.md`、任务卡 `docs/T6-REPAIR-TASK-CARD.md` / `docs/T6-REPAIR-P1-TASK-CARD.md`
- **模型档位**: 主 Agent + 审计员均为 `vectide/glm-5.3`（strong 档，运行时会话自证；无降档）
- **结论**: **verdict = fail**（2 blocker + 4 high，门禁不通过；**门禁主体是前端与检索契约面**，T6-01/02/03/04/06 修复本身成立）

---

## 1. 审计方法与独立性声明

| 审计面 | 执行者 | 独立性 | 裁决 |
|---|---|---|---|
| 安全面（T6-01 摄入路径 + T6-03/06/08 基建） | 审计员 A2（fresh context） | **独立** | `pass`（0 blocker/high；2 medium + 4 low + 3 info） |
| 性能与证据真实性面（T6-02 / T6-04） | 审计员 B（fresh context） | **独立** | `pass`（T6-02/T6-04 关闭；2 medium + 4 low） |
| 前端与检索契约面（T6-05 / T6-01 前端 / 响应契约） | 审计员 C2（fresh context） | **独立** | **`fail`（2 blocker + 4 high + 3 medium/low）** |
| 侦察与事实基线（环境、交付物存在性、L1/L2 复跑、全端点契约扫描） | 主 Agent | 非独立（实施者同一模型） | 与 C2 结论一致，无冲突 |

**独立性说明**: 三路审计员均在 fresh context 中独立执行、未看到主 Agent 的侦察结论，且**相互不可见**；A2 与 C2 各自独立复现了主 Agent 的关键事实（A2 复现路径穿越/去重/上限；C2 独立复现 `results` 缺字段与 `sort` 502）。C2 的裁决与主 Agent 侦察结论**互相印证**，不存在"主 Agent 结论倒灌审计员"的情形。
**审计员 C 首派失败**（无产出，见 §6 恢复事件），按 L2 策略重派为 C2；本报告前端面的结论**来自 C2 的独立裁决**，非主 Agent 自查。

**环境冻结声明**: 审计期间主 Agent 未写入仓库任何文件；审计员被明确告知 `docs/` 非审计对象且禁止写入。审计用压测报告输出被重定向到 `/tmp`，未覆写 `docs/evidence/benchmark-1m.md`。

---

## 2. 一手环境事实（可复现）

| # | 命令 | 真实输出（节选） | 含义 |
|---|---|---|---|
| E1 | `docker compose config --services` | `meilisearch` `postgres` `server` | T6-03 已修：编排自包含 |
| E2 | `docker ps --format '{{.Names}}\t{{.Status}}'` | 三容器均 `Up … (healthy)` | T6-06 已修（对照 T6 审计 E3 的 `unhealthy` / `FailingStreak=3316`） |
| E3 | `curl -s -H 'Authorization: Bearer …' 127.0.0.1:7700/stats` | `"numberOfDocuments":1000000`，`databaseSize:1840902144` | T6-04 的前提成立：索引**真实 100 万**（非 50 万） |
| E4 | `REPORT_FILE=/tmp/… ./scripts/benchmark-1m.sh` | 5000 ok / 0 errors，`p95 44.97 ms`，RPS 1448.94 | T6-02/T6-04 **复现成功**（文档值 44.79ms，噪声内一致） |
| E5 | `curl 127.0.0.1:7700/indexes/assets/settings` | `filterableAttributes=["mime_type","resource_type"]`，**`sortableAttributes=[]`** | **C2-2 根因**：从未配置可排序属性 |
| E6 | `curl '…/api/v1/assets?resource_type=image&sort=size_bytes:desc'` | **502** `{"error":"search backend unavailable"}`；服务端日志 `Attribute size_bytes is not sortable` | 排序一用即毁检索 |
| E7 | `curl '…/api/v1/assets?sort=size_bytes:desc&limit=2'` | 200，顺序与不带 `sort` 完全一致 | C2-3：仅 `sort` 时静默走 PG 路径、参数被丢弃 |
| E8 | `curl '…/api/v1/assets?limit=24'` | 顶层键 `['limit','offset','results']` | C2-1/C2-4：无 `assets`、无 `total` |
| E9 | `curl '…/api/v1/assets?resource_type=image&limit=1'` → `total=1000`；`?offset=1000` → `hits=0` | — | C2-5：`maxTotalHits=1000` 硬墙，百万资产翻不过第 1000 条 |
| E10 | `GOCACHE=/tmp/go-cache-audit go build ./... && go vet ./... && go test -race -count=1 ./...` | 三项 exit 0；`api`/`search`/`storage` 三包 `ok` | L1 真绿（**注意**：不带 `GOCACHE` 覆盖时会因 `~/.cache/go-build` 被沙箱拒绝而产出**假绿**，见 §7） |
| E11 | `cd playground && npx tsc --noEmit` | exit 0 | 类型检查通过（但**拦不住** C2-1：`as Promise<…>` 断言掩盖了字段错配） |
| E12 | `psql … SELECT count(*)` | `assets=8`、**`tags=0`**、**`asset_tags=0`**、`jobs=6`（全 completed） | 见 §5：标签/确认链路从未被执行过 |
| E13 | 全端点契约扫描（后端 `writeJSON` 键 vs 前端取数） | 仅 `GET /api/v1/assets` 错配；`asset`/`job`/`jobs`/`existing`/`file_deduped` 全部一致 | C2-1 是**孤立单点**，非系统性 |

---

## 3. Findings 汇总

| ID | 严重度 | 面 | 标题 | 证据 | 影响 |
|---|---|---|---|---|---|
| **C2-1** | **blocker** | 契约 | 响应契约失配：后端返 `results`，前端读 `assets` | `handlers.go:232,254` vs `api.ts:37`、`assets.tsx:67`（E8、E13） | 资产页恒渲染空态（"暂无符合条件的资产"、`共 0 个资产`、`1–0 / 0`、翻页永久禁用）→ **T6-05 全部前端成果运行时不可见**，T6-01 的浏览器侧摄入闭环用户看不到结果 |
| **C2-2** | **blocker** | 检索 | `sort` + `q`/filter 组合即 502 | E5+E6：`sortableAttributes=[]`；`meili.go:106-108` 无条件发 `sort` | 任务卡 §2 项二要求的排序参数**一用就毁掉整个检索请求**（502） |
| C2-3 | high | 检索 | 仅传 `sort` 时静默无效 | E7；`handlers.go:215` 分支条件不含 `sortParam` | 同一参数在两条路径下语义不一致：一条 502、一条被无声丢弃；误配无任何告警 |
| C2-4 | high | 契约 | PG 路径缺 `total` | E8；`handlers.go:253-257` | 无搜索/过滤时 `total=0` → 分页控件失效 |
| C2-5 | high | 检索 | 深分页硬墙 `maxTotalHits=1000` | E9 | 百万资产库只能翻到第 1000 条，`共 N 个资产` 被截断到 1000 → **与"百万资产可检索"的产品口径直接冲突** |
| C2-6 | medium | 契约 | Meili 命中缺 `path`/`metadata`/`created_at` | `?q=hero` 首命中字段缺失；`meili.go:58-77` 只索引少量字段；`assets.tsx:276` | 卡片/详情渲染 `Invalid Date`、路径与尺寸信息为空 |
| C2-7 | medium | 交付物 | MIME 过滤维度未实现 | `assets.tsx:206-221` 仅 4 个 `resource_type` 按钮，`mimeType` 从不传 | 任务卡 §2 项二「按 MIME 过滤」未达成 |
| C2-8 | medium | 测试 | 新增单测无断言力 | `meili_test.go:11-18` httptest 恒返 200 且不校验请求体；`handlers_test.go:137-149` 只测 `meili=nil` | 任务卡「补齐对应单元测试」**名义完成、实质零覆盖**——这正是 C2-1/C2-2 能漏过 L1 的原因 |
| C2-9 | low | 交付物 | 排序 UI 缺失 | grep 仅 `api.ts` 出现 `sort` | 排序能力无前端入口 |
| A2-01 | medium | 安全 | multipart 大文件先落容器 `/tmp`，绕开资产卷 | `handlers.go:463 ParseMultipartForm(8MiB)`；compose 仅挂 `asset_data:/data` | 单请求最多 ~101MiB 双写；无认证并发上传可写满宿主 docker 层，卷配额覆盖不到 |
| A2-02 | medium | 安全 | `/healthz` 是纯 liveness | `handlers.go:85-87` 静态 `ok`；Meili 400 期间容器仍 healthy | T6-06 假阴性已修，**假阳性面仍在**：依赖故障不触发重启/告警 |
| A2-03 | low | 安全 | 请求体上限硬编码，`MAX_UPLOAD_BYTES` 调小不生效 | `handlers.go:448` vs `main.go:71` | 配置与实现不一致 |
| A2-04 | low | 安全 | 白名单只约束后缀、不校验内容；嗅探失败回退扩展名 | 实测 `..png` 装二进制 → `mime_type=image/png`、`content_sniffed=false` | 类型混淆；**未来加下载端点时若按 DB mime 内联返回即成存储型 XSS** |
| A2-05 | low | 安全 | 提交后入库/回读失败留孤儿文件，无 GC | `handlers.go:520-541` | 磁盘缓慢泄漏 |
| A2-06 | low | 安全 | 慢速上传受 `ReadTimeout=15s` 限制，报 400 误导 | `main.go:30`；20MiB@1MB/s 实测 400 | 大文件低速上传必失败且状态码误导 |
| A2-07…09 | info | 安全 | `Open()` 先 open 后 `EvalSymlinks`（当前无调用方）；去重回传完整元数据（无认证属边界外）；镜像 tag 未按 digest 固定 | — | 后续加固项 |
| B-01 | medium | 证据 | 报告 p95 被 ~40ms 客户端传输伪影主导 | httptrace：42/43ms 全在 body-read；同查询 curl/python 端到端 2–4ms；Meili `processingTimeMs` 0–3ms | 44.79ms **不是检索延迟**；伪影只抬高不压低，故不推翻 PASS，但口径必须修正 |
| B-02 | medium | 证据 | 证据自称"端到端"，实为直连 `7700` | `scripts/benchmark-1m.sh:8`、`l2-integration-p1.md:36-48` | **产品链路 `8080 /api/v1/assets` 的延迟无任何证据覆盖** |
| B-03 | low | 证据 | 脚本静默忽略 CLI 参数 | 脚本全文无 `$@`；`--count=999999 --only-search=false` 实测被忽略 | 任务卡 §2 五个参数仅 3 个有 env 等价物，误配无告警 |
| B-04 | low | 证据 | 查询集含 2 个 0 命中词（占 7.1% 请求） | `receipt`/`license` `estimatedTotalHits=0` | 经检验未拉低 p95（fast 子集 19.6% > 5%） |
| B-05 | low | 证据 | 无 warmup；p95 天然丢弃最慢 5% | `cmd/bench/main.go:122-148` | 冷启动影响被隐藏（本次 max 69ms，不改变结论） |
| B-06 | low | 证据 | 证据文档数字与措辞过期/夸大 | 报告 LMDB `1408.01MB` vs 实测 `1755.62MB`；`l2-integration-p1.md` §2.2 标题含"与排序"而排序实测是坏的；§3 称"已实测索引到 1,000,000 条"却附 `Only Search=true` 日志 | 状态类数字不可再引用；标题/断言超出证据范围 |

---

## 4. 逐项裁决（T6 原 findings）

| 原 finding | 修复声称 | T6′ 裁决 | 依据 |
|---|---|---|---|
| **T6-01** blocker（无真实文件摄入） | `internal/storage` + `POST /assets/upload` + 前端文件选择 | ✅ **关闭**（后端/安全面）<br>⚠️ **前端不可见** | A2 实证：路径穿越（`../../../../etc/passwd.png` → 纯哈希路径）、`%2f`/`..\..\`/4000 字符名、`.png.exe` 415、`.PNG` 归一化、100MiB+2KiB 413、chunked 103MiB 413、失败零残留、去重不重复落盘、落盘 `0600`、SQL 全参数化。**但** C2-1 使浏览器侧摄入结果不可见 |
| **T6-02** high（脚本未交付） | `scripts/benchmark-1m.sh` | ✅ **关闭** | 脚本存在且可用（健康检查、断点续跑正确——ID 空洞 `bench-000500000..2` 与 `1,000,003 ≥ 1,000,000` 自动跳过双向佐证）；余 B-03 形式缺陷 |
| **T6-03** high（compose 缺 server） | 新增 `server` 服务 + Dockerfile + vendor | ✅ **关闭** | E1+E2；三容器由 compose 管理、`depends_on: service_healthy`、`127.0.0.1` 绑定 |
| **T6-04** high（100 万指标未证实） | 100 万实测 p95=44.79ms | ✅ **关闭** | 两次独立复现（脚本 44.97ms / 45.02ms；python 50 线程独立口径 42.42ms），余量 ≥2×；E3 索引真实 100 万。**但口径须按 B-01 修正为"50 并发饱和下客户端观测 p95≈42–45ms，服务端处理 0–3ms"** |
| **T6-05** high（前端交付物缺项） | 后端 filter/sort + 前端双视图/过滤/防抖 | ❌ **未关闭** | 见下 |
| **T6-06** medium（healthcheck 假阴性） | 改 `127.0.0.1` | ✅ **关闭** | 容器内跑原命令 = OK；`localhost:7700` 实测 RC=1（证明根因真实）。余 A2-02 假阳性面 |
| T6-08 medium（端口暴露） | 端口收敛 `127.0.0.1` | ✅ **关闭** | A2：8080/5432/7700 发布 `HostIp` 全为 `127.0.0.1` |

### T6-05 逐项达成

| 任务卡 §2 项二要求 | 裁决 | 证据 |
|---|---|---|
| 多维属性过滤栏（resource_type / MIME / 防抖） | **部分** | `resource_type` 4 键 + 150ms 防抖达成（`assets.tsx:206-221`、`:75-82`）；**MIME 过滤未实现**（C2-7）；且 C2-1 使结果不可见 |
| 网格 / 表格视图切换 | **代码达成、运行时不可验收** | 视图齐备（`assets.tsx:226-317`），但列表恒空只渲空态 |
| `tsc --noEmit` 0 错误 | **达成** | E11 |
| 视口分页与**虚拟化/懒加载流式网格** | **未达成** | 仅服务端分页（`limit=24` + 上/下页 `:320-340`）；`virtual`/`masonry`/`IntersectionObserver` 在 `assets.tsx` 零命中（唯一命中在无关的 `showcase/shared.tsx:344`）；且因 C2-4 分页实际不可用 |
| （后端）`resource_type`/`mime_type`/`sort` 过滤与排序 | **部分** | 前两者实测正确（image/document/video/pdf 命中类别正确）；**`sort` 完全不可用**（C2-2/C2-3） |
| （测试）补齐对应单元测试 | **未达成** | C2-8：新增测试零断言力 |

---

## 5. 对 T7 的影响（闭环前提缺口）

SESSION.md §5 的 MCD 闭环为「自托管启动 → 上传/导入 → 元数据+索引 → 搜索/筛选/**预览** → **人工分类/文本标签** → 提交慢标注 → 排队/执行/重试/通知 → **人工确认 AI 建议并写入标注**」。T6′ 实测（E12）：

1. **「人工分类/文本标签」不存在**：`Store` 无任何标签写入方法，全仓无 `INSERT INTO tags/asset_tags` 写入路径；**`tags=0`、`asset_tags=0`**（8 资产 / 6 个已完成任务下仍为 0）。
2. **「人工确认 AI 建议并写入标注」不存在**：无该端点（路由表仅 8 条），前端只**只读展示**建议徽章（`assets.tsx:415-441`），无确认按钮。
3. **「预览」不存在**：无文件读取/缩略图端点（`storage.Open` 已实现但未挂路由，P0 证据文档 §5.2 已自述），前端无 `<img>`。

**结论**：T7 的 L3 全闭环验收**不具备前提**（历史缺口，非本次修复引入）。另 C2-1/C2-2/C2-5/C2-6 亦会直接影响 L3 表现（列表空态、排序 502、深分页、`Invalid Date`）。

---

## 6. 恢复事件记录

| 事件 | 级别 | 情况 | 处置 |
|---|---|---|---|
| T6′-L1-1 | L1 | 审计员 C（前端与契约面）首次派发**无产出失败**，closing message 仅剩注入的技能文本 | 收紧提示词（去掉 Vite/Playwright 重型任务，限定 curl+静态核对）后重派为 C2，成功并给出 fail 裁决 |
| T6′-L1-2 | L1 | 审计员 A（安全面）首次派发**无产出失败** | 重派为 A2，成功并给出 pass 裁决 |
| T6′-L1-3 | L1 | 初次 `go build/vet/test` 使用默认 `GOCACHE=~/.cache/go-build`，被文件沙箱拒绝（`permission denied`），而管道后的 `$?` 取到 `tail` 的退出码 **0** → 曾出现**假绿** | 改 `GOCACHE=/tmp/…` 并以文件重定向捕获真实退出码，重跑得真绿（E10）；教训：管道会吃掉真实退出码 |

> 经验（候选人记忆）：本环境 subagent 审计员**首派无产出失败率仍高**（本次 2/4），但只要把提示词收敛到"curl + 只读代码"这类轻任务即可显著提高成功率；重型（起前端 dev server / 浏览器自动化）派发更容易整轮无产出。

---

## 7. 未验证项与盲区

1. **浏览器真实渲染未经机器验证**：本会话模型无图像输入（`read_image` 实测 `model "glm-5.3" does not declare image input`），C2-1 的浏览器现象为「响应确无 `assets` 键 + `?? []` 兜底」的强推断；**真正的 L3 浏览器闭环属 T7**。
2. **磁盘写满 / 存储只读 / PG 宕机时 `/healthz` 表现**：需占满磁盘或停容器，均被审计禁令排除，仅代码推断。
3. **并发同内容上传竞态、100MiB 上传内存峰值**未压测。
4. **`8080` 产品链路延迟**未被基准覆盖（B-02）；`curl` 曾观察到 3–45ms 间歇抖动，根因未定（疑 delayed-ACK/Nagle，容器外未复现）。
5. **规模敏感性未证明**：无 50 万 vs 100 万对照队列，且 `maxTotalHits=1000` 使各查询词工作量趋同，故"1M 下 p95<100ms"成立但**不构成规模扩展性证明**。
6. 审计期间索引被并发写入（1,000,000 → 1,000,006），任何"文档总数"快照非定值。

---

## 8. 门禁判定与处置建议

- 任务卡门禁：`verdict = pass` 且无 open blocker/high 方可交付 → **fail**（2 blocker + 4 high，全部集中在**前端与检索契约面**；T6-01 后端/安全面、T6-02/03/04/06 修复本身成立）。
- 阻断性质：**不是安全漏洞**（安全面独立 pass），而是**交付物在运行时不可用** + **证据覆盖不足**（编译绿、测试零断言、基准口径失真）。

**处置顺序建议**

| 优先 | 项 | 处置 |
|---|---|---|
| P0 | C2-1 / C2-4 | 前端改读 `results`（或后端统一改 `assets`）+ PG 路径补 `total`；补一条**断言响应契约**的单测 |
| P0 | C2-2 / C2-3 | `EnsureIndex` 补 `sortableAttributes`（并对 1M 索引触发重索引）；`sort` 纳入 Meili 分支条件；`sort` 参数白名单校验（非法 → 400） |
| P1 | C2-5 | 配置 `pagination.maxTotalHits`（或改前端分页口径），使百万资产可翻页 |
| P1 | C2-6 / C2-9 / C2-7 | 详情走 `GET /assets/{id}` 取全字段；补排序 UI；补 MIME 过滤 UI（或按 §3 修任务卡口径） |
| P1 | C2-8 | 把 httptest 换成**校验请求体**的假 Meili（断言 filter/sort 语法、设置项），并断言 GET 列表响应键 |
| P2 | A2-01…06、B-01…06 | 见 §3；其中 B-01/B-02/B-06 要求**修正证据文档口径**（应作为独立小任务，不得静默改判） |
| 阻断 T7 | §5 三项闭环缺口 | 需用户决策：补齐最小确认/标签/预览能力，或按现状降级 T7 口径 |

---

## 9. 结论

**T6′ verdict = fail。** T6-01（后端与安全面）、T6-02、T6-03、T6-04、T6-06、T6-08 的修复经独立复核**成立并关闭**；**T6-05 未关闭**，且 P1 修复在**前后端响应契约**与**排序链路**上引入了两处 blocker 级运行时缺陷（C2-1/C2-2），叠加 C2-3/C2-4/C2-5 三条 high，使"前端可用 + 百万资产可检索"这一 T6-05/T6-04 的交付意图在运行时无法成立。

在 C2-1/C2-2 修复并回归验证前，**T7 不具备验收前提**；即便修复，§5 的三项 MCD 闭环缺口仍需先行决策。

---

## 10. 附注：复审后的处置状态（2026-09-12 追加）

> 本节为**追加状态**，不修改第 3/4/8 节中审计当时的原始裁决与证据。

| finding | 状态 | 证据 |
|---|---|---|
| C2-1 blocker（响应契约失配） | **已修** | 前端改读 `results`；`TestListAssetsMeiliContract` 断言响应键；浏览器回归 `列表渲染非空 cards=15` |
| C2-2 blocker（`sort` 必然 502） | **已修** | `EnsureIndex` 补 `sortableAttributes`（Meili task 227 `succeeded`）；`TestEnsureIndexConfiguresSortableAttributes`；curl 实测三类排序均 200 且顺序正确 |
| C2-3 high（仅 `sort` 静默丢弃） | **已修** | `sort` 纳入 Meili 分支 + `ValidateSort` 白名单（非法 400） |
| C2-4 high（PG 路径缺 `total`） | **已修** | 新增 `Store.CountAssets`；curl 实测 PG 路径 `total=15` |
| C2-6 medium（命中缺字段 → `Invalid Date`） | **已修** | 索引补 `path`/`created_at`；前端日期防御渲染 + 详情回源；浏览器实测无 `Invalid Date` |
| C2-8 medium（单测零断言力） | **已修** | 新增 8 个有断言力的用例（断言真实请求体/响应键） |
| C2-5 high（`maxTotalHits=1000` 深分页硬墙） | **打开** | 需权衡内存后决策，未动 |
| C2-7 / C2-9 | **打开** | 属新增 UI 面，未获批不动 |
| A2-01…A2-06、B-01…B-06 | **A2 已修（见 §12），B 打开** | 其中 B-01/B-02/B-06 要求修正证据文档口径 |

**C2-6 的精确化（补充审计当时未能区分的细节）**：Meili 命中缺字段**只发生在 App 写入的文档**（`UpsertAssetDocument` 仅索引 6 个字段）；压测语料 `bench-*` 文档由生成器写入了 `path/created_at/tags/owner/description`，因此**同一缺陷在两类文档来源上表现不一致**——审计时对 `bench-*` 文档取样会看不到 `Invalid Date`。修复后用 App 上传资产专门回归（卡片曾渲染 `Invalid Date`、详情路径为空，修复后为 `—` 与真实内容寻址路径）。

**修复本身的一处自证缺口（自我复查）**：第一版 C2-6 修复引入新缺陷——对非 UUID 的 `bench-*` id 无条件回源 `GET /assets/{id}` 触发 400，且把 `selectedAsset` 置空导致**弹窗被关闭**（比修复前更糟）。该缺陷由浏览器回归脚本的 `badResponses` 捕获，已修（仅 UUID 回源 + 条件覆盖）。

**修复的实施者自证声明**：以上修复与验证均由**主 Agent（实施者，strong 档）**完成，**不构成独立复审**；门禁要求下一轮执行 **T6″** 独立复核 C2-1…C2-4/C2-6。

---

## 11. T6″ 独立复核（修复后的裁决）

- **执行者**: 独立复核员（fresh context，`vectide/glm-5.3` strong 档），**只读**：未修改仓库任何文件、未重启容器
- **输入**: `docs/evidence/l2-integration-t6-prime-fix.md`（实施者自证，即复核对象）
- **结论**: **verdict = pass（6/6 项成立）**

| # | 复核项 | 裁决 | 独立证据（复核员自行执行） |
|---|---|---|---|
| 1 | 响应契约（C2-1） | **成立** | 两条路径顶层键均恰为 `limit/offset/results/total`；`grep -rn "\.assets\b\|assets:"` 全 `playground/src` 无字段读取（唯一命中是注释） |
| 2 | 排序真序（C2-2） | **成立** | desc → `[100000,13058,13057,9899,4104,2047×5]` 严格非增；asc → `[9,22,31,45,1024…]` 非减；`limit=100` 双向序列 `monotonic: True` |
| 3 | 非法排序 400（C2-3） | **成立** | `bogus:zzz` / `size_bytes` / `size_bytes:up` / `created_at:desc` 全部 400，且**未到达检索后端** |
| 4 | 前端字段（C2-1） | **成立** | `api.ts:36` 为 `results`；`assets.tsx:83 setAssets(r.results ?? [])` |
| 5 | PG 路径 `total`（C2-4） | **成立** | `handlers.go:216` 先校验、`:222` sort-only 也进 Meili 分支、`:254-271` PG 分支调 `CountAssets`（`store.go:125`） |
| 6 | 测试断言力（C2-8） | **成立** | `go test -race` 19 用例全 PASS；测试断言真实请求体（path/filter/sort/q/sortableAttributes）与响应键（含"无 `assets` 键"） |

**证伪尝试（均未成功）**: 全仓无 stale `assets` 读取；跨页一致性（desc `offset=4` 首元素 = `offset=0` 第 5 项）；`offset=1500` → 200 + 空 results（未 502）；并在**实况索引**上验证排序确实生效（不只是 stub 断言）。

**T6″ 新发现（处置状态）**

| ID | 严重度 | 内容 | 状态 |
|---|---|---|---|
| T6″-1 | 中低 | Meili 路径 `total` 被截到 1000（`estimatedTotalHits` 受 `maxTotalHits=1000` 限制）；`offset>1000` **静默返回空列表 + HTTP 200**，用户翻到尾部只见空白 | 即 C2-5 的另一表现，**待用户决策** |
| T6″-2 | 低 | `meili.go` 中 `SortableFields` 注释称"只收录 `UpsertAssetDocument` 真实写入的字段"，而该函数实际已写入 `created_at`/`path` —— **注释与实现不符** | **已修**：注释改为说明"`created_at` 已写入但排序语义不稳定（语料为 `+08:00` 偏移串、App 为 RFC3339 UTC），故不开放排序，由 `ValidateSort` 明确 400" |
| T6″-3 | 低（待定） | PG 中存在资产名为 `..%2f..%2f..%2fetc%2fshadow.png`（`name:asc` 排首位），疑为审计员 A2 的穿越测试残留 | **本轮已核实并关闭**：该值仅是 PG 里的**展示名字符串**；`/data` 下只有 `assets`、无越界物、`.tmp` 残留 0、无符号链接，9 个存储文件全部符合 `sha[0:2]/sha[2:]` 内容寻址格式 → 穿越在存储层确已中和。**但测试残留数据仍留在库中**（A2 的 `evil.png`/`passwd.png`/`shadow.png` 与本轮 `pw-smoke.png`），建议在 T7 的空库环境中自然隔离，或另行批准后清理 |

**门禁更新**: T6″ 对 C2-1/C2-2/C2-3/C2-4/C2-6 的修复给出 **pass**；**C2-5 仍打开**，故 T6′ 的门禁依旧不放行（`verdict=fail` 的原始裁决维持，第 8 节不变）。

---

## 12. P2 加固批（A2-01…A2-06）处置状态（2026-09-13 追加，实施者自证）

> 本节为**追加状态**，不修改第 3/4/8/10/11 节中审计与复核当时的原始裁决与证据。

| finding | 状态 | 机制与证据 |
|---|---|---|
| A2-01（multipart 大文件先落容器 `/tmp`） | **已修** | `ParseMultipartForm` → `MultipartReader` 流式解析，文件部分直接进 `storage.Save`（写进资产卷内）。L2 实测 12MiB 上传后容器可写层 `delta=0 bytes`、`/tmp` 条目 0；反向实验证明旧实现必然落 `multipart-*` 12,582,912 bytes |
| A2-02（`/healthz` 纯 liveness） | **已修** | 新增 `GET /readyz`：PG/Meili/存储逐项 + 任一不可达 503；`/healthz` 保持纯存活（依赖故障不触发重启）。L2 实测 PG、Meili 双向停机-恢复全部断言通过 |
| A2-03（请求体上限硬编码） | **已修** | 上限改为 `store.MaxBytes() + 1MiB`。L2 一次性容器 `MAX_UPLOAD_BYTES=1MiB`：2MiB → 413、512KiB → 201 |
| A2-04（只约束后缀、按声明 MIME 内联返回） | **已修** | 预览前 `MatchedContentType` 核对内容与声明（不一致 → 415）+ `Content-Security-Policy: … sandbox`。L2 以元数据端点构造「HTML 字节声明为 `image/png`」→ 415，真实 `text/html` 资产 → 200。**并修掉初版逐字比较导致的 md/csv/json 误拒**（改为文本家族收敛判定） |
| A2-05（失败留孤儿文件） | **已修** | 入库/回读失败路径调用 `storage.Discard`（幂等、经越界校验；去重命中对象不删）。L2 停机 PG 上传 → 500 且资产卷内无该 sha 对象 |
| A2-06（`ReadTimeout=15s` 截断慢上传） | **已修** | 服务端级超时保持严格（读 15s / 头 15s / 写 30s），上传端点用 `http.ResponseController` 放宽本次请求读 5min、写 5min+30s——**同时解除 `WriteTimeout=30s` 这道第二墙**。L2 实测 4MiB @100KB/s = 40.97s → 201 |
| A2-07…A2-09（info） | **打开** | 上传端点无认证；镜像 tag 未按 digest 固定；不在本批范围 |

**证据**：`docs/evidence/l2-integration-p2-hardening.md`（L1 全绿 + L2 53/53 PASS + L3 回归 8/8 + 反向验证）；任务卡 `docs/P2-HARDENING-TASK-CARD.md`；L2 脚本 `scripts/l2-p2-hardening.sh`。

**独立性声明**：本批修复与验证均由**主 Agent（实施者，strong 档）**完成，**不构成独立复审**（任务卡 AC-11 未执行）。第 8 节的门禁判定与第 11 节 T6″ 结论不因本节改变；C2-5 虽已修（`docs/evidence/l3-mcd-acceptance.md`），其独立复审同样未执行。
