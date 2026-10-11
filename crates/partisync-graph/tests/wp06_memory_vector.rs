//! M11-WP06-T03/T04 探针（SPEC M11-WP06 §3；P25 记忆域向量通道契约，
//! docs/tests/properties.md:32 登记行）：(b) key 域隔离——key 仅由
//! memory_id 派生（blake3 前 8B）、独立索引实例、同 memory_id upsert
//! 幂等不增条目；(c) 生命周期同步——预计算向量注入 → tombstone → GC
//! 物理删后向量通道不可召回该 memory_id；(d) 降级显式——向量腿不可用
//! 返回 BM25 结果 + 降级标记，零静默零 panic。
//!
//! 【T03 判例边界，T04 已接线】(c) 的 T03 探针为手工驱动生命周期腿
//! 原语（store.rs 挂点当时未接线）；T04 起新增 `p25c_full_chain_*`
//! 全链探针——tombstone/GC 由 store 自动摘除向量，探针只断言契约终态。
//!
//! 无模型：向量全部为预计算合成正交基（单位基向量 e_i，cos(e_i,e_j)=0
//! i≠j、自距离 0），嵌入器为测试注入的合成实现（`MemoryEmbedder`
//! trait），不触 fastembed（`graph-embed` feature 默认关，SPEC §2.1
//! 嵌入与落库分离判定的可测性收益）。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use partisync_core::error::{PartisyError, Severity};
use partisync_graph::{
    memory_vector_key, MemoryEmbedder, MemoryVectorIndex, Store, DEGRADED_VECTOR_UNAVAILABLE,
    MEMORY_VECTOR_DIM,
};
use serde_json::json;
use sqlx::Row;

/// 预计算合成向量：第 `i` 维单位基向量（512 维，两两正交）。
fn basis_vec(i: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; MEMORY_VECTOR_DIM];
    v[i] = 1.0;
    v
}

/// 进程内唯一临时目录（不引新 dev 依赖：tempfile 非 workspace pin，
/// Cargo.toml 顶层依赖区零变更红线——SPEC §5 末行；用完尽力清）。
fn temp_artifact_dir(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "partisync-graph-wp06-{}-{nanos}-{tag}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("创建临时目录");
    dir
}

async fn seeded_store() -> Store {
    let store = Store::open_in_memory().await.expect("打开内存库");
    store
        .seed_device_volume("dev-a", "Device A", "fp-a")
        .await
        .expect("播种设备");
    store
}

/// 模拟嵌入伴生落账（T04 接线前，伴表行由探针注入；status 2=done 沿
/// sidecar_items 五态机，v19 schema 判例）。
async fn seed_companion_done(store: &Store, memory_id: &str) {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0);
    sqlx::query(
        "INSERT INTO memory_embedding (memory_id, status, dim, detail, updated_ns)
         VALUES (?, 2, ?, 'probe', ?)",
    )
    .bind(memory_id)
    .bind(i64::try_from(MEMORY_VECTOR_DIM).unwrap_or(i64::MAX))
    .bind(ns)
    .execute(store.pool_ref())
    .await
    .expect("伴表落账");
}

async fn companion_count(store: &Store, memory_id: &str) -> i64 {
    sqlx::query("SELECT count(*) AS n FROM memory_embedding WHERE memory_id = ?")
        .bind(memory_id)
        .fetch_one(store.pool_ref())
        .await
        .expect("伴表计数")
        .get::<i64, _>("n")
}

// ---------------------------------------------------------------------------
// P25 (b) key 域隔离：key 仅由 memory_id 派生 + 同 id upsert 幂等不增条目
// ---------------------------------------------------------------------------

