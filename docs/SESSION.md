# SESSION.md — partisync 路线基线 v2

> **用途**：抗上下文压缩的项目事实源。进入新阶段或继续会话前必须重读；若路线变化，先更新本文件再行动。
> **当前阶段**：MCD 闭环已通过 L3 浏览器验收（T7，8/8 断言）；C2-5 与 T7 三项前提缺口（标签/确认/预览）已补齐；**P2 安全加固批 A2-01…A2-06 已实现、复审并修掉 1 个 medium 缺陷，L1/L2/L3 全绿**；**AC-11 独立复审已满足（2026-09-13）**——换路由后三面 fresh-context 审计员（vectide/glm-5.3、minimax-cn/MiniMax-M3、tencent/hy3、zai/glm-5.3-flash）全部产出有效报告，**verdict 均 pass、无 blocker/high**，新发现 4 medium + 9 low + 3 info 残留（未修，见 §14）。详见 §12/§13/§14。

## 1. 真实目标

把 `partisync` 从泛化的多源文件聚合/同步工具收敛为一款**面向 AI/ML 团队的自托管智能数据资产管理（DAM）基础软件**：

- 统一摄入、组织和检索 AI 数据资产；
- 以高文件吞吐和大规模低延迟检索作为底座能力；
- 通过后台异步慢标注降低 AI 调用成本；
- 将资产、人工标注、AI 建议和数据集组织在同一工作流中；
- 首版以 Web + 自托管 Docker 为主，后续再扩展桌面、本地优先和托管服务。

### 关键澄清

- “聚合多端”指数据分布在多个数据源，用户在一个客户端统一处理，不是同一账号在多个设备间同步同一份数据。
- “极致性能”指文件摄入/处理吞吐和索引检索能力，不指 AI 推理速度。
- 检索目标是**跨百万资产的元数据/全文搜索 p95 < 100ms**；必须用可复现 benchmark 证明，不能只作宣传语。
- AI 标注采用后台异步、可排队、可批处理的“慢标注”模式，优先降低调用成本，不追求实时推理。

## 2. 产品定位与竞争边界

### 定位

> **为 AI/ML 团队提供可自托管、高性能、资产管理与标注衔接一体化的数据资产底座。**

### 不正面竞争的对象

- 不与 Label Studio/CVAT 拼完整的专业框选、多边形和视频标注套件；
- 不与 Bynder/AEM 拼企业品牌资产、营销审批和大型 DAM 销售体系；
- 不与 Immich 拼照片管理；
- 不与 rclone 拼全协议同步 CLI；rclone 只作为连接器/传输能力的参考；
- 不把 partisync 做成 n8n/Pipedream 式通用工作流平台。

### 预期差异化

1. AI 数据资产的统一管理、检索与标注衔接，而不是孤立的标注工具；
2. 可量化的大规模索引性能：百万资产、p95 < 100ms；
3. 后台慢标注、批处理、成本预算与人工确认，而不是默认实时调用昂贵模型；
4. 自托管、数据可控、适合内部数据集与合规场景；
5. `Resource.type` 保持扩展性：首版以图像和文档为主，未来可扩展视频、音频、SaaS 资源等。

## 3. 当前已定决策

