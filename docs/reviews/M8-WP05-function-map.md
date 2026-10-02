# PartiSync 功能逻辑树与映射全景（M8-WP05 依据文档）

> 任务: M8-WP05-T00 · 日期: 2026-10-02 · 性质: 设计依据文档（SPEC
> M8-WP05 起草输入；后续界面任务卡与评审的对照基准）· 现状盘点:
> HEAD `ba0112b`（desktop 子代理实测盘点，见 §5 复核日志）·
> 负责人: @lead · 执行: GLM-5.3-Flash (ZCode)

## 1. 功能逻辑树（产品功能 → 界面 → IPC → 后端）

```
PartiSync 桌面壳（crates/partisync-desktop）
├─ A. 统计概览（常驻 header 卡片区）
│   └─ A1 资产统计（7 卡：文件/目录/容量/唯一内容/去重节省/重复组/块级节省）
│       ├─ IPC: get_stats ──────────→ graph::Store（SQLite 权威元数据）
│       └─ IPC: cas_stats ──────────→ cas::ChunkStore（块库统计）
├─ B. 浏览（tab: browse）
│   └─ B1 目录浏览（面包屑 + 条目表：名/大小/mtime/内容身份前 8）
│       └─ IPC: list {prefix} ──────→ graph EntryRow（children 前缀查询）
│   └─ B2【WP05-T2】条目详情（内容身份/cas 引用/转写预览）
│       ├─ IPC: asset_detail（新增）→ graph 行 + cas 引用计数
│       └─ MCP: asset_read ─────────→ sidecar（OCR/Whisper 产物读回）
├─ C. 搜索（tab: search）
│   └─ C1 BM25 检索（现状唯一路径）
│       └─ IPC: search {q,limit} ───→ IndexEngine::bm25_only（懒加载）
│   └─ C2【WP05-T1 旗舰】语义混合检索（BM25+向量 RRF）
│       └─ IPC: search_hybrid（新增）→ IndexEngine::hybrid_search（await，
│                                   向量索引/嵌入模型懒加载，见 §4-N1）
│   └─ C3【WP05-T2】转写检索开关（OCR/Whisper 文本域）
│       └─ IPC: search {include_transcript:true} → Bm25Query 字段（后端
│                                   已支持，前端未暴露——零后端增量）
├─ D. 重复内容（tab: dups）
│   └─ D1 去重组列表（top 50）
│       └─ IPC: duplicates {top} ───→ graph 去重组查询
├─ E. 作业（tab: jobs）
│   └─ E1 作业列表（状态色标）
│       └─ IPC: jobs ───────────────→ graph::jobs::list
├─ F. 同步状态【WP05-T3 新界面】
│   └─ F1 同步统计（applied/skipped/conflicts）
│       └─ IPC: sync_stats（新增）──→ sync::SyncStats（session.rs 已有
│                                   结构体；desktop 目前不依赖
│                                   partisync-sync——§4-N2 新接线）
│   └─ F2 事件/对账最近活动
│       └─ IPC: sync_recent（新增）→ oplog/journal 尾部读取
│   └─ F3（联动）E1 作业列表即同步作业载体（jobs tab 数据复用）
├─ G. 扩展（tab: ext）
│   └─ G1 工具列表 + 调用沙盒（现状）
│       └─ MCP: ext_list / ext_<name> → sidecar（gateway ext.rs
│         ExtRegistry::scan ~/.partisync/extensions）→ ext-host
│         （wasmtime 47 + epoch/fuel 终止，M8-WP06）
│   └─ G2【WP05-T4】能力面展示 + 调用历史
│       └─ MCP: ext_list 载荷扩展（manifest capabilities 已在载荷内）
└─ H. 横切
    ├─ H1 全局错误条（{kind,msg} → #error-region，5s 自清）
    ├─ H2 窗口状态持久化（window-state.json）
    └─ H3 5s 轮询（stats + 当前 tab；无事件订阅）
```

## 2. 后端 crate 间关联（desktop 可达面）

