# TASK-CARD: partisync T6-REPAIR-P1（P1 项修复阶段）

> 上游输入：`docs/evidence/audit-t6.md`（T6 审计：T6-02, T6-04, T6-05 遗留 High 项）
> 前置基础：P0 已完成（T6-01 真实文件摄入，T6-03 compose 包含 server，T6-06 healthcheck 修复），见 `docs/evidence/l2-integration-t6-repair.md`。
> 本阶段目标：解决全部 P1 级缺陷（T6-02, T6-04, T6-05），为 T6' 独立复审及 T7（L3 浏览器闭环）创造完整条件。

## 1. 任务基础信息

- **任务名称**: T6-REPAIR-P1 — 100万基准脚本与压测闭环 + 前端多维属性过滤与性能网格
- **关联**: `docs/SESSION.md`、`docs/evidence/audit-t6.md`、`docs/evidence/benchmark-t5.md`
- **执行模式**: 智能模式（执行级硬门）
- **高风险标识**: **中**（涉及百万级索引构建压测，容器内存/磁盘占用监控；后端 API 查询参数增加 filter/sort）

## 2. P1 任务分解与规范

### 项一：T6-02 & T6-04 (100万真实压测与脚本自动化)
- **交付物**: `scripts/benchmark-1m.sh`
- **内容要求**:
  - 自动检查 MeiliSearch 连通性与健康状态；
  - 支持传入参数（`--only-search`, `--start`, `--count`, `--concurrency`, `--requests` 等）；
  - 支持增量索引或全量索引到 1,000,000 文档（当前库已有 500,003 条，可支持从 500,000 追加到 1,000,000，或全量重建并校验总数）；
  - 查询集扩充，包含高频、中频与长尾查询词，避免纯缓存假象；
  - 自动化输出 latency 直方图、p50/p90/p95/p99 及 RPS 指标；
  - 严格验证 100 万资产规模下搜索 p95 < 100ms 约束，保存报告至 `docs/evidence/benchmark-1m.md`。

### 项二：T6-05 (前端属性过滤与虚拟滚动/高效展示网格)
- **后端配合**:
  - `internal/api/handlers.go`：`GET /api/v1/assets` 增加 `resource_type`, `mime_type` 等过滤参数（传递给 Meilisearch filter 语法），以及排序参数；
  - `internal/search/meili.go`：支持带 `filter`（如 `resource_type = "image"`）与 `sort` 的搜索；
  - 补齐对应单元测试。
- **前端实现**:
  - `playground/src/lib/api.ts`：更新 `listAssets` 参数支持 filter/sort；
  - `playground/src/pages/assets.tsx`：
    - 多维属性过滤栏（按 resource_type 过滤：全部/图片/文档/视频；按 MIME 过滤；防抖搜索）；
    - 网格视图 / 表格视图切换（Cards / Grid 视图与 Table 视图）；
    - 视口分页与虚拟化/懒加载流式网格，提升海量数据渲染性能；
    - 保持前端 `tsc --noEmit` 0 错误。

## 3. 验收标准与检查层级

| 级别 | 验收方式 | 证据要求 |
|---|---|---|
| L1 | `go build ./... && go vet ./... && go test -race -count=1 ./...` 全绿；前端 `npm run build` / `tsc --noEmit` 0 错误 | 编译与测试日志 |
| L2 | 1. 运行 `scripts/benchmark-1m.sh`，Meilisearch 真实索引达到 1,000,000 条，50 并发 5000 次查询实测 p95 < 100ms；<br>2. `curl` 验证后端 `GET /api/v1/assets?resource_type=image&q=...` 过滤参数生效；<br>3. 前端界面过滤/网格交互无报错，L2 验证记录落地。 | `docs/evidence/benchmark-1m.md`<br>`docs/evidence/l2-integration-p1.md` |
| L3 | 为 T7 L3 Playwright 自动化提供完备功能入口 | 记录于文档 |