| 编号 | 决策 | 取舍与理由 |
|---|---|---|
| D1 | 产品方向为 AI 数据资产 DAM | 比泛文件聚合更有明确用户问题和竞争缝隙 |
| D2 | 首版目标用户为 AI/ML 团队 | 与 Web + 服务端架构匹配，资产规模和检索性能有真实价值；个人创作者付费弱，通用 DAM 竞争强 |
| D3 | **首版客户端采用 Wails (Go 桌面端 + Web 同构)** | 原生窗口直接享受本地磁盘与文件系统高吞吐，同时保留 Vite Web HMR 极速体验；支持跨平台打包 |
| D4 | 首版部署为单机本地优先运行（支持 Docker/本地嵌入） | 数据完全不出用户本地磁盘；SaaS 托管版后置 |
| D5 | 服务端/本地核心持有资产与索引的权威副本 | Wails 本地 Go 进程直读本地磁盘，完全突破浏览器沙箱限制，彻底兑现高吞吐与索引指标 |
| D6 | 性能目标为 p95 < 100ms @ 100 万资产 | 以元数据/全文检索为首要可验证指标，另行测量导入、索引和导出吞吐 |
| D7 | 首版标注为分类 + 文本标注 | 保留人工确认和 AI 建议；暂不实现视觉框选/多边形编辑器 |
| D8 | AI 采用后台异步慢标注 | 任务队列、批处理、成本控制、失败重试和人工确认优先于实时体验 |
| D9 | 首版资产类型以图像 + 文档为主 | 先验证通用资产索引和标注闭环；视频/音频后置 |
| D10 | 首版数据源以文件型源/上传为主 | 本地文件夹、对象存储、WebDAV 等可作为摄入源；SaaS 资源暂不进入 MCD |
| D11 | 模型保留 `Resource.type` 抽象 | 后续可增加 `notion_page`、`github_issue`、`vault` 等类型，避免首版锁死为单一文件模型 |
| D12 | 跨源能力先做轻量标签/集合/数据集组织 | 不做强工作流编排；跨源动作链另列产品方向 |
| D13 | **服务端核心语言选用 Go (Golang)** | 高并发 I/O、内存受控、部署轻量（单二进制静态链接），Immich 与 rclone 的同款工业级验证选型 |
| D14 | **搜索底座选用 Meilisearch** | 独立轻量容器，开箱即用高吞吐与容错，工业界已验证百万资产亚秒级/毫秒级稳定表现，运维负担小 |
| D15 | **慢标注任务系统选用 DB-based 队列** | 依托关系库（Postgres `FOR UPDATE SKIP LOCKED` 或 SQLite WAL）承载任务状态，单机自托管无需外挂 Redis |

## 4. 功能借鉴边界

### rclone

借鉴 connector/backend 抽象、能力自描述、过滤器、限流、断点续传和配置管理思想。必须在实施前决定“直接调用 rclone”还是“自写适配层”，不得无边界复制其复杂度。

### Cryptomator

作为后续隐私能力参考：Vault 抽象、客户端密钥、零知识存储和文件名/内容加密。不是 MCD 前置依赖；安全设计需单独评审。

### FreeFileSync

仅参考差异对比和同步策略的交互思想。文件夹双向同步不是当前 DAM MCD 的核心，不因此引入冲突副本或 CRDT。

### 其他重点参考

- Immich：百万级媒体库的索引与自托管产品经验；
- Label Studio/CVAT：标注任务与人工审校边界；
- Tantivy/Meilisearch：全文索引与检索性能候选；
- SQLite/Postgres/DuckDB：元数据、事务和分析查询的候选底座；
- DVC/lakeFS：后续数据集版本与可追溯性参考。

## 5. MCD（首个可独立验收阶段）

### 目标闭环

```text
自托管启动
  → 创建/选择资产库
  → 上传或导入图像/文档
  → 提取基础元数据并建立索引
  → Web 搜索、筛选、预览
  → 人工分类/文本标签
  → 提交后台慢标注任务
  → 任务排队、执行、失败重试/完成通知
  → 人工确认 AI 建议并写入标注
```

### MCD 边界

- 单机自托管、先不承诺多租户和多区域；
- Web UI + 服务端 API；
- 资产元数据、标签、集合、标注任务可持久化；
- 至少一个真实文件摄入路径和一个可替换的 AI endpoint；
- 以合成/脱敏的 100 万资产元数据集进行性能基准；
- 首版不包含视觉框选、多人实时协作、跨设备同步、SaaS 连接器生态、端到端加密和托管版。

### MCD 完成定义

必须达到 L1/L2 检查，并通过 L3 真实入口验收：在空库 Docker 环境中启动服务，经浏览器完成摄入→索引→搜索→标注任务→人工确认闭环；同时产出可复现的百万资产 p95 搜索 benchmark。未完成 L3 时只能称为“部分验证”。

## 6. 阶段路线

### 阶段一：MCD 基础闭环

- 服务端资产库、对象存储/本地存储适配；
- 图像/文档摄入和元数据提取；
- 全文/结构化索引与搜索；
- 标签、集合和基础分类/文本标注；
- 后台慢标注队列、批处理、成本记录、人工确认；
- Docker 单机部署与百万资产 benchmark。

### 阶段二：数据集与规模能力

- 数据集快照、版本和可追溯 lineage（参考 DVC/lakeFS）；
- 去重、质量检查、采样和导出；
- 视频/音频资产；
- 更丰富的文件型 connector；
- 多用户协作和权限；
- 性能剖析、增量索引和更大规模部署。

### 阶段三：差异化与生态

- 主动学习循环和标注质量度量；
- 本地模型/Ollama 与可配置推理 endpoint；
- Cryptomator 风格的零知识 Vault；
- 桌面端/本地文件监听与可选同步；
- SaaS 资源类型与跨源 connector；
- 自托管与 SaaS 托管双形态。