```
partisync-desktop（壳）
├─ partisync-graph ──── 权威元数据（SQLite）：list/stats/jobs/duplicates
│   └─（写入方=引擎管线/同步，desktop 只读）
├─ partisync-cas ────── 块库：cas_stats/内容引用（desktop 只读）
├─ partisync-index ──── 检索：bm25_only（现状）→ hybrid_search（WP05）
│   ├─ 依赖嵌入模型（fastembed）——hybrid 路径加载重（§4-N1）
│   └─ 向量索引（usearch 子索引，M4-WP02/M6-D67 交付）
├─ partisync-mcp 侧车 ─ MCP 工具面：asset_search/read/organize/
│   │                    dataset_export/job_status/ext_*
│   ├─ partisync-gateway（lib 逻辑）→ index/ext-host
│   └─ ext-host（wasmtime 47 + epoch/fuel M8-WP06）
└─ partisync-sync【WP05-T3 新依赖】── SyncStats/oplog 状态
    └─（Hub/iroh 通道面不在 desktop 进程内——同步状态=本机引擎视角）
```

## 3. 前端内部关联（app-core.js）

- **共享状态**：`curPath`（browse 当前目录，5s 轮询保持）；tab 切换
  触发各自 load 函数（`data-tab` → 函数映射）；
- **双检索通道选择**（WP05-T1 设计点）：search tab 内 BM25/语义开关 →
  同一 `#q` 输入分别走 `search`/`search_hybrid`——**后端两查询路径
  结果结构对齐**（content_id/score/highlight vs HybridResult 字段映射
  在 IPC 层归一，前端零分支）；
- **错误传播**：所有 invoke 经 `call()` 包装统一 `{kind,msg}` →
  `showError`（WP05 新 command 必须沿用 error.rs 分类学）；
- **轮询成本**：5s 轮询仅 stats + 活跃 tab——sync 界面纳入同模式
  （不引入事件订阅，沿现状架构）。

## 4. 缺口与裁决（防低级逻辑错误清单）

| # | 缺口/风险 | 裁决 |
|---|---|---|
| N1 | **hybrid 检索的嵌入模型加载重**（fastembed 模型数百 MB），若在启动路径会击穿冷启动预算（M6-WP03-T04：P50 400ms < 1500ms 硬线） | `search_hybrid` 懒加载沿 `OnceCell<IndexEngine>` 现有模式扩展：**向量引擎独立 OnceCell**，首次切语义开关才加载；SPEC 验收含冷启动回归项 |
| N2 | **desktop 不依赖 partisync-sync**（同步状态无数据源） | T3 新增 sync 只读依赖（SyncStats/oplog 尾读），不引入 sync 写路径/网络面（Hub/iroh 不进 desktop 进程） |
| N3 | hybrid 与 BM25 结果结构不同（HybridResult vs SearchHit） | IPC 层归一为统一 SearchHit 形（score 口径差异在响应带 `mode` 字段标注） |
| N4 | include_transcript 后端已支持但前端未暴露 | T2 纯前端开关 + IPC 参数透传（零后端增量） |
| N5 | mcp_call（sidecar 通道）与直接 IPC（graph/index 通道）双通道并存 | 裁决：**高频读路径走直接 IPC**（stats/list/search），**能力聚合/扩展走 mcp_call**（asset_*/ext_*）——避免同功能双通道漂移；asset_read 详情读回走 mcp_call（转写/OCR 产物在 sidecar 能力域） |
| N6 | 空库/空索引边界（10 个既有测试覆盖空态） | 新 command 沿用 mock_builder 空态测试模板 |
| N7 | **GUI 验收硬性规则**（AGENTS.md 2026-10-02）：桌面端 PR 前须实际操控电脑验证 | T1–T4 各任务验收含 GUI 实操步骤 + 截图归档 docs/screenshots/ |

## 5. 复核日志

- 现状盘点（UI 5 tab/7 command/10 测试）由子代理实测产出（HEAD
  `ba0112b`）；hybrid_search/SyncStats/include_transcript 三个关键 API
  由主会话源码 grep 复核；N1–N7 裁决为设计判断（标注）。
