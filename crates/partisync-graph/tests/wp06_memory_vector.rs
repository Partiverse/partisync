//! M11-WP06-T03 探针（SPEC M11-WP06 §3；P25 记忆域向量通道契约，
//! docs/tests/properties.md:32 登记行）：(b) key 域隔离——key 仅由
//! memory_id 派生（blake3 前 8B）、独立索引实例、同 memory_id upsert
//! 幂等不增条目；(c) 生命周期同步——预计算向量注入 → tombstone → GC
//! 物理删后向量通道不可召回该 memory_id。
//!
//! 【诚实边界】(c) 的向量摘除/伴表清理在本探针内为**手工驱动生命周期腿
//! 原语**（`MemoryVectorIndex::remove` + v19 伴表 FK CASCADE）——store.rs
//! 的 tombstone/GC 自动挂点属 T04 接线（SPEC §5 store.rs 面）；本探针
//! 锁定的是生命周期腿必须维持的契约终态（摘除后近邻检索不含该 id）。
//! (d) 降级探针留 T04 查询腿接线后补（memory_search 降级标记透传面，
//! SPEC §2.4/§3），本任务卡已注明。
//!
//! 无模型：向量全部为预计算合成正交基（单位基向量 e_i，cos(e_i,e_j)=0
//! i≠j、自距离 0），不触 fastembed（`graph-embed` feature 默认关，
//! SPEC §2.1 嵌入与落库分离判定的可测性收益）。

use partisync_graph::{memory_vector_key, MemoryVectorIndex, Store, MEMORY_VECTOR_DIM};
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