## 7. 未决问题（下轮讨论，不在未决前编码）

1. AI/ML 团队的首个细分场景：计算机视觉数据集、文档/NLP 数据集，还是多模态研究资料；
2. 首版导入来源：浏览器上传、服务端挂载目录、对象存储，或其中的最小组合；
3. 服务端技术栈与索引组合：Rust/Tantivy、独立 Meilisearch，或关系库全文检索；
4. “文件处理吞吐”的具体目标：导入、哈希、缩略图、导出分别需要达到什么指标；
5. AI endpoint 的协议、成本预算、隐私边界和失败重试语义；
6. 是否需要认证、团队权限和审计日志进入 MCD。

## 8. 明确排除（防止范围蔓延）

- 首版不做 CRDT/OT、富文本合并、文件冲突副本；
- 首版不做 4MB 分块双向同步；
- 首版不做完整通用工作流自动化；
- 首版不做实时 AI 推理承诺；
- 首版不因“极致性能”引入 GPU 推理系统；
- 首版不同时覆盖所有资产格式和专业标注类型；
- 不把实验性 benchmark 数字当作已实现承诺，必须实测后发布。

## 9. 当前进度与下一步

- [x] 澄清"聚合多端"= 多数据源统一处理；
- [x] 完成市场/竞品方向扫描；
- [x] 收敛为 AI 数据资产 DAM 定位；
- [x] 明确 Web、自托管、百万资产检索和后台慢标注方向；
- [x] 本文件升级为 v2 路线基线；
- [x] 固化 D13-D15 技术栈（Go / Meilisearch / DB 队列）与 ADR-001、MCD-TASK-CARD；
- [x] 桌面端方向收敛：双模同构（Go Headless 服务端 + 预留 Wails 桌面壳入口）；
- [x] **第一梯队 T1 完成**：docker-compose（postgres 16 + meilisearch 1.8，healthcheck 全绿）+ `deploy/postgres/init.sql`（assets/tags/asset_tags/annotation_jobs + 索引）；
- [x] **第一梯队 T2 完成**：Go 服务端骨架（cmd/server + internal/{models,store,search,api}），标准库 net/http，6 个 REST 端点，`go build/vet/test` 三绿（含 -race）；
- [x] **独立审计（strong 档）**：verdict=pass，无 blocker/high；4 项 medium 记录于 `docs/evidence/audit-t1-t2.md`；
- [x] **L2 集成验证**：真实容器+真实服务走通 上传→去重→Meili 同步→检索→标注任务→出队 SQL 全链路，见 `docs/evidence/l2-integration-t1-t2.md`；
- [x] **T3 前**：审计 medium 4 项全部修复（M1 输入校验 / M2 UUIDv7-8 兼容 / M3 processing 租约恢复 / M4 Meili ctx 传播）；main.go 默认 DSN 与 compose 密码对齐（待 L2 验证）；
- [x] **T3 完成**：慢标注异步任务系统交付，含 `internal/worker/worker.go`（Worker + MockAnnotator + Annotator 接口 + 优雅关闭），L1 三绿，`docs/evidence/l2-integration-t3.md` L2 验证通过；
- [x] **T4 完成**：MeiliSearch EnsureIndex 修复（POST /indexes + 405）；GET /api/v1/jobs/{id} 端点；Vite proxy 配置；Typed API client；AssetLibraryPage（搜索/分页/标注模态框）；App.tsx 重写为资产管理 Shell。L2 验证通过，证据：`docs/evidence/l2-integration-t4.md`。前端 TypeScript 零错误；
- [x] **T5 完成**：500k 资产 benchmark，NDJSON 流式写入 MeiliSearch，50 并发 5000 请求，p95=44.54ms < 100ms ✅。证据：`docs/evidence/benchmark-t5.md`（`cmd/bench/main.go`）；
- [x] **T6 完成（审计已出，门禁 fail）**：`docs/evidence/audit-t6.md` —— 1 blocker + 4 high + 7 medium + 4 low + 2 info。blocker=T6-01 无真实文件摄入（MCD 闭环首步缺失）；high=T6-02 `scripts/benchmark-1m.sh` 缺失、T6-03 compose 无 `server` 服务、T6-04 100 万指标未证实（实测 50 万）、T6-05 前端虚拟滚动网格/属性过滤缺失。安全面独立审计员 A `verdict=pass`；并发面与证据面因 subagent 连续失败改由主 Agent 自查（非独立）。**T7 在 P0（T6-06/T6-03/T6-01）修复前不具备验收前提。**
- [x] T6′：修复后独立复审（已执行，verdict=fail；见下与 §12）；
- [x] **T6-REPAIR P0 完成（L2 已验证，L3 未跑）**：healthcheck 修复 + compose `server` 服务（自包含编排）+ 真实文件摄入（`internal/storage` + `POST /api/v1/assets/upload` + 前端上传）。证据：`docs/evidence/l2-integration-t6-repair.md`；任务卡：`docs/T6-REPAIR-TASK-CARD.md`。
- [x] **T6-REPAIR P1 完成（L2 已验证）**：
  - T6-02：交付可复现压测脚本 `scripts/benchmark-1m.sh`；
  - T6-04：真实 100 万资产索引构建完成（LMDB 1.48GB），50 并发 5000 次请求实测 p95 = 44.79ms < 100ms，生成报告 `docs/evidence/benchmark-1m.md`；
  - T6-05：后端 `GET /api/v1/assets` 与 Meili 检索客户端支持 `resource_type`, `mime_type`, `sort` 过滤；前端交付「卡片网格 / 表格」双视图与分类过滤器，`npm run build` 全绿。证据：`docs/evidence/l2-integration-p1.md`；任务卡：`docs/T6-REPAIR-P1-TASK-CARD.md`。
