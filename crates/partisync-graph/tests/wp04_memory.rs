//! M10-WP04-T01 探针（SPEC M10-WP04 §2.1/§3 T01；P20 措辞注记判例延伸，
//! wp02_memory.rs 判例）：软删墓碑不动根 / update 墓碑旧+写新 / 复活 /
//! verify 实态旗标（deleted/tombstones）透出 / oplog ("memory","delete") 面。

use partisync_graph::memory::inclusion_proof;
use partisync_graph::Store;
use serde_json::json;

async fn seeded_store() -> Store {
    let store = Store::open_in_memory().await.expect("打开内存库");
    store
        .seed_device_volume("dev-a", "Device A", "fp-a")
        .await
        .expect("播种设备");
    store
}

/// §3-T01 删除语义：delete 后 search 三路径（精确 id / FTS / LIKE 兜底）
/// 均不见；verify(id) proof 仍过且 deleted=true；根值与删除前相等（软删
/// 不动根——叶不含 deleted/hlc）；count 不变（承诺集含墓碑，tombstones=1）；
/// oplog ("memory","delete") payload 恰含 memory_id，行 hlc 推进到 op 键；
/// 重复 delete 幂等（不产新 oplog、不推 hlc）。【P20 措辞注记】
#[tokio::test]
async fn t01_delete_tombstone_soft_root_stable() {
    let store = seeded_store().await;
    let out = store
        .memory_write("delete probe fact row 探针行", &["t".into()], &json!({}))
        .await
        .expect("首写");
    let id = out.memory_id.clone();
    let snap0 = store.memory_root_snapshot().await.expect("快照").unwrap();
    assert_eq!(snap0.memory_count, 1);

    store.memory_delete(&id).await.expect("删除");

    // search 三路径均不见
    assert!(
        store
            .memory_search(None, None, Some(&id), 20, 0)
            .await
            .expect("精确查")
            .results
            .is_empty(),
        "精确 id 路径不见墓碑"
    );
    assert!(
        store
            .memory_search(Some("probe"), None, None, 20, 0)
            .await
            .expect("FTS 查")
            .results
            .is_empty(),
        "FTS 路径不见墓碑"
    );
    assert!(
        store
            .memory_search(Some("探"), None, None, 20, 0)
            .await
            .expect("LIKE 查")
            .results
            .is_empty(),
        "LIKE 兜底路径不见墓碑"
    );

    // 行墓碑 + hlc 推进；proof 仍过且 deleted=true
    let row = store.memory_by_id(&id).await.expect("查行").unwrap();
    assert_eq!(row.deleted, 1, "软删墓碑");
    assert!(row.hlc.is_some(), "行 hlc 推进");
    let rows = store.memory_rows().await.expect("rows");
    let proof = inclusion_proof(&rows, &id).expect("墓碑行仍在承诺集");
    assert!(proof.ok, "墓碑行 proof 仍通过（删除不抹除承诺）");
    assert!(proof.deleted, "verify 实态旗标透出");

    // 软删不动根 + count 不变（承诺集含墓碑）
    let snap = store.memory_root_snapshot().await.expect("快照").unwrap();
    assert_eq!(snap.root, snap0.root, "软删不动根");
    assert_eq!(snap.memory_count, snap0.memory_count, "承诺集规模不变");
    let report = store.verify_memory().await.expect("全检");
    assert!(report.ok);
    assert_eq!(
        report.memory_count, snap0.memory_count as usize,
        "memory_count 含墓碑"
    );
    assert_eq!(report.tombstones, 1, "全检透出墓碑计数");

    // oplog ("memory","delete")：payload 恰含 memory_id；行 hlc = op 键
    let del = store
        .pending_oplog()
        .await
        .expect("oplog")
        .into_iter()
        .find(|r| r.entity == "memory" && r.op == "delete")
        .expect("delete oplog 行");
    assert_eq!(del.entity_id, id);
    assert_eq!(del.domain, 1, "共享域");
    let payload: serde_json::Value = serde_json::from_str(&del.payload).expect("payload JSON");
    assert_eq!(
        payload,
        json!({ "memory_id": id }),
        "payload 恰含 memory_id"
    );
    assert_eq!(
        row.hlc.as_deref(),
        Some(del.hlc.as_str()),
        "行 hlc = delete 键"
    );

    // 重复 delete 幂等：不推 hlc（无新 oplog 的等价断言）
    store.memory_delete(&id).await.expect("重复删除幂等");
    let row2 = store.memory_by_id(&id).await.expect("查行").unwrap();
    assert_eq!(row2.hlc, row.hlc, "已墓碑再删不推水位");
}

