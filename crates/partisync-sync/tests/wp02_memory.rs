//! M9-WP02-T03 收敛探针（SPEC M9-WP02 §2.5 / §3；P6/P19 判例延伸）：
//! memory 行随既有 push/bisync 管线多端收敛——零新增同步代码路径。
//!
//! 验收口径：双端各自 memory_write → bisync 不动点 → 双端 `memory_rows`
//! 逐行一致且 root 相等；同 id 双端独立写（created_ns/origin 簿记异）
//! 收敛一行、根一致（叶不含 hlc/deleted，LWW 胜者行决定承诺集）。

use partisync_core::Ulid;
use partisync_graph::store::Store;
use partisync_sync::{capture, session};

async fn node(tag: &str, device: &str) -> Store {
    let dir = std::env::temp_dir().join(format!("wp02-mem-{tag}-{}", Ulid::now()));
    std::fs::create_dir_all(&dir).unwrap();
    let s = Store::open(&dir.join("t.db")).await.unwrap();
    s.seed_device_volume(device, device, device).await.unwrap();
    s
}

/// 双端可见状态收敛比较口径：全部 memory 行（按 memory_id 升序）+ 根快照。
async fn memory_state_of(store: &Store) -> (Vec<partisync_graph::MemoryRow>, Option<String>) {
    let rows = store.memory_rows().await.unwrap();
    let root = store.memory_root_snapshot().await.unwrap().map(|s| s.root);
    (rows, root)
}

#[tokio::test]
async fn p20_bisync_two_writers_converge_root_equal() {
    let a = node("a", "dev-a").await;
    let b = node("b", "dev-b").await;

    a.memory_write("a 侧事实", &["src:a".into()], &serde_json::json!({"k": 1}))
        .await
        .unwrap();
    b.memory_write("b 侧事实", &["src:b".into()], &serde_json::json!({}))
        .await
        .unwrap();

    let stats = session::bisync(&a, &b, session::BisyncOpts::default())
        .await
        .unwrap();
    assert_eq!(stats.rounds, 2, "两行各一个方向：第 2 轮应到不动点");

    let (rows_a, root_a) = memory_state_of(&a).await;
    let (rows_b, root_b) = memory_state_of(&b).await;
    assert_eq!(rows_a.len(), 2);
    assert_eq!(rows_a, rows_b, "双端 memory 行逐行一致");
    assert_eq!(root_a, root_b, "收敛后证明树根相等（P20-a 延伸）");
    assert!(root_a.is_some());
    assert!(a.verify_memory().await.unwrap().ok);
    assert!(b.verify_memory().await.unwrap().ok);

    // 收敛后再捕获 + 单向 push：根不变（hlc 不进叶），对端跟进一致
    let id = rows_a[0].memory_id.clone();
    capture::record_memory_upsert(&a, &id).await.unwrap();
    let root_after = memory_state_of(&a).await.1;
    assert_eq!(root_a, root_after, "重捕获只升 hlc 水位，承诺根不变");
    let s2 = session::push(&a, &b).await.unwrap();
    assert_eq!(s2.applied, 1);
    let (rows_b2, root_b2) = memory_state_of(&b).await;
    assert_eq!(rows_b2, memory_state_of(&a).await.0);
    assert_eq!(root_b2, root_after, "重捕获传播后根仍相等");
}

#[tokio::test]
async fn p20_same_id_independent_writes_converge_one_row() {
    let a = node("a", "dev-a").await;
    let b = node("b", "dev-b").await;

    // 同 (content, tags, metadata) 双端独立写：内容寻址同 id，簿记异
    // （created_ns / origin_device 各取本端）→ HLC LWW 收敛一行
    let out_a = a
        .memory_write("共同事实", &["shared".into()], &serde_json::json!({}))
        .await
        .unwrap();
    let out_b = b
        .memory_write("共同事实", &["shared".into()], &serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(out_a.memory_id, out_b.memory_id, "内容寻址同 id");
    assert!(!out_a.deduplicated && !out_b.deduplicated);
    let row_a = a.memory_by_id(&out_a.memory_id).await.unwrap().unwrap();
    let row_b = b.memory_by_id(&out_b.memory_id).await.unwrap().unwrap();
    assert_ne!(row_a.origin_device, row_b.origin_device, "簿记异前提");
    assert_ne!(row_a.created_ns, row_b.created_ns, "簿记异前提");

    session::bisync(&a, &b, session::BisyncOpts::default())
        .await
        .unwrap();

    let (rows_a, root_a) = memory_state_of(&a).await;
    let (rows_b, root_b) = memory_state_of(&b).await;
    assert_eq!(rows_a.len(), 1, "同 id 双端独立写收敛一行");
    assert_eq!(rows_a, rows_b, "双端终态同一胜者行（含簿记字段）");
    assert_eq!(root_a, root_b, "胜者行唯一 ⇒ 叶集与根双端一致");
    assert!(a.verify_memory().await.unwrap().ok);
    assert!(b.verify_memory().await.unwrap().ok);
}

#[tokio::test]
async fn capture_memory_upsert_missing_target_is_fatal() {
    let a = node("a", "dev-a").await;
    let err = capture::record_memory_upsert(&a, "no-such-id")
        .await
        .expect_err("不存在目标必须 Fatal");
    assert!(matches!(
        err.severity,
        partisync_core::error::Severity::Fatal
    ));
}