- [x] **T6′ 独立复审已执行（verdict = fail）**：三路 fresh-context 审计员并行裁决——安全面 `pass`、性能与证据真实性面 `pass`、**前端与检索契约面 `fail`（2 blocker + 4 high）**。报告：`docs/evidence/audit-t6-prime.md`。
- [x] **T6′ 阻断项修复完成**（C2-1 响应契约 / C2-2 排序 502 / C2-3 sort 静默丢弃 / C2-4 PG 缺 total / C2-6 命中字段不全 + 补齐有断言力的单测）：证据 `docs/evidence/l2-integration-t6-prime-fix.md`；浏览器回归脚本 `scripts/browser-regression.mjs`（12/12 PASS，真实 Chromium），截图 `docs/verification/t6-prime-fix/`。
- [x] **T6″ 独立复核已完成：verdict = pass（6/6）**，见 `docs/evidence/audit-t6-prime.md` §11；
- [ ] T7：L3 浏览器真实入口验收（Playwright 自动化，截图写入 `docs/verification/`）。**前提缺口未决**：MCD 闭环的「人工分类/文本标签」「人工确认 AI 建议并写入标注」「预览」三项在代码中不存在（`tags`/`asset_tags` 实测 0 行、无文件读取/预览端点）。
- [x] **C2-5 修复 + T7 前提缺口补齐（2026-09-12，用户决策"两问均选A"）**：
  - C2-5：`pagination.maxTotalHits=1,100,000`，`total` 真实、深分页可翻（实测 `offset=1000/100000/999998` 均非空、跨页一致性通过）；单测 `TestEnsureIndexRaisesMaxTotalHits`；
  - 标签系统：`GET/POST /assets/{id}/tags` + `DELETE /assets/{id}/tags/{tagID}` + `GET /tags`（Store 层 6 个方法）；
  - 人工确认：`POST /jobs/{id}/confirm`（事务内 source='ai' 关联；仅 completed 可确认，409/400/404 守卫实测）；
  - 预览：`GET /assets/{id}/preview`（越界防护实测 400）；
  - 前端：详情弹窗内联预览 + 标签增删 UI + AI 确认按钮，`npm run build` 全绿；
  - **T7 L3 通过（8/8 断言，consoleErrors=0，badResponses=0）**：脚本 `scripts/l3-mcd-acceptance.mjs`，证据 `docs/evidence/l3-mcd-acceptance.md`，截图 `docs/verification/t7-l3/`（7 张）；执行后实测 assets=17/tags=4/asset_tags=6；
  - 声明：修复与验证为主 Agent 实施，未做 T7 后独立复审；建议下轮派 fresh-context 审计员。
