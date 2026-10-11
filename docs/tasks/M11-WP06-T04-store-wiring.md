# Task: M11-WP06-T04 store 三腿接线 + fail-open 降级 + P25(c)(d) 探针

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP06-T04 |
| **类型** | store 面接线实施任务（SPEC M11-WP06 §2.0/§2.4/§2.5 写入时机/§3 探针 c(d)/§5 store 面；零 MCP 面——mode 参数透出与 instructions 文本属 gateway 任务） |
| **来源** | SPEC M11-WP06 §2.4（fail-open + 显式降级标记）、§2.5（BM25 Top-N + 向量近邻 Top-N → RRF、嵌入异步伴生不进写事务）、§3（P25 (c) 生命周期探针全链 + (d) 降级探针两场景）；ADR-0035 决策 2/3 |
| **创建日期** | 2026-10-11 |
| **执行日期** | 2026-10-11 |
| **口径** | 通道默认未挂载 = 全部新路径 no-op（BM25 通道逐字节零变更，SPEC §2.0）；宿主经 `attach_vector_channel` 显式挂载（init fail-fast = SPEC §2.2 与 §2.4 查询期 fail-open 的有意不对称）；key→id 映射 = blake3 单向，候选集 = 伴表 done 行 JOIN 活行逐 id 派生 key 比对 |

## 交付

①**查询腿**：`Store::memory_search_hybrid`（BM25 Top-N 走既有
`memory_search` 全路径零变更 + 向量近邻 Top-N → `rrf_fuse` k=60），
返回 `MemoryHybridReport { results, total, degraded }`（memory.rs 新
结构；score = RRF 融合分，total = 融合命中数截断前）；向量腿错误
（嵌入失败/维度漂移/usearch 错误）一律 fail-open 降级不上抛。②**写入
腿**：`memory_write` 事务提交后 `spawn_embed_companion`（tokio 异步伴生，
不进写事务，SPEC §2.5 写入时机）+ 确定性接口 `memory_embed_one` +
`embed_companion_task`（嵌入 → 向量落库 → v19 伴表 status 落账；失败记
status=4 + detail，不上抛不 panic——ADR-0035 决策 3 派生数据无一致性债）；
挂点覆盖 memory_write 复活/插入臂、memory_update 新行、apply_remote_memory
（远端 upsert 胜出 = 内容到达）。③**生命周期腿**：memory_delete /
memory_update 墓碑臂 / apply_remote_memory_delete / memory_gc 物理删
（先取目标 id 同判据）→ `vector_lifecycle_remove` 同步摘除向量（P25-c
全链；通道缺位 no-op，remove 幂等；伴表行随 v19 FK CASCADE 级联清）。
④**挂载面**：`attach_vector_channel`（artifact 打开 + init fail-fast
首嵌探针校验维度 ≠512/嵌入失败拒绝挂载）+ `MemoryEmbedder` trait +
`MemoryVectorChannel`（Send+Sync 编译期钉死）+ `DEGRADED_VECTOR_UNAVAILABLE`
常量（memory_vector.rs）。⑤**探针**（tests/wp06_memory_vector.rs 6 新
测试，合成嵌入器注入零模型）：P25(c) 全链（store 自动摘除，零手工
原语）、P25(d) 两场景（artifact 缺失 = 通道未挂载 → BM25 原样 + 降级
标记；模型不可用 = init 失败拒绝挂载 + 运行期翻转失败 → 降级 + 伴表
failed 记账）、hybrid 融合正控（B 被 BM25 词面零命中、经向量腿带入，
RRF 排序断言，tag 过滤约束两腿）、写入腿伴生确定性路径、init fail-fast
维度漂移拒绝。

## 涉及文件清单（Iron Rule 9）