/// P25-b：同 memory_id 双写 upsert 幂等（条目数不变 + 覆盖生效），
/// 不同 memory_id 派生不同 key（key 仅由 memory_id 决定）。
#[tokio::test]
async fn p25b_key_domain_upsert_idempotent() {
    // key 派生：确定性 + 仅由 memory_id 决定（不同 id → 不同 key；
    // blake3 前 8B 大端 u64 逐式对照在 src/memory_vector.rs 单测）
    assert_eq!(memory_vector_key("m-1"), memory_vector_key("m-1"));
    assert_ne!(memory_vector_key("m-1"), memory_vector_key("m-2"));

    let dir = temp_artifact_dir("idempotent");
    let index = MemoryVectorIndex::open_or_create(dir.join("memory_vec.usearch"))
        .expect("打开记忆域向量索引");
    assert_eq!(index.size(), 0, "新建索引空");

    let id = "01JMEM00000000000000000000";
    index.upsert(id, &basis_vec(0)).expect("首写");
    assert_eq!(index.size(), 1, "首写一叶");
    // 双写（换一条预计算向量，模拟重嵌入覆盖）：P25-b 同 id upsert 不增条目
    let rewritten = basis_vec(1);
    index.upsert(id, &rewritten).expect("重写");
    assert_eq!(index.size(), 1, "同 id 双写幂等不增条目");
    assert!(index.contains(id).expect("在位判定"), "条目在位");

    // 覆盖生效：重写后的向量自查询距离 0（Cos 距离，同向量 → 0）
    let hits = index.search(&rewritten, 10).expect("自查询");
    assert!(!hits.is_empty());
    assert_eq!(hits[0].key, memory_vector_key(id), "命中即本 id 的 key");
    assert!(
        hits[0].distance.abs() < 1e-4,
        "覆盖后自距离 0，实际 {}",
        hits[0].distance
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// P25-b：独立索引实例物理隔离——两个 MemoryVectorIndex 实例互不可见
/// （与资产域 content key 域隔离同理：独立实例 = 独立条目空间，
/// ADR-0035 决策 5 混域出局）。
#[tokio::test]
async fn p25b_independent_instances_isolated() {
    let dir = temp_artifact_dir("isolated");
    let a = MemoryVectorIndex::open_or_create(dir.join("a.usearch")).expect("实例 A");
    let b = MemoryVectorIndex::open_or_create(dir.join("b.usearch")).expect("实例 B");

    a.upsert("id-in-a", &basis_vec(2)).expect("写 A 实例");
    assert_eq!(a.size(), 1, "A 实例一叶");
    assert_eq!(b.size(), 0, "B 实例不共享 A 的条目（独立实例）");
    b.upsert("id-in-b", &basis_vec(3)).expect("写 B 实例");

    // A 的近邻检索只见 A 自己的条目
    let hits = a.search(&basis_vec(2), 10).expect("A 检索");
    assert_eq!(hits.len(), 1, "A 实例近邻检索不含 B 实例条目");
    assert_eq!(hits[0].key, memory_vector_key("id-in-a"));

    // 产物文件物理分离（独立 artifact 面）
    assert!(dir.join("a.usearch").exists(), "独立 artifact A");
    assert!(dir.join("b.usearch").exists(), "独立 artifact B");
    std::fs::remove_dir_all(&dir).ok();
}

/// P25-b：artifact 持久化往返（save → open_or_create(restore) → 条目与
/// 检索不回退）——独立 memory_vec.usearch artifact 的可重建基线。
#[tokio::test]
async fn p25b_artifact_restore_roundtrip() {
    let dir = temp_artifact_dir("roundtrip");
    let path = dir.join("nested").join("memory_vec.usearch");
    {
        let index = MemoryVectorIndex::open_or_create(&path).expect("首开（父目录自建）");
        index.upsert("m-roundtrip", &basis_vec(4)).expect("写入");
        index.save(&path).expect("落盘");
    }
    let reopened = MemoryVectorIndex::open_or_create(&path).expect("restore 重开");
    assert_eq!(reopened.size(), 1, "restore 后条目数不回退");
    assert!(
        reopened.contains("m-roundtrip").expect("在位"),
        "restore 后 key 在位"
    );
    let hits = reopened.search(&basis_vec(4), 10).expect("自查询");
    assert_eq!(hits[0].key, memory_vector_key("m-roundtrip"));
    assert!(hits[0].distance.abs() < 1e-4, "F32 量化自距离 ~0");

    // 维度契约 fail-fast（SPEC §2.2）：≠512 拒绝写入与查询
    assert!(reopened.upsert("m-bad", &[0.0f32; 511]).is_err());
    assert!(reopened.upsert("m-bad", &[0.0f32; 513]).is_err());
    assert!(reopened.search(&[0.0f32; 511], 10).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------
// P25 (c) 生命周期同步：预计算向量注入 → tombstone → GC 物理删 → 近邻检索
// 不含该 memory_id
// ---------------------------------------------------------------------------

/// P25-c：写入（预计算向量注入）→ 软删墓碑 → 生命周期摘除 → GC 物理删
/// （伴表随 FK CASCADE 级联清，v19 判例沿 v18 content_chunk）→ 近邻检索
/// 不含该 memory_id，未删项召回不回退。
#[tokio::test]
async fn p25c_tombstone_gc_lifecycle_excludes_id() {
    let store = seeded_store().await;
    let m1 = store
        .memory_write("生命周期探针行 A 语义内容", &["p25".into()], &json!({}))
        .await
        .expect("写 A")
        .memory_id;
    let m2 = store
        .memory_write("生命周期探针行 B 另一语义内容", &["p25".into()], &json!({}))
        .await
        .expect("写 B")
        .memory_id;

    // 预计算向量注入（嵌入与落库分离，SPEC §2.1）+ 伴表落账
    let dir = temp_artifact_dir("lifecycle");
    let index = MemoryVectorIndex::open_or_create(dir.join("memory_vec.usearch"))
        .expect("打开记忆域向量索引");
    index.upsert(&m1, &basis_vec(10)).expect("注入 A 向量");
    index.upsert(&m2, &basis_vec(20)).expect("注入 B 向量");
    seed_companion_done(&store, &m1).await;
    seed_companion_done(&store, &m2).await;

    assert_eq!(index.size(), 2, "两叶在位");
    assert!(index.contains(&m1).expect("A 在位"));
    assert!(index.contains(&m2).expect("B 在位"));
    assert_eq!(companion_count(&store, &m1).await, 1, "A 伴表在账");

    // 写入期召回基线：A 自查询命中自身（key 比对还原 memory_id）
    let hits = index.search(&basis_vec(10), 10).expect("注入后自查询");
    assert_eq!(hits[0].key, memory_vector_key(&m1), "写入期召回");

    // 软删墓碑（tombstone，不动根不动向量行——承诺面语义 P20；向量行的
    // 摘除属于生命周期腿，此处手工驱动原语 = T04 store.rs 挂点接线位）
    store.memory_delete(&m1).await.expect("删 A");
    let row = store.memory_by_id(&m1).await.expect("查 A").unwrap();
    assert_eq!(row.deleted, 1, "软删墓碑在案");
    assert!(row.hlc.is_some(), "墓碑行 hlc 推进（GC 定年判据）");

    // 生命周期腿原语：tombstone 摘除后向量通道不可召回（P25-c tombstone 口径）
    assert!(index.remove(&m1).expect("摘除 A"), "实有条目摘除");
    assert!(!index.contains(&m1).expect("已摘"), "摘除后不在索引");
    let hits = index.search(&basis_vec(10), 10).expect("摘除后近邻检索");
    assert!(
        !hits.iter().any(|h| h.key == memory_vector_key(&m1)),
        "tombstone 摘除后近邻检索不含该 memory_id"
    );
    assert!(
        hits.iter().any(|h| h.key == memory_vector_key(&m2)),
        "未删项召回不回退"
    );
    // 摘除幂等：重复摘除返回 false（重放安全）
    assert!(!index.remove(&m1).expect("二次摘除"), "重复摘除幂等");

    // GC 物理删：墓碑回填远古水位（wp04_memory.rs:319-324 判例口径）后
    // retention=0 清除；伴表行随 FK CASCADE 级联清（v19 派生数据判例）
    let ancient = format!("{:016x}-{:08x}-{:016x}", 1_000u64, 0u32, 1u64);
    sqlx::query("UPDATE memory SET hlc = ? WHERE memory_id = ?")
        .bind(&ancient)
        .bind(&m1)
        .execute(store.pool_ref())
        .await
        .expect("回填远古 hlc");
    let report = store.memory_gc(0).await.expect("GC");
    assert_eq!(report.purged, 1, "过保留期墓碑物理清除");
    assert!(
        store.memory_by_id(&m1).await.expect("查 A").is_none(),
        "GC 后 memory 行物理清除"
    );
    assert_eq!(
        companion_count(&store, &m1).await,
        0,
        "伴表行随 FK CASCADE 级联清（GC 物理删后嵌入状态不可残留）"
    );
    assert_eq!(companion_count(&store, &m2).await, 1, "未删项伴表不动");

    // GC 后向量通道终态：不可召回 + 幂等 GC 空转
    assert!(!index.contains(&m1).expect("终态判位"));
    let hits = index.search(&basis_vec(10), 10).expect("GC 后近邻检索");
    assert!(
        !hits.iter().any(|h| h.key == memory_vector_key(&m1)),
        "GC 物理删后向量通道不可召回该 memory_id（P25-c 终态）"
    );
    let again = store.memory_gc(0).await.expect("二次 GC");
    assert_eq!(again.purged, 0, "GC 幂等空转");

    std::fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------
// M11-WP06-T04：store 三腿接线探针——P25(c) 全链（store 自动摘除）+
// P25(d) 降级两场景 + hybrid 融合正控 + §2.2 init fail-fast
// ---------------------------------------------------------------------------

/// 合成嵌入器（测试注入 `MemoryEmbedder`）：按映射表回向量，未登记内容
/// 报错（可作 init fail-fast / 降级场景触发器）。
struct TableEmbedder {
    vectors: HashMap<String, Vec<f32>>,
}

impl TableEmbedder {
    fn new(entries: Vec<(&str, Vec<f32>)>) -> Self {
        Self {
            vectors: entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        }
    }
}

impl MemoryEmbedder for TableEmbedder {
    fn embed(&self, content: &str) -> Result<Vec<f32>, PartisyError> {
        self.vectors
            .get(content)
            .cloned()
            .ok_or_else(|| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("TableEmbedder 无 {content:?} 映射").into()),
            })
    }
}

/// 恒失败嵌入器（模拟模型缓存缺失/初始化失败——SPEC §2.4 降级触发器）。
struct FailingEmbedder;

impl MemoryEmbedder for FailingEmbedder {
    fn embed(&self, _content: &str) -> Result<Vec<f32>, PartisyError> {
        Err(PartisyError {
            severity: Severity::Fatal,
            source: Some("模型缓存缺失（探针模拟）".into()),
        })
    }
}

/// 运行期翻转失败嵌入器：init 探针通过（flag=false），运行期置位后失败
/// ——覆盖「init 已过、运行期模型故障」的 fail-open 分支。
struct FlakyEmbedder {
    fail: Arc<AtomicBool>,
}

impl MemoryEmbedder for FlakyEmbedder {
    fn embed(&self, content: &str) -> Result<Vec<f32>, PartisyError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(PartisyError {
                severity: Severity::Fatal,
                source: Some("运行期模型故障（探针模拟）".into()),
            });
        }
        // 确定性合成：内容字节和选基向量（512 维内）
        let idx = content.bytes().map(usize::from).sum::<usize>() % MEMORY_VECTOR_DIM;
        Ok(basis_vec(idx))
    }
}

/// 维度漂移嵌入器（SPEC §2.2 init fail-fast 触发器：首嵌 511 维）。
struct DimDriftEmbedder;

impl MemoryEmbedder for DimDriftEmbedder {
    fn embed(&self, _content: &str) -> Result<Vec<f32>, PartisyError> {
        Ok(vec![0.0f32; MEMORY_VECTOR_DIM - 1])
    }
}

/// T04 写入腿伴生（确定性路径）：attach 后写入 → `memory_embed_one` →
/// 伴表 done + 向量在索引；通道缺位 store 返回 Ok(false)（no-op 判定位）。
#[tokio::test]
async fn t04_write_leg_companion_embeds_and_records() {
    let store = seeded_store().await;
    let m = store
        .memory_write("伴生嵌入探针行", &["t04".into()], &json!({}))
        .await
        .expect("写入")
        .memory_id;
    // 通道未挂载：embed_one no-op（Ok(false)）——零行为变更判定位
    assert!(!store.memory_embed_one(&m).await.expect("未挂载 no-op"));

    let dir = temp_artifact_dir("companion");
    let embedder = Arc::new(TableEmbedder::new(vec![
        ("", basis_vec(0)),               // attach init 探针（EMBED_PROBE_CONTENT = ""）
        ("伴生嵌入探针行", basis_vec(5)), // 写入内容
    ]));
    store
        .attach_vector_channel(dir.join("memory_vec.usearch"), Some(embedder))
        .expect("挂载通道");
    assert!(
        store.memory_embed_one(&m).await.expect("伴生嵌入"),
        "嵌入落库成功"
    );
    // 伴表 done（status=2，dim=512）
    let (status, dim): (i64, i64) =
        sqlx::query_as("SELECT status, dim FROM memory_embedding WHERE memory_id = ?")
            .bind(&m)
            .fetch_one(store.pool_ref())
            .await
            .expect("伴表读取");
    assert_eq!(status, 2, "伴表 done");
    assert_eq!(dim, i64::try_from(MEMORY_VECTOR_DIM).unwrap_or(i64::MAX));
    std::fs::remove_dir_all(&dir).ok();
}

/// SPEC §2.2 init fail-fast：首嵌维度 ≠ 512 → 拒绝挂载报错（模型漂移
/// 防线）；拒绝后查询腿降级（fail-open 与 fail-fast 的不对称，§2.4 ③）。
#[tokio::test]
async fn t04_attach_dim_drift_refused() {
    let store = seeded_store().await;
    let dir = temp_artifact_dir("dimdrift");
    let r = store.attach_vector_channel(
        dir.join("memory_vec.usearch"),
        Some(Arc::new(DimDriftEmbedder)),
    );
    assert!(r.is_err(), "首嵌维度 ≠ 512 拒绝挂载（SPEC §2.2 fail-fast）");
    // 拒绝后通道保持未挂载 → 查询腿降级而非报错（§2.4 不对称设计）
    let rep = store
        .memory_search_hybrid("任意查询", None, 10)
        .await
        .expect("hybrid 不 panic");
    assert_eq!(rep.degraded, Some(DEGRADED_VECTOR_UNAVAILABLE));
    std::fs::remove_dir_all(&dir).ok();
}

/// P25(d) 场景一（SPEC §3）：向量 artifact 缺失（通道未挂载）→ hybrid
/// 返回 BM25 结果 + 显式降级标记，零静默零 panic；BM25 通道输出与默认
/// 路径逐项一致（SPEC §2.0 零变更）。
#[tokio::test]
async fn p25d_degraded_artifact_missing() {
    let store = seeded_store().await;
    let a = store
        .memory_write("artifact missing 蓝色天空探针行", &[], &json!({}))
        .await
        .expect("写 A")
        .memory_id;
    let b = store
        .memory_write("artifact missing 绿色草地探针行", &[], &json!({}))
        .await
        .expect("写 B")
        .memory_id;

    // ≥3 字符 → FTS trigram 路径（LIKE 兜底路径为既有精确匹配语义，
    // store.rs 2592 原样——零变更红线，探针按 FTS 路径构造正/负控）
    let hybrid = store
        .memory_search_hybrid("蓝色天空", None, 10)
        .await
        .expect("hybrid 零 panic");
    assert_eq!(
        hybrid.degraded,
        Some(DEGRADED_VECTOR_UNAVAILABLE),
        "显式降级标记（不静默）"
    );
    let bm25 = store
        .memory_search(Some("蓝色天空"), None, None, 10, 0)
        .await
        .expect("BM25 默认路径");
    // BM25 通道行为零变更（SPEC §2.0）：结果与降级标记下的输出一致
    let hybrid_ids: Vec<String> = hybrid.results.iter().map(|h| h.memory_id.clone()).collect();
    let bm25_ids: Vec<String> = bm25.results.iter().map(|h| h.memory_id.clone()).collect();
    assert_eq!(hybrid_ids, bm25_ids, "降级输出 = BM25 原样");
    assert_eq!(hybrid.total, bm25.total);
    assert!(
        hybrid_ids.contains(&a),
        "FTS trigram 正控：词面命中行在降级输出中"
    );
    assert!(!hybrid_ids.contains(&b), "负控：词面未命中行不在降级输出");
}

/// P25(d) 场景二（SPEC §3）：模型不可用两分支——①init 失败（模型缓存
/// 缺失）→ attach 拒绝 + hybrid 降级；②init 已过、运行期失败 → hybrid
/// 降级 + 伴表 failed 记账。零静默零 panic。
#[tokio::test]
async fn p25d_degraded_model_unavailable() {
    // ① init 失败：attach 拒绝（fail-fast）→ 通道缺位 → hybrid 降级
    let store = seeded_store().await;
    let dir = temp_artifact_dir("modelfail");
    let r = store.attach_vector_channel(
        dir.join("memory_vec.usearch"),
        Some(Arc::new(FailingEmbedder)),
    );
    assert!(r.is_err(), "init 失败拒绝挂载（SPEC §2.2）");
    let rep = store
        .memory_search_hybrid("蓝色", None, 10)
        .await
        .expect("hybrid 零 panic");
    assert_eq!(rep.degraded, Some(DEGRADED_VECTOR_UNAVAILABLE));

    // ② 运行期失败：init 通过 → 正常态（degraded=None，索引空非降级）
    // → 运行期置失败 → hybrid 降级 + 伴表 failed（status=4）
    let store2 = seeded_store().await;
    let fail = Arc::new(AtomicBool::new(false));
    store2
        .attach_vector_channel(
            dir.join("flaky.usearch"),
            Some(Arc::new(FlakyEmbedder {
                fail: Arc::clone(&fail),
            })),
        )
        .expect("init 通过挂载");
    let rep_ok = store2
        .memory_search_hybrid("任意", None, 10)
        .await
        .expect("hybrid 正常态");
    assert_eq!(rep_ok.degraded, None, "通道可用 = 非降级（索引空属正常）");
    fail.store(true, Ordering::SeqCst);
    let rep_fail = store2
        .memory_search_hybrid("任意", None, 10)
        .await
        .expect("hybrid 运行期失败零 panic");
    assert_eq!(rep_fail.degraded, Some(DEGRADED_VECTOR_UNAVAILABLE));
    // 伴生嵌入失败 → 伴表 failed 记账（status=4 + detail），不上抛不 panic
    let m = store2
        .memory_write("运行期失败伴生行", &[], &json!({}))
        .await
        .expect("写入")
        .memory_id;
    assert!(
        !store2
            .memory_embed_one(&m)
            .await
            .expect("失败记账返回 false"),
        "嵌入失败不上抛"
    );
    let (status, detail): (i64, String) =
        sqlx::query_as("SELECT status, detail FROM memory_embedding WHERE memory_id = ?")
            .bind(&m)
            .fetch_one(store2.pool_ref())
            .await
            .expect("伴表 failed 行");
    assert_eq!(status, 4, "伴表 failed 记账");
    assert!(!detail.is_empty(), "失败原因落账");
    std::fs::remove_dir_all(&dir).ok();
}

/// hybrid 查询腿正控（SPEC §2.5）：向量腿把 BM25 零命中的语义近邻带入
/// 结果（RRF 融合排序），degraded=None。
#[tokio::test]
async fn t04_hybrid_fuses_bm25_and_vector() {
    let store = seeded_store().await;
    let dir = temp_artifact_dir("hybrid");
    // A 词面命中 "blue"；B 词面不命中但语义向量与查询向量一致
    let a_content = "alpha blue sky 探针";
    let b_content = "beta green grass 探针";
    let embedder = Arc::new(TableEmbedder::new(vec![
        ("", basis_vec(0)),         // attach init 探针
        (a_content, basis_vec(10)), // A 内容向量
        (b_content, basis_vec(20)), // B 内容向量
        ("blue", basis_vec(20)),    // 查询 "blue" 嵌入 = B 的向量（词面/语义分歧构造）
    ]));
    store
        .attach_vector_channel(dir.join("memory_vec.usearch"), Some(embedder))
        .expect("挂载通道");
    let a = store
        .memory_write(a_content, &[], &json!({}))
        .await
        .expect("写 A")
        .memory_id;
    let b = store
        .memory_write(b_content, &[], &json!({}))
        .await
        .expect("写 B")
        .memory_id;
    assert!(store.memory_embed_one(&a).await.expect("嵌 A"));
    assert!(store.memory_embed_one(&b).await.expect("嵌 B"));

    let rep = store
        .memory_search_hybrid("blue", None, 10)
        .await
        .expect("hybrid");
    assert_eq!(rep.degraded, None, "向量腿已参与，无降级");
    let ids: Vec<String> = rep.results.iter().map(|h| h.memory_id.clone()).collect();
    assert!(
        ids.contains(&b),
        "B 被 BM25 词面零命中、经向量腿带入（语义召回）"
    );
    assert!(ids.contains(&a), "A 经 BM25 腿召回");
    assert_eq!(rep.total, 2, "融合命中数");
    // RRF 排序：A = 1/60（BM25 rank0）+ 1/61（向量 rank1）> B = 1/60
    assert_eq!(ids[0], a, "A 融合分最高");
    assert!(
        rep.results[0].score > rep.results[1].score,
        "results 按融合分降序"
    );
    // tag 过滤同 BM25 口径：tag 不匹配的向量腿候选被过滤
    let rep_tag = store
        .memory_search_hybrid("blue", Some("nomatch-tag"), 10)
        .await
        .expect("hybrid tag 过滤");
    assert!(
        !rep_tag
            .results
            .iter()
            .any(|h| h.memory_id == b || h.memory_id == a),
        "tag 过滤同时约束两腿候选"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// P25(c) 全链探针（T04 store 接线后）：写入（伴生嵌入）→ tombstone
/// （store 自动摘除向量）→ GC 物理删（伴表 FK CASCADE 级联清）→ hybrid
/// 近邻检索不含该 memory_id、未删项召回不回退——全链无手工原语驱动。
#[tokio::test]
async fn p25c_full_chain_store_wired_lifecycle() {
    let store = seeded_store().await;
    let dir = temp_artifact_dir("fullchain");
    let a_content = "fullchain A 语义内容探针";
    let b_content = "fullchain B 语义内容探针";
    let embedder = Arc::new(TableEmbedder::new(vec![
        ("", basis_vec(0)),
        (a_content, basis_vec(10)),
        (b_content, basis_vec(20)),
        ("q-A", basis_vec(10)), // 查询向量 = A 的向量
        ("q-B", basis_vec(20)), // 查询向量 = B 的向量
    ]));
    store
        .attach_vector_channel(dir.join("memory_vec.usearch"), Some(embedder))
        .expect("挂载通道");
    let a = store
        .memory_write(a_content, &[], &json!({}))
        .await
        .expect("写 A")
        .memory_id;
    let b = store
        .memory_write(b_content, &[], &json!({}))
        .await
        .expect("写 B")
        .memory_id;
    assert!(store.memory_embed_one(&a).await.expect("嵌 A"));
    assert!(store.memory_embed_one(&b).await.expect("嵌 B"));
    assert_eq!(companion_count(&store, &a).await, 1, "A 伴表在账");

    // 写入期召回基线：A、B 均可经向量腿召回
    let rep = store
        .memory_search_hybrid("q-A", None, 10)
        .await
        .expect("基线 hybrid");
    let ids: Vec<String> = rep.results.iter().map(|h| h.memory_id.clone()).collect();
    assert!(ids.contains(&a), "写入期召回 A");
    assert_eq!(rep.degraded, None);

    // ① tombstone：store 生命周期腿自动摘除向量（探针零手工驱动）
    store.memory_delete(&a).await.expect("删 A");
    let rep = store
        .memory_search_hybrid("q-A", None, 10)
        .await
        .expect("tombstone 后 hybrid");
    assert!(
        !rep.results.iter().any(|h| h.memory_id == a),
        "tombstone 后向量通道不可召回（P25-c，store 自动摘除）"
    );
    // 伴表行仍在（软删不级联）但候选映射过滤 deleted=0 → 不召回
    assert_eq!(companion_count(&store, &a).await, 1, "软删不动伴表行");

    // ② GC 物理删：行清除 + 伴表 FK CASCADE 级联清 + 向量终态不可召回
    let ancient = format!("{:016x}-{:08x}-{:016x}", 1_000u64, 0u32, 1u64);
    sqlx::query("UPDATE memory SET hlc = ? WHERE memory_id = ?")
        .bind(&ancient)
        .bind(&a)
        .execute(store.pool_ref())
        .await
        .expect("回填远古 hlc");
    let report = store.memory_gc(0).await.expect("GC");
    assert_eq!(report.purged, 1, "过保留期墓碑物理清除");
    assert!(store.memory_by_id(&a).await.expect("查 A").is_none());
    assert_eq!(companion_count(&store, &a).await, 0, "伴表随级联清");
    let rep = store
        .memory_search_hybrid("q-A", None, 10)
        .await
        .expect("GC 后 hybrid");
    assert!(
        !rep.results.iter().any(|h| h.memory_id == a),
        "GC 物理删后近邻检索不含该 memory_id（P25-c 终态）"
    );

    // ③ 未删项召回不回退 + GC 幂等
    let rep = store
        .memory_search_hybrid("q-B", None, 10)
        .await
        .expect("B 召回");
    assert!(
        rep.results.iter().any(|h| h.memory_id == b),
        "未删项 B 召回不回退"
    );
    assert_eq!(rep.degraded, None);
    let again = store.memory_gc(0).await.expect("二次 GC");
    assert_eq!(again.purged, 0, "GC 幂等空转");

    std::fs::remove_dir_all(&dir).ok();
}