- [x] **P2 独立复审（AC-11）满足（2026-09-13）**：换路由（deepseek 时代三连败后，本轮 vectide/minimax/tencent/zai 四路）并行派发三面 fresh-context 审计员，全部产出有效报告，**verdict 均 pass、无 blocker/high**：
  - 安全面 `pass（有保留）`：`docs/evidence/audit-p2-independent-security.md`——A2-01/03/04/05+F6 主干 live 实测成立；2 medium（S1 回读失败 Discard 悬空行、S2 部件头无上限 DoS）+ 3 low；
  - 探针/超时面 `pass`：`docs/evidence/audit-p2-independent-probe-timeout.md`——A2-02/03/06 live+单测双成立，F6 在 99MiB+3MiB junk 场景实测清理有效；1 medium（F-I1 `storage.Healthy()` 弱探针）+ 4 low；
  - 证据真实性面 `pass`：`docs/evidence/audit-p2-independent-evidence.md`——L1 三绿、L2 S1–S8 命名卷等价复现、L3 8/8 独立重跑均一致，无伪造/夸大；1 medium（L2 脚本绑宿主 /tmp 不可移植）+ 3 low；
  - 残留 findings 汇总与处置建议见 §14；测试遗留资产行/对象待用户批准后统一清理。
- [x] **SPA 静态托管进 Docker（2026-09-12）**：`cmd/server/main.go` 新增 `spaHandler`（`WEB_DIST=/app/web` 启用，/api 与 /healthz 优先）；Dockerfile 改为 COPY 宿主机 `playground/dist`（容器内无法访问 npm registry，前端由宿主机构建）；`.dockerignore` 放行 dist。L3 全部断言对静态托管服务复测通过（8/8，consoleErrors=0）。体验地址 `http://127.0.0.1:8080/`。
- [x] **P2 安全加固批收尾（2026-09-13）**：A2-01…A2-06 全部关闭——multipart 改流式（不再落容器 `/tmp`）、新增 `GET /readyz` 就绪探针、请求体上限跟随 `MAX_UPLOAD_BYTES`、预览内容-声明一致性校验（415，并修掉初版 md/csv/json 误拒）、失败路径孤儿文件清理、上传端点按请求放宽读写期限（40.97s 慢上传实测 201）。L1 三绿（初版 14 个新用例，含反向验证）；L2 脚本 `scripts/l2-p2-hardening.sh` **53/53 PASS**；L3 回归 **8/8 PASS**（`docs/verification/p2-hardening-run.json`）；证据 `docs/evidence/l2-integration-p2-hardening.md`；任务卡 `docs/P2-HARDENING-TASK-CARD.md`。随后复审（3 次独立派发全败 → 实施者自查，**非独立**）发现并修复 medium 缺陷 **F6**（`a6a6702`），新增单测与 L2 S8 后 **60/60 PASS**、单测 16 个。**独立复审仍未满足（AC-11 pending）**，详见 §13 与 `docs/evidence/audit-p2-hardening.md`。

## 10. 项目事实源

- 工作目录：`/home/acme/Documents/deepseek harness agent/partisync`
- 当前前端实验产物：`playground/`（Vite + React + TypeScript + Tailwind + shadcn）
- 设计参考：`docs/shadcn-design-guidelines.md`、`docs/shadcn-design-spec.html`、`docs/ui-design-spec.html`
- 服务端已实现（Go，`cmd/server` + `internal/{models,store,search,storage,api,worker}`），数据层为 Postgres（`deploy/postgres/init.sql`），检索为 Meilisearch；容器编排含 `postgres`/`meilisearch`/`server` 三个服务。
- 本环境工具集不提供 `agent_teams_*`（AgentTeams）；审计与并行复核统一改用 `subagent` 派发 fresh-context 审计员，独立性在报告中显式声明。

## 11. 当前阶段：T6-REPAIR（P0 门禁解除）