crates/partisync-graph/src/store.rs（三腿接线 + attach + hybrid）·
crates/partisync-graph/src/memory.rs（MemoryHybridReport）·
crates/partisync-graph/src/memory_vector.rs（MemoryEmbedder/Channel/
DEGRADED 常量/探针内容常量）· crates/partisync-graph/src/lib.rs（导出）·
crates/partisync-graph/tests/wp06_memory_vector.rs（6 新探针 + 头注
边界更新）· docs/tasks/M11-WP06-T04-store-wiring.md（本卡）。
**Cargo.toml / Cargo.lock / deny.toml 零变更**（T03 已落 usearch 边）。

## 验收（SPEC §3 对应项；2026-10-11 勾闭）

- [x] 生命周期探针全链（无模型，预计算/合成向量注入）：写入→tombstone→GC 物理删→hybrid 近邻检索不含该 memory_id，未删项召回不回退，伴表随级联清，GC 幂等（`p25c_full_chain_store_wired_lifecycle`；T03 手工驱动探针 `p25c_tombstone_gc_lifecycle_excludes_id` 原样保留不放宽）
- [x] 降级探针两场景（无模型）：artifact 缺失（通道未挂载）+ 模型不可用（init 失败拒绝挂载 / 运行期失败翻转）→ BM25 结果原样 + `degraded="vector_unavailable"`，零静默零 panic（`p25d_degraded_artifact_missing` / `p25d_degraded_model_unavailable`）；运行期失败伴表 failed（status=4+detail）记账在案
- [x] 默认行为回归：cargo test -p partisync-graph 全绿 40 测试（lib 3 + jobs 2 + journal 4 + store 6 + wp02 10 + wp04 5 + wp06 10），现有 memory 套件（wp02/wp04）零断言改动零删测试；通道未挂载路径全部 no-op（BM25 零变更 SPEC §2.0）
- [x] 门禁：cargo fmt --all --check / cargo clippy -p partisync-graph --all-targets -- -D warnings / cargo test -p partisync-graph 2026-10-11 实跑绿（隔离 worktree + CARGO_TARGET_DIR 共享缓存）
- [x] 依赖红线：本 PR 零 Cargo.toml/Cargo.lock/deny.toml 变更
- [x] 任务卡诚实边界登记（见下）

## 诚实边界（不做/后置/发现，随任务卡登记）

- **MCP 面不在本任务**：`mode` 参数 schema 透出 / 降级标记载荷透传 /
  instructions 文本同步 = gateway mcp.rs 面（SPEC §5，gateway 任务）；
  store 层已提供 `memory_search_hybrid` + `MemoryHybridReport.degraded`。
- **fastembed 真嵌入器不在本任务**：`MemoryEmbedder` trait 就位，
  fastembed 实现 + `graph-embed` feature + MANIFEST blake3 填实 +
  真模型维度探针随 ai/models 面任务（SPEC §5 partisync-ai 面）。
- **partisd 宿主/批量回填器不在本任务**：ADR-0035 决策 3 的宿主内
  内存队列与 examples 批量回填器随 partisd/examples 面任务；本任务
  提供 `memory_embed_one` 确定性接口与 spawn 伴生挂点（进程内 tokio
  任务即最简队列原语）。
- **伴生任务为无界 spawn**（每写一笔一笔任务）：10⁵ 级批量写入走
  examples 批量回填器（不入 memory_write），量级风险登记，宿主任务
  接线时按 ADR-0035 决策 3 收口。
- **key→id 映射为候选集逐 id 派生比对**（伴表 done 行 JOIN 活行，
  O(向量数)/查询）：blake3 单向不可反解的既定代价；10⁵ 规模 ~10ms
  纸面（blake3 短输入），实测归档属存储/时长实测报告任务。
- **既有 LIKE 兜底路径为精确匹配语义**（memory_search like_escape 后
  不加 % 包裹，store.rs:2726 判例原样）——本任务发现并核实为既有
  行为，**不顺手修**（BM25 零变更红线 SPEC §2.0；如需修正另立任务）。
- **hybrid total 口径 = 融合命中数（截断前）**，与 memory_search 的
  全库匹配数语义不同——gateway 透出时如需对齐再议（登记待评审）。
