# Task: M11-WP06-T05 memory_search hybrid 公共面 + MANIFEST 填实 + 嵌入宿主/回填器

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP06-T05 |
| **类型** | 公共面/宿主面实施任务（SPEC M11-WP06 §2.3/§2.5 公共面/§6-8/§5 gateway+partisd+ai+examples 面；T04 store 三腿接线之上的对外暴露与真嵌入备料） |
| **来源** | SPEC §2.3（MANIFEST BGESmallZHV15 五文件 blake3 逐字填实 + 常量对照单测）、§2.5（memory_search 可选参数 mode=hybrid + 降级标记透传 + instructions 同步 + MCP 契约探针 + partisd loopback e2e）、§6-8（partisd 宿主内内存队列/崩溃全量回填）、§2.6（回填器打印预算数字） |
| **创建日期** | 2026-10-11 |
| **执行日期** | 2026-10-11 |
| **口径** | fastembed 7.0.1 pinned 源码核实（cargo registry：TextEmbedding::try_new text_embedding/impl.rs:32、TextInitOptions builder init.rs:71-92、embed impl.rs:379、Embedding=Vec<f32> common.rs:32、BGESmallZHV15 models/text_embedding.rs:46、default feature hf-hub-native-tls 在位）；评估件 M11-WP05-t05 §2 候填哈希表逐字；rmcp 3.4.0 stateless 面 SEP-2575 meta 键核实（tower.rs:614-637 + model/meta.rs:394） |

## 交付

①**gateway/mcp.rs**：memory_search 工具 schema 增量可选参数
`mode: "bm25"|"hybrid"`（tools/list 透出，无参数请求 schema 与响应载荷
形状与现状一致——MemorySearchReport 序列化面零字段变更）；mode=hybrid
分派 store 层 `memory_search_hybrid`（memory_id 精确查在场沿现状优先；
hybrid 需 query；未知 mode 显式拒）；`MemoryHybridReport.degraded` 随
载荷透传（fail-open，SPEC §2.4）；instructions 文本同步；`pub async fn
install_memory_vector_channel`（partisd 宿主位挂载钩子）。MCP 契约单测
4 件（schema 透出/默认形状一致/hybrid 降级透传/mock 通道融合正控）。
②**partisd**：`--memory-embed [--memory-vec-artifact] [--model-cache]`
宿主位（feature `memory-embed` = dep graph + graph/graph-embed，默认
关）——启动时经 install_memory_vector_channel 挂载三腿激活；生命周期
语义（§6-8/ADR-0035 决策 3）代码注释登记：内存队列无持久化，崩溃/终止
丢队列，重启后回填器全量重算补齐；init fail-fast（嵌入器加载失败拒绝
启动，exit 1）。loopback e2e 探针 tests/wp06_surface.rs（tools/list
透出 + mode=hybrid 降级标记透传 + 无参数形状一致，全走 MCP 协议面）。
③**ai/models.rs**：MANIFEST v3 六条目——新增 EMBED_MODEL_TEXT_ZH
（BGESmallZHV15，source=Xenova/bge-small-zh-v1.5，blake3=权重主文件
onnx/model.onnx 哈希）；`BGE_SMALL_ZH_V15_FILES` 五文件钉版常量表
（评估件 §2 候填哈希表逐字）；常量对照单测 3 件（条目存在性 + 64 hex +
逐字对照 + 旧条目 None 欠账不顺手填）。④**graph**：Cargo.toml
fastembed 依赖边（optional）+ `graph-embed` feature（默认关，沿
ai-embed/index-embed 判例）+ `memory_vector::embed::FastembedMemoryEmbedder`
（feature 门控真嵌入器，Mutex<TextEmbedding> 串行化，维度 ≠512 输出
拒绝）；examples/backfill_memory_vectors.rs 批量回填器（dev-target，
required-features 门控）：按伴表状态全量扫描补齐（pending/running/
failed/未登记），嵌入 → usearch artifact → 伴表落账 → 打印预算数字
（artifact ≤1.5× 库主文件 / 耗时 ≤1800s / done/failed 计数）。

## 涉及文件清单（Iron Rule 9）

crates/partisync-gateway/src/mcp.rs · crates/partisd/src/main.rs ·
crates/partisd/Cargo.toml（memory-embed feature）·
crates/partisd/tests/wp06_surface.rs（新）·
crates/partisync-ai/src/models.rs ·
crates/partisync-graph/Cargo.toml（fastembed 边 + feature + example
声明）· crates/partisync-graph/src/memory_vector.rs（embed 模块）·
crates/partisync-graph/examples/backfill_memory_vectors.rs（新）·
Cargo.lock（optional 边登记自动行）· docs/tasks/M11-WP06-T05-surface.md
（本卡）。**deny.toml / workspace Cargo.toml 顶层依赖区零变更**
（fastembed=既有 pin Cargo.toml:58 的 crate 级依赖边，SPEC §5 末行）。

## 验收（SPEC §3 对应项；2026-10-11 勾闭）

- [x] MANIFEST 单测：2.3 常量对照断言绿（五文件哈希/字节数与评估件 §2 逐字一致）；models.rs 编译面 blake3 非 None（BGESmallZHV15 条目 Some；bge-m3/clip 欠账维持 None，SPEC §2.3 不强制）
- [x] MCP 契约探针：tools/list 中 memory_search schema 透出可选参数 mode（单测 + partisd loopback e2e 双面）；无参数请求响应载荷与现状形状一致断言（单测 + loopback）
- [x] 降级标记透传：mode=hybrid 通道未挂载 → BM25 结果原样 + degraded=vector_unavailable 经 MCP 协议面（单测 + loopback e2e）零静默零 panic
- [x] 默认行为回归：cargo test --workspace 全绿（116 个测试二进制零失败；gateway/partisd/ai/graph 既有套件零断言改动）
- [x] 门禁：cargo fmt --all --check / cargo clippy --workspace --all-targets -- -D warnings / cargo test --workspace 2026-10-11 实跑绿（隔离 worktree，CARGO_TARGET_DIR 共享缓存）；另实跑 clippy --features partisd/memory-embed,partisync-graph/graph-embed --all-targets -D warnings 绿（feature 门控面干净）
- [x] 依赖红线：deny.toml / workspace Cargo.toml 顶层依赖区零变更（fastembed/usearch 均既有 pin 的依赖边）
- [x] 任务卡诚实边界登记（见下）