- **触发**: T6 审计 verdict=fail（1 blocker + 4 high），T7 不具备验收前提。审计报告：`docs/evidence/audit-t6.md`。
- **本阶段任务卡**: `docs/T6-REPAIR-TASK-CARD.md`（范围/非目标/验收/风险/回滚/技能，动手前必读）。
- **P0 范围**: T6-06 healthcheck 假阴性 → T6-03 compose `server` 服务 → T6-01 真实文件摄入最小闭环（multipart + 安全落盘 + 流式 SHA256 去重 + 最小元数据提取 + 前端文件选择）。
- **本阶段完成定义**: L1（build/vet/race + 新增路径安全单测）与 L2（`docker compose up` 后 meilisearch healthy + server running + 真实 curl 上传/检索闭环，证据落 `docs/evidence/l2-integration-t6-repair.md`）；**不含 L3**。
- **门禁**: 完成后由 **T6′（新会话，独立 strong 档）** 复审 T6-01/03/06；verdict=pass 才进 T7。
- **禁区**: 不删除既有数据/容器卷内容；不做认证/RBAC；不改 MCD 口径；不触碰生产/凭据/外部发布。
- **未完成项**（留待后续阶段，勿在本阶段顺手做）: T6-02 `scripts/benchmark-1m.sh` + 100 万实测（P1）、T6-04 指标口径（P1）、T6-05 前端虚拟滚动网格/属性过滤（P1）、T6-07…T6-16（P2）。
- **P0 执行结果（2026-09-12）**: ✅ **L2 已验证，L3 未跑**。证据：`docs/evidence/l2-integration-t6-repair.md`。
  - T6-06 已修（meilisearch 从 unhealthy → healthy，根因 `localhost`→`::1` 而 Meili 仅监听 IPv4）；
  - T6-03 已修（新增 `Dockerfile` + compose `server` 服务 + `depends_on: service_healthy`，三容器全 green；`vendor/` 入库以保证容器内离线构建）；
  - T6-01 已修（新增 `internal/storage`（内容寻址 + 流式 SHA256 + 扩展名白名单 + 大小上限 + 前缀/符号链接校验 + 图片尺寸）与 `POST /api/v1/assets/upload`（multipart），前端加文件选择/上传；实测上传→去重→入库→Meili 检索→标注任务全链路无回归，既有 50 万索引数据完好）；
  - 顺带：三服务端口绑定收敛为 `127.0.0.1`（审计 T6-08 的暴露面）。
  - **未做**：P1（T6-02/04/05）与 P2；**T6′ 独立复审未执行**——门禁仍不放行。

## 12. 当前阶段：T6′（独立复审 → 阻断项修复）

- **触发**: T6-REPAIR P0+P1 完成后需独立复审方可进 T7（`docs/T6-REPAIR-P1-TASK-CARD.md` §3）。
- **复审结论（2026-09-12）**: **verdict = fail**（2 blocker + 4 high）。报告：`docs/evidence/audit-t6-prime.md`。
  - 安全面（独立）`pass`；性能与证据真实性面（独立）`pass`（T6-02/T6-04 **关闭**：两次独立复现 p95≈45ms，余量 ≥2×）；
  - **前端与检索契约面（独立）`fail`**：C2-1 响应契约失配（前端读 `assets`、后端返 `results` → 列表恒空态）、C2-2 `sort` 必 502、C2-3 仅 `sort` 静默丢弃、C2-4 PG 路径缺 `total`、C2-5 深分页 1000 硬墙、C2-6 命中字段不全（`Invalid Date`）、C2-7 MIME 过滤未实现、C2-8 单测零断言力、C2-9 排序无 UI。
  - T6-01（后端/安全面）、T6-03、T6-06、T6-08 经独立实证**确认修好**；**T6-05 未关闭**（虚拟滚动/懒加载网格仍未实现，MIME 过滤缺）。
- **修复结果（2026-09-12，本轮）**: C2-1/C2-2/C2-3/C2-4/C2-6 已修 + C2-8 测试空洞已补。证据：`docs/evidence/l2-integration-t6-prime-fix.md`；回归脚本 `scripts/browser-regression.mjs`；截图 `docs/verification/t6-prime-fix/`。
- **T6″ 独立复核（2026-09-12，本轮）**: **verdict = pass（6/6 项成立）**。复核员自行复现排序真序（desc/asc 双向单调）、契约键、400 校验、19 个测试的断言力，并尝试证伪未果；另在实况索引上确认 `sortableAttributes` 确实生效。新发现 3 条：Meili `total` 截断（=C2-5，仍打开）、`SortableFields` 注释与实现不符（**已修**，仅注释）、PG 中穿越测试残留资产名（已核实仅为展示名字符串，存储层无越界）。完整裁决见 `docs/evidence/audit-t6-prime.md` §11。
- **门禁状态**: **不放行**（原始 `verdict=fail` 不变）。原因：① **C2-5 仍打开**（Meili `maxTotalHits=1000`：`total` 被截到 1000，`offset>1000` 静默返回空列表 + HTTP 200，与"百万资产可检索"口径冲突）；② T7 前提缺口（下方）未决。
- **T7 前提缺口（需用户决策）**: MCD 闭环的「人工分类/文本标签」「人工确认 AI 建议并写入标注」「预览」在代码中不存在——`Store` 无标签写入方法、全仓无 `INSERT INTO tags/asset_tags`、路由表仅 8 条无确认端点、无文件读取/缩略图端点；实测库中 `tags=0`/`asset_tags=0`（8 资产 / 6 已完成任务）。属 T2/T4 时代历史缺口，非本次修复引入。
- **残留项（更新于 2026-09-13，见 §13）**: C2-5 **已修**（`l3-mcd-acceptance.md`）、A2-01…A2-06 **已修**（`l2-integration-p2-hardening.md`）；**仍打开**：C2-7/C2-9（MIME 过滤 UI / 排序 UI）、B-01/B-02/B-06（证据文档口径修正）、A2-07…A2-09（info 级）；`gofmt` 已全绿（`cmd/bench`、`internal/worker` 已格式化）。
- **恢复事件（本轮）**: 审计员首派无产出失败 2/4（安全面、前端契约面各 1 次），收紧提示词后重派成功；初次 `go build/vet/test` 因默认 `GOCACHE` 被沙箱拒绝而产生**假绿**（管道吃掉了真实退出码），改 `GOCACHE=/tmp/...` 并以文件重定向取退出码后得真绿；`docker compose build` 需写工作区外 `~/.docker/buildx`，需一次性提权。