/// §3-T01 更新语义：update 后新 id 可检索、旧 id 墓碑且 search 不见；
/// canonical 等价 update 为 no-op（行数/根/hlc 均不变）；对不存在/已墓碑
/// id update 显式报错（不静默建行）。
#[tokio::test]
async fn t01_update_tombstones_old_writes_new() {
    let store = seeded_store().await;
    let a = store
        .memory_write("旧事实", &["a".into()], &json!({}))
        .await
        .expect("首写");
    let id_a = a.memory_id.clone();

    let upd = store
        .memory_update(&id_a, "新事实 v2 row", &["b".into()], &json!({ "v": 2 }))
        .await
        .expect("更新");
    assert!(!upd.deduplicated, "更新为状态变更");
    assert_ne!(upd.memory_id, id_a, "内容寻址 ⇒ 更新换身份");
    let id_b = upd.memory_id.clone();

    // 新 id 可检索；旧 id 墓碑且 search 不见
    assert_eq!(
        store
            .memory_search(None, None, Some(&id_b), 20, 0)
            .await
            .expect("新 id 查")
            .total,
        1,
        "新 id 可检索"
    );
    assert!(
        store
            .memory_search(None, None, Some(&id_a), 20, 0)
            .await
            .expect("旧 id 查")
            .results
            .is_empty(),
        "旧 id 墓碑不见于 search"
    );
    let rows = store.memory_rows().await.expect("rows");
    assert_eq!(rows.len(), 2, "同事务墓碑旧 id + 写入新行");
    let row_a = rows.iter().find(|r| r.memory_id == id_a).unwrap();
    assert_eq!(row_a.deleted, 1);
    let report = store.verify_memory().await.expect("全检");
    assert!(report.ok);
    assert_eq!(report.tombstones, 1);

    // canonical 等价（同 id）→ no-op：行数/根/hlc 均不变
    let root = store
        .memory_root_snapshot()
        .await
        .expect("快照")
        .unwrap()
        .root;
    let hlc_b = rows
        .iter()
        .find(|r| r.memory_id == id_b)
        .unwrap()
        .hlc
        .clone();
    let noop = store
        .memory_update(&id_b, "新事实 v2 row", &["b".into()], &json!({ "v": 2 }))
        .await
        .expect("等价更新");
    assert_eq!(noop.memory_id, id_b, "no-op 返回原 id");
    assert!(noop.deduplicated, "canonical 等价 no-op");
    assert_eq!(
        store.memory_rows().await.expect("rows").len(),
        2,
        "行数不变"
    );
    assert_eq!(
        store
            .memory_root_snapshot()
            .await
            .expect("快照")
            .unwrap()
            .root,
        root,
        "根不变"
    );
    let hlc_b2 = store.memory_by_id(&id_b).await.expect("查行").unwrap().hlc;
    assert_eq!(hlc_b2, hlc_b, "no-op 不推 hlc");

    // 目标不存在 / 已墓碑 → 显式报错
    assert!(
        store
            .memory_update("no-such-id", "x", &[], &json!({}))
            .await
            .is_err(),
        "不存在 id update 必报错"
    );
    let err = store
        .memory_update(&id_a, "又一次", &[], &json!({}))
        .await
        .expect_err("墓碑 id update 必报错");
    assert!(format!("{err:?}").contains("墓碑"), "错误面指向墓碑态");
}

/// §3-T01 复活语义：delete 后同参数 memory_write → 复活（deleted=0、
/// search 复见、verify deleted=false），返回 deduplicated=false；复活是
/// upsert 状态变更（oplog upsert 键回填行 hlc）；活行二次写 deduplicated=true
/// 语义不变；叶不含 deleted/hlc ⇒ 复活后根回删除前原值。【P20 措辞注记】
#[tokio::test]
async fn t01_write_revives_tombstone() {
    let store = seeded_store().await;
    let a = store
        .memory_write("复活探针 fact row", &["r".into()], &json!({}))
        .await
        .expect("首写");
    let id = a.memory_id.clone();
    let root0 = store
        .memory_root_snapshot()
        .await
        .expect("快照")
        .unwrap()
        .root;
    store.memory_delete(&id).await.expect("删除");

    let rev = store
        .memory_write("复活探针 fact row", &["r".into()], &json!({}))
        .await
        .expect("重写");
    assert_eq!(rev.memory_id, id, "同参数同身份");
    assert!(!rev.deduplicated, "复活非 deduplicated");
    let row = store.memory_by_id(&id).await.expect("查行").unwrap();
    assert_eq!(row.deleted, 0, "墓碑清除");
    assert_eq!(
        store
            .memory_search(None, None, Some(&id), 20, 0)
            .await
            .expect("search")
            .total,
        1,
        "search 复见"
    );
    let rows = store.memory_rows().await.expect("rows");
    let proof = inclusion_proof(&rows, &id).expect("行在承诺集");
    assert!(proof.ok && !proof.deleted, "verify 实态 = 活");
    let up = store
        .pending_oplog()
        .await
        .expect("oplog")
        .into_iter()
        .rev()
        .find(|r| r.entity == "memory" && r.op == "upsert")
        .expect("复活产 upsert oplog");
    assert_eq!(
        Some(up.hlc.as_str()),
        row.hlc.as_deref(),
        "复活键回填行 hlc"
    );
    assert_eq!(
        store
            .memory_root_snapshot()
            .await
            .expect("快照")
            .unwrap()
            .root,
        root0,
        "叶编码不含 deleted/hlc ⇒ 复活根回原值"
    );

    // 活行二次写幂等语义不变（store.rs:2100 既有路径）
    let again = store
        .memory_write("复活探针 fact row", &["r".into()], &json!({}))
        .await
        .expect("重写");
    assert!(again.deduplicated, "活行幂等");
    let row2 = store.memory_by_id(&id).await.expect("查行").unwrap();
    assert_eq!(row2.hlc, row.hlc, "活行幂等不推 hlc");
}
