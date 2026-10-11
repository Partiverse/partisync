# Task: M11-WP06-T03 记忆域向量通道底座——schema v19 + usearch 包装 + key 域 + P25(b)(c) 探针

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP06-T03 |
| **类型** | graph 底座面实施任务（SPEC M11-WP06 §2.1/§2.2/§3 探针 b + §5 graph 底座面；零 MCP/store.rs 接线——查询腿/写入腿/生命周期腿自动挂点属 T04） |
| **来源** | SPEC M11-WP06 §2.1（落点 graph 独立通道 + key 域 memory 专享）、§2.2（dim=512 维度契约）、§3（P25 (b) key 域隔离单测 + (c) 生命周期探针，无模型，预计算向量注入）；ADR-0035 决策 1/4/5 |
| **创建日期** | 2026-10-11 |
| **执行日期** | 2026-10-11 |
| **口径** | usearch 2.26.2 pinned 源码核实后使用（cargo registry `usearch-2.26.2/rust/lib.rs`：Key=u64 :47 / new :1340 / restore :1369 / save :1714 / add :1507 / remove :1626 / contains :1659 / search :1449 / reserve :1555；MetricKind::Cos=距离 :281）；key 派生沿 content_key 判例（index/search/vector.rs:22-28）；RRF 沿 hybrid.rs:85-117 判例（k=60，本地实现 = ADR-0035 决策 4 登记的 ~20 行重复债务） |

## 交付

①schema.sql **v19 additive-only 迁移**：`memory_embedding` 嵌入状态伴表
（memory_id PK + REFERENCES memory ON DELETE CASCADE、status 五态机沿
sidecar_items 口径 0=pending/1=running/2=done/3=skipped/4=failed、dim
维度审计面、detail、updated_ns + status 索引）——幂等创建走 schema.sql
`CREATE TABLE IF NOT EXISTS`（migrate() raw_sql 重跑即达，v16/v18 判例
同式：纯新表无 ALTER 需求，故 migrate() 末段无新增代码段）；CASCADE 沿
v18 content_chunk 判例（GC 物理删父行时派生伴表随行级联清，无级联时 FK
炸 GC 路径——v18 对抗评审同源教训）。②`memory_vector.rs` 新模块：
`memory_vector_key`（blake3(memory_id) 前 8B 大端 u64，memory 专享 key
域）、`MemoryVectorIndex`（独立 usearch 实例、dim 512 常量
MEMORY_VECTOR_DIM、Cos/F32、upsert 幂等覆盖 / remove 幂等摘除 /
contains / search / save / open_or_create(restore)、维度契约 fail-fast）、
`rrf_fuse`（k=60；并列裁决 = memory_id 字典序升序，较判例 Equal 透传更
严，无模型探针可稳定断言）、预计算向量注入接口（嵌入与落库分离，SPEC
§2.1）。③lib.rs 模块声明 + 导出；Cargo.toml usearch 依赖边（既有
workspace pin，Cargo.lock 仅边登记行，deny.toml / workspace 顶层依赖区
零变更）。④P25 (b)(c) 无模型探针 tests/wp06_memory_vector.rs（4 测试，
正交基合成向量，零 fastembed 触碰）。

## 涉及文件清单（Iron Rule 9）

crates/partisync-graph/src/schema.sql（v19 伴表）·
crates/partisync-graph/src/memory_vector.rs（新）·
crates/partisync-graph/src/lib.rs（模块声明 + 导出）·
crates/partisync-graph/Cargo.toml（usearch 依赖边）·
crates/partisync-graph/tests/wp06_memory_vector.rs（新，探针）·
docs/tasks/M11-WP06-T03-graph-base.md（本卡）· Cargo.lock（边登记自动行）

## 验收（SPEC §3 对应项；2026-10-11）

- [x] key 域隔离单测（无模型）：派生函数断言（确定性/仅由 memory_id 决定/blake3 前 8B 逐式对照 src 内单测）+ 同 id 双写 upsert 后条目数不变（`p25b_key_domain_upsert_idempotent`）+ 独立实例物理隔离（`p25b_independent_instances_isolated`）+ artifact restore 往返（`p25b_artifact_restore_roundtrip`）
- [x] 生命周期探针（无模型，预计算向量注入）：写入→tombstone→摘除→GC 物理删→近邻检索不含该 memory_id，未删项召回不回退，GC 幂等空转；伴表随 FK CASCADE 级联清（`p25c_tombstone_gc_lifecycle_excludes_id`）；软删除（未 GC）行为按 tombstone 口径在探针内断言（墓碑在案 deleted=1 + hlc 推进 + 摘除后不可召回）并登记
- [x] 默认行为回归：cargo test -p partisync-graph 全绿（30 测试：lib 3 + jobs 2 + journal 4 + store 6 + wp02 10 + wp04 5 + wp06 4），零断言放宽零删测试
- [x] 门禁：cargo fmt --all --check 绿 / cargo clippy -p partisync-graph --all-targets -- -D warnings 绿 / cargo test -p partisync-graph 绿（2026-10-11 实跑）
- [x] 依赖红线：deny.toml 零变更、workspace Cargo.toml 顶层依赖区零变更（usearch=既有 pin 的 crate 级依赖边 + Cargo.lock 边登记行，ADR-0035 决策 1 依赖图新边登记兑现）
- [x] usearch 2.26.2 pinned 源码查证在案（模块头注释逐 API 列行号），非凭记忆
- [x] 诚实边界登记（见下）

## 诚实边界（不做/后置，随任务卡登记）

- **P25 (d) 降级探针本任务不做**——memory_search 降级标记透传面（SPEC
  §2.4/§3）依赖 T04 查询腿接线，T04 接线后补。
- **生命周期腿自动挂点不在本任务**——store.rs tombstone/GC 挂点、
  memory_search 查询腿融合接线、写入腿异步伴生均属 T04（SPEC §5
  store.rs/mcp.rs/partisd 面）；本任务探针手工驱动生命周期腿原语
  （`MemoryVectorIndex::remove` + v19 CASCADE），锁定契约终态。
- **fastembed 依赖边 + `graph-embed` feature 不在本任务**——SPEC §5
  Cargo.toml 面的 fastembed 边随 T04 嵌入调用面落地（本任务零嵌入代码，
  usearch 边先行为 P25(b)(c) 无模型探针的唯一依赖前提）。
- **`memory_vec.usearch` artifact 落盘位置未定**——由宿主 partisd 决定，
  T04 接线；本任务提供 open_or_create/save 原语与 restore 往返探针。
- **伴表写入方未接线**——v19 表在 T03 仅有 schema 面（探针内手工落账
  模拟伴生）；产品写入方（异步伴生/批量回填器）属 T04/examples 面。