## 13. 当前阶段：P2 安全加固批收尾（A2-01…A2-06）

- **触发**: `docs/evidence/audit-t6-prime.md` §3 安全面 A2-01…A2-06（P2）+ §12 残留项；用户 2026-09-13 决策「收尾 P2 安全加固批」。
- **任务卡**: `docs/P2-HARDENING-TASK-CARD.md`（ADAC 风险分级 C、AC 矩阵、UV 场景、接口语义与回滚）。
- **交付（机制）**:
  - A2-01 上传改 `MultipartReader` 流式解析，文件部分直接进 `storage.Save`，不再经 `ParseMultipartForm` 落容器 `/tmp`；
  - A2-02 新增 `GET /readyz`（PG/Meili/存储逐项探测，任一不可达 503），`/healthz` 保持纯存活语义；新增 `search.Client.Health`、`storage.Store.Healthy`；
  - A2-03 请求体上限 = `store.MaxBytes() + 1MiB`（`MAX_UPLOAD_BYTES` 调小真实生效）；
  - A2-04 预览前核对内容与声明 MIME（不一致 415）+ `CSP … sandbox`；**并修掉初版逐字比较导致的 `.md`/`.csv`/`.json` 误拒**（改文本家族收敛判定）；
  - A2-05 入库/回读失败 → `storage.Discard` 清理孤儿文件（去重对象不删）；
  - A2-06 上传端点用 `ResponseController` 放宽本次请求的读写期限（5min / 5min+30s），服务端级常规超时保持 15s/15s/30s；**顺带解除 `WriteTimeout=30s` 这道第二墙**；
  - 工程卫生：`gofmt -l cmd internal` 为空。
- **验证**: L1 `build/vet/test -race` 三绿 + **16 个新用例**（含反向验证：还原旧实现后新测试确实失败）；L2 `scripts/l2-p2-hardening.sh` **60/60 PASS**（`75fd604` 为 53/53，修复提交新增 S8 七项）；L3 回归 T7 脚本 **8/8 PASS**（静态托管 :8080，consoleErrors=0、badResponses=0，截图用修复版重采）。证据：`docs/evidence/l2-integration-p2-hardening.md` + `docs/evidence/audit-p2-hardening.md`。
- **复审（2026-09-13）**: 计划派 3 名 fresh-context 独立审计员（安全面、探针/超时面、证据真实性面），**三次全部无产出失败**（1 次只剩状态栏文字、1 次卡在工具参数序列化坏循环、1 次空消息）；按纪律改为主 Agent 自查并标注**非独立**。
  - 报告：`docs/evidence/audit-p2-hardening.md`（verdict 分面、findings F1–F6、证伪尝试清单、盲区）。
  - **复审发现并已修 1 个 medium 缺陷（F6，`a6a6702`）**：file 部件已落盘、随后部件触发请求体上限时，已落盘对象无人清理 → 无 DB 行的孤儿文件（一次性容器实测 413 但对象存在）。修复后同场景 `NO_ORPHAN`，并新增单测 + L2 S8 常驻回归。
  - 其余 findings：F1（400 文案回显内部错误串）已修；F2（`/readyz` 回显内部拓扑）接受并记录；F3（元数据端点接受任意 path，读取面已中性化）、F4（A2-05 窄并发窗，未实测）待办；F5 接受。
  - 复核补测：**存储型 XSS 在执行层被拦下**（真实 Chromium：对照组脚本执行成功、预览端点被 CSP sandbox 拒绝）；`去重命中对象不误删` 由代码级说明升级为实测。