## 诚实边界（不做/后置，随任务卡登记）

- **真模型 e2e 留 T06 实测**：memory-embed/graph-embed feature 默认关
  （模型下载/推理不入 CI，评估件 §7）；`--memory-embed` 真模型挂载的
  守护进程级 e2e、维度契约 ≥10 句 512 维归档（docs/reviews/）、语义
  收益 e2e 演示（BM25 0 命中 vs hybrid 命中）属 T06（评估件 §2 判例
  格式）。本 PR 的 mock/降级探针已锁定协议面与降级面。
- **真模型文件哈希重验为本地验收**（SPEC §2.3）：`BGE_SMALL_ZH_V15_FILES`
  逐字取自评估件 §2 冒烟实录；HF 下载腿（hf-mirror）协议兼容性复测与
  本地哈希重验留 T06（SPEC §6-3 风险登记）。
- **存储/时长实测报告未产出**（SPEC §3 实测项）：回填器已能打印预算
  数字（artifact vs 库主文件、耗时），10⁵ 叶库实测归档留 T06。
- **fastembed 嵌入为串行 Mutex**（embed &mut self）：批量回填吞吐受限于
  单线程逐句 ~3.5ms；批量 embed(texts, batch) 接口优化留后续（预算
  余量内，纸面 2.6min/10⁵）。
- **hybrid `total` 口径 = 融合命中数（截断前）**（T04 登记，gateway 面
  未对齐 memory_search 全库计数语义——如需对齐待评审）。
- **diff 规模登记**：~1070 insertions（含两新文件），超铁律 3 ≤400 行
  口径——gateway/partisd/ai/examples 四面互为验收前提（MCP 探针依赖
  store 面接口与 partisd 装配），判例 #231/#237/#238；请评审显式处置。
- 分支基线：m11/wp06-t04-store-wiring @e5544e5（T04 已合入 main squash
  303842d），叠加分支存活判例。

## 审查处置（2026-10-11 对抗审查，逐条；fix PR 落码）

- [x] **【阻断·生命周期】向量落盘恢复闭环**——落锤 = 审查建议 (a)+(b) 组合：
  ①daemon 关停 save——`MemoryVectorChannel` 登记挂载 artifact 路径 +
  `Store::save_memory_vector_artifact`（spawn_blocking 包同步 FFI IO）+
  gateway `save_memory_vector_artifact` 包装 + partisd clean-exit 路径
  调用（SIGTERM 优雅关停即落盘，save 失败 stderr 告警不阻断关停）；
  ②回填器对账重嵌——done 行逐 id `index.contains` 全量对账（缺失即
  重嵌；contains 实测 10⁵ 条目 ~0.2µs/次，临时基准 2026-10-11，与
  index crate vector.rs:170-173 判例的 30ms remove 口径区分），覆盖
  daemon 崩溃/save 失败窗口；`--no-reconcile` 显式跳过（登记用）。
  无模型探针 2 件锁定原语面：save 往返持久化（restore 在盘）+ 未落盘
  向量 contains 检出（回填器触发依据）。审查三条备选中未选 (c)（ADR
  登记不消除丢失，且 ADR-0035 决策 3 原承诺经 (a)+(b) 兑现，无需修订）。
- [x] **【次要·契约】hybrid offset 静默忽略**——透传落锤：`memory_search_hybrid`
  增 `offset: u32` 参（融合榜 skip(offset).take(limit)，total 口径 =
  融合命中数截断前不折减）；mcp.rs hybrid 分支传 `input.offset`；既有
  探针调用点全量更新（offset=0 语义不变）。T04 卡诚实边界同步改写。
- [x] **【次要·GC 泄漏】**——摘除序落锤：memory_gc 改为**先逐 id
  vector_lifecycle_remove、后 DELETE**——摘除失败即整体失败（墓碑行
  仍在、重试可续，remove 幂等），消灭「已物理删行向量永久残留 artifact」
  的窗口；残留后果（召回安全/artifact 膨胀）随代码注释登记。
- [x] **【次要·静默面】**——伴生任务级失败记账：spawn wrapper 对
  `embed_companion_task` 的 Err（含墓碑竞态兜底摘除失败）落伴表
  status=4 + detail（`companion task: {e}`）——零静默；失败行由回填器
  重试。
- [x] **【小疵】**——wp06_surface.rs 复制粘贴重复断言删除；backfiller
  Outcome 三态（Embedded/Failed/Skipped）——行缺失/墓碑计 Skipped 不再
  计入 failed，embed/upsert 失败才计 Failed（伴表 status=4）。
- [x] **contains 代价实证登记**：临时基准（zz_bench_contains，未入库）
  10⁵ 条目 upsert 85.7s、contains 0.2µs/次（1000 次命中/1000 次未命中
  均 <1ms）——index crate vector.rs:170-173 判例「contains 随规模超线性
  30ms@10⁴」的 contains 部分与本次实测不符（该口径为 remove）；全量
  contains 对账 @118k done 行 ≈ 24ms，回填预算内。