- **门禁**: ~~独立复审（AC-11）仍未满足~~ → **已满足（2026-09-13）**：三面 fresh-context 独立审计员全部返回且 verdict=pass（报告见 §9 末条与 §14）；`75fd604`/`a6a6702` 的实施者自证结论被独立复现确认。`audit-t6-prime.md` 原始 `verdict=fail` 与 T6″ 结论不因本批改变（历史记录保持原样）。
- **下一步建议（更新）**: ① ~~补一次真正独立的复审~~ **已完成**；② 清理库/卷内测试残留（`p2-*`、历史 `t4-l2.jpg`/`evil.png` 及本轮三面审计员新增的测试资产行/对象，需用户批准后实施）；③ **P2.1 修复批**：§14 的 4 medium（S1/S2/M1/F-I1）+ 低成本 low 项；④ C2-7/C2-9（MIME 过滤 UI / 排序 UI）与 B-01/B-02/B-06（证据口径 + 8080 产品链路基准）。

## 14. P2 独立复审残留 findings（2026-09-13，待排期）

> 来源：`docs/evidence/audit-p2-independent-{security,probe-timeout,evidence}.md`。三面 verdict 均 pass，以下为 accumulative 待修清单，**未在本轮修复**。

### medium（4）

| ID | 面 | 问题 | 位置/复现 | 修复方向 |
|---|---|---|---|---|
| S1 | 安全 | 回读失败路径 `Discard` 删掉已入库（inserted=true）行引用的对象 → 悬空 DB 行、预览 404 | `internal/api/handlers.go:624-635`（代码推演+审计员实测路径存在） | 改为删 DB 行或保留文件 |
| S2 | 安全 | multipart 部件头无独立上限：实测 8MB 部件头 201 接受；默认配置单请求头部内存 ≈ 请求体上限×并发 → DoS | `internal/api/handlers.go`（流式解析循环） | 限单部件头 64KiB + 部件数上限 |
| F-I1 | 探针 | `storage.Healthy()` 弱探针：仅 `os.Stat`，read-only `.tmp`/被替换为 regular file 时假阳性 healthy；docstring `not writable-ready` 名实不符 | `internal/storage`（审计员实测 5 case 中 2 假阳性，`.audit-tmp` 探针已自清） | 改为真实写探针（临时文件 create+delete）+ 磁盘空间下限 |
| M1 | 证据 | `scripts/l2-p2-hardening.sh` S5/S8 一次性容器绑宿主 `/tmp`，跨环境（/tmp 对 docker daemon 不可见）必败，可移植性风险 | 脚本 S5/S8 段 | 改命名卷 |

### low（9）

- S3（安全）：`.md` 内容含 `<script>` 时入库 `mime=text/html` 且预览内联 200，唯一脚本防线 CSP sandbox；建议上传侧拒 text/html 嗅探或强制 attachment。
- S4（安全）：并发同内容上传 Stat→Rename 竞态，一方 insert 失败可 Discard 他方引用的共享对象（推演，窗口小）。
- S5（安全）：`.tmp` 无启动/定期清扫，崩溃残留永久留存。
- F-I2（探针）：`/readyz` 在 compose 部署无人消费（compose 不支持 readiness），仅对 K8s readinessProbe 有用。
- F-I3（探针）：`MAX_UPLOAD_BYTES` 未在 `docker-compose.yml` 暴露。
- F-I4（探针）：`allowSlowUpload` 静默吞 `SetReadDeadline/WriteDeadline` 错误，未来中间件改动会静默退化且无日志。
- F-I5（文档）：`docs/P2-HARDENING-TASK-CARD.md:23,39` A2-06 描述与代码不符（实为 server 级 15s 保持、仅上传端点按请求放宽）。
- L1（证据）：单测新增数文档口径 14/15/16 互不一致，git 实为 18 个新测试函数——低估非夸大。
- L2（证据）：`scripts/l3-mcd-acceptance.mjs` 的 `ok` 判定漏 `badResponses`（AC-08 要求其为 0）。

### info（3）

- F-I6：`readyz_test.go` 缺 PG 失败路径单测；F-I7：`/readyz` 错误串泄露内部拓扑（127.0.0.1 only 可接受，上反代前需改二元）；F-I8：`storage.Healthy` docstring 名实不符（随 F-I1 修）。

### 盲区（审计员声明，主 Agent 转录）

- CSP sandbox 浏览器真实行为未实测；F6 未在 live 复现（依赖单测+探针面 live 等价复现）；PG/Meili 中途故障未 live `docker stop` 验证（禁破坏容器）；历史 53/53 基线无法回放。
- **未做（勿顺手做）**: 认证/RBAC、A2-07…A2-09、前端 415 文案提示、MCD 口径变更。
