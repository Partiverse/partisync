//! M9-WP02-T02 探针（P20 记忆层可验证承诺，SPEC §2.7/§3）：
//! 确定性（乱序同根）/ 可靠性（包含验证 + 三类单字节篡改必败 + 路径长对数界）
//! / 可重算（快照重建 + 篡改检出）/ 幂等写 / canonical JSON（§6-R2）。

use partisync_graph::memory::{
    audit_path, canonical_json, memory_identity, tree_root, verify_inclusion, MemoryWriteOutcome,
};
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

async fn write_row(store: &Store, i: usize) -> MemoryWriteOutcome {
    store
        .memory_write(
            &format!("fact-{i}: partisync 记忆探针行"),
            &["probe".to_string()],
            &json!({ "i": i }),
        )
        .await
        .expect("memory_write")
}

/// P20-a（纯函数）：根是行集的确定性函数——同集乱序同根；任一字段（含
/// created_ns/origin_device 溯源簿记）变动必异根。两库独立写同一事实集
/// 的根一致性归 T03 收敛探针（LWW 胜者行全等后同根）。
#[test]
fn p20a_root_deterministic_over_row_set() {
    let mk = |i: usize| partisync_graph::MemoryRow {
        memory_id: memory_identity(
            &format!("fact-{i}"),
            "[\"probe\"]",
            &format!("{{\"i\":{i}}}"),
        ),
        content: format!("fact-{i}"),
        content_hash: partisync_cas_content_digest(&format!("fact-{i}")),
        tags: "[\"probe\"]".into(),
        metadata: format!("{{\"i\":{i}}}"),
        created_ns: 1_000 + i as i64,
        origin_device: "dev-a".into(),
        hlc: None,
        deleted: 0,
    };
    let rows: Vec<_> = (0..40).map(mk).collect();
    let hash = |rs: &[partisync_graph::MemoryRow]| {
        let mut hs: Vec<[u8; 32]> = rs.iter().map(partisync_graph::memory::leaf_hash).collect();
        // 叶序 = memory_id 升序（SQL ORDER BY 的等价形态）
        let keys: Vec<String> = rs.iter().map(|r| r.memory_id.clone()).collect();
        let mut idx: Vec<usize> = (0..rs.len()).collect();
        idx.sort_by(|&x, &y| keys[x].cmp(&keys[y]));
        hs = idx.iter().map(|&i| hs[i]).collect();
        tree_root(&hs)
    };
    let mut reversed = rows.clone();
    reversed.reverse();
    assert_eq!(hash(&rows), hash(&reversed), "P20-a: 同集乱序必同根");
    let mut tampered = rows.clone();
    tampered[7].created_ns += 1; // 溯源簿记在承诺内
    assert_ne!(hash(&rows), hash(&tampered), "行字段变动必异根");
}

/// P20-a（身份面）+ P20-b：内容寻址身份与写入顺序无关；全叶包含验证 +
/// 路径长对数界。
#[tokio::test]
async fn p20ab_inclusion_holds_and_identity_order_independent() {
    let a = seeded_store().await;
    let b = seeded_store().await;
    // 同一事实集，两种插入顺序（a 正序 / b 倒序）
    for i in 0..40 {
        write_row(&a, i).await;
    }
    for i in (0..40).rev() {
        write_row(&b, i).await;
    }
    let rows_a = a.memory_rows().await.expect("rows a");
    let rows_b = b.memory_rows().await.expect("rows b");
    assert_eq!(rows_a.len(), 40);
    let mut ids_a: Vec<String> = rows_a.iter().map(|r| r.memory_id.clone()).collect();
    let mut ids_b: Vec<String> = rows_b.iter().map(|r| r.memory_id.clone()).collect();
    ids_a.sort();
    ids_b.sort();
    assert_eq!(ids_a, ids_b, "P20-a: 内容寻址身份与写入顺序无关");
    assert!(a.verify_memory().await.expect("verify a").ok);
    assert!(b.verify_memory().await.expect("verify b").ok);

    let hashes_a: Vec<[u8; 32]> = rows_a
        .iter()
        .map(partisync_graph::memory::leaf_hash)
        .collect();
    let root_a = tree_root(&hashes_a);
    for (i, leaf) in hashes_a.iter().enumerate() {
        let path = audit_path(&hashes_a, i).expect("路径存在");
        assert!(
            path.len() as u32 <= 7, // ⌈log2 40⌉ + 1 = 6 + 1
            "P20-b: 审计路径长 {i}={} 超对数界",
            path.len()
        );
        assert!(
            verify_inclusion(leaf, &path, i, 40, &root_a),
            "P20-b: 第 {i} 叶包含验证必成立"
        );
    }
}

/// P20-b：三类单字节篡改（leaf / audit_path / root）必败。
#[tokio::test]
async fn p20b_single_byte_tamper_always_fails() {
    let store = seeded_store().await;
    for i in 0..8 {
        write_row(&store, i).await;
    }
    let rows = store.memory_rows().await.expect("rows");
    let hashes: Vec<[u8; 32]> = rows
        .iter()
        .map(partisync_graph::memory::leaf_hash)
        .collect();
    let root = tree_root(&hashes);
    let idx = 3;
    let path = audit_path(&hashes, idx).expect("路径存在");

    // ① 篡改叶哈希
    let mut bad_leaf = hashes[idx];
    bad_leaf[0] ^= 0x01;
    assert!(!verify_inclusion(&bad_leaf, &path, idx, 8, &root));
    // ② 篡改路径任一元素
    for j in 0..path.len() {
        let mut tampered = path.clone();
        tampered[j][0] ^= 0x01;
        assert!(
            !verify_inclusion(&hashes[idx], &tampered, idx, 8, &root),
            "路径元素 {j}"
        );
    }
    // ③ 篡改根
    let mut bad_root = root;
    bad_root[0] ^= 0x01;
    assert!(!verify_inclusion(&hashes[idx], &path, idx, 8, &bad_root));
    // ④ 越界 index 不通过构造期（audit_path 返回 None 由本测试直查）
    assert!(audit_path(&hashes, hashes.len()).is_none());
}

/// SPEC §2.2 幂等写：同参数二次写 deduplicated=true，行数与根不变；
/// oplog 记录（entity="memory"）+ 行 hlc 回填。
#[tokio::test]
async fn memory_write_idempotent_same_identity() {
    let store = seeded_store().await;
    let first = write_row(&store, 1).await;
    assert!(!first.deduplicated);
    let root = store.refresh_memory_root().await.expect("root").root;

    let second = write_row(&store, 1).await;
    assert!(second.deduplicated);
    assert_eq!(second.memory_id, first.memory_id);
    let rows = store.memory_rows().await.expect("rows");
    assert_eq!(rows.len(), 1, "幂等写不新增行");
    assert_eq!(
        store.refresh_memory_root().await.expect("root").root,
        root,
        "根不变"
    );
    assert!(rows[0].hlc.is_some(), "行 hlc 随首写回填");

    // 不同 tags → 不同身份（内容寻址判定面）
    let other = store
        .memory_write(
            "fact-1: partisync 记忆探针行",
            &["different".to_string()],
            &json!({ "i": 1 }),
        )
        .await
        .expect("write");
    assert!(!other.deduplicated);
    assert_ne!(other.memory_id, first.memory_id);
}

/// §6-R2：metadata 键序 / tags 顺序不影响身份（canonical JSON 显式排序）。
#[tokio::test]
async fn r2_canonical_json_key_order_invariant() {
    let a = seeded_store().await;
    let b = seeded_store().await;
    a.memory_write(
        "事实",
        &["t1".into(), "t2".into()],
        &json!({ "b": 1, "a": 2 }),
    )
    .await
    .expect("write a");
    b.memory_write(
        "事实",
        &["t1".into(), "t2".into()],
        &json!({ "a": 2, "b": 1 }),
    )
    .await
    .expect("write b");
    let ra = a.memory_rows().await.expect("rows");
    let rb = b.memory_rows().await.expect("rows");
    assert_eq!(ra[0].memory_id, rb[0].memory_id, "键序不得影响身份");
    assert_eq!(ra[0].metadata, rb[0].metadata);
    assert_eq!(
        ra[0].tags, rb[0].tags,
        "tags canonical 字符串一致（BTreeMap 键序）"
    );
    // canonical_json 直查：嵌套对象键序
    let x = canonical_json(&json!({ "z": [3, 1], "a": { "y": 2, "b": 1 } }));
    assert_eq!(x, r#"{"a":{"b":1,"y":2},"z":[3,1]}"#);
}

/// metadata 非 object 拒绝（SPEC §2.2 契约）。
#[tokio::test]
async fn metadata_non_object_rejected() {
    let store = seeded_store().await;
    let err = store
        .memory_write("事实", &[], &json!([1, 2, 3]))
        .await
        .expect_err("数组 metadata 必拒");
    assert!(
        format!("{err:?}").contains("JSON object"),
        "错误面须指向 metadata 类型约束"
    );
}

/// P20-c：直接 SQL 改行内容 → 列级失配检出；改快照根 → 重算不一致检出；
/// refresh 重建后恢复（可重算语义）。
#[tokio::test]
async fn p20c_tamper_detected_and_snapshot_rebuildable() {
    let store = seeded_store().await;
    for i in 0..5 {
        write_row(&store, i).await;
    }
    assert!(store.verify_memory().await.expect("verify").ok);

    // ① 篡改行内容（绕过 memory_write，直改持久层）
    sqlx::query("UPDATE memory SET content = '篡改后的事实' WHERE content LIKE 'fact-2:%'")
        .execute(store.pool_ref())
        .await
        .expect("篡改");
    let report = store.verify_memory().await.expect("verify");
    assert!(!report.ok, "P20-c: 行篡改必报不一致");
    assert_eq!(report.content_mismatches.len(), 1, "列级校验定位到被篡改行");

    // ② 重算可恢复（根为派生值）——content_hash 需同步修复后 verify 恢复 ok
    let chash = partisync_cas_content_digest("篡改后的事实");
    sqlx::query("UPDATE memory SET content_hash = ? WHERE content = '篡改后的事实'")
        .bind(&chash)
        .execute(store.pool_ref())
        .await
        .expect("修复列级哈希");
    store.refresh_memory_root().await.expect("refresh");
    assert!(
        store.verify_memory().await.expect("verify").ok,
        "重算重建后恢复"
    );

    // ③ 篡改快照根 → 重算不一致
    sqlx::query("UPDATE memory_root SET root = '0'")
        .execute(store.pool_ref())
        .await
        .expect("篡改根");
    let report = store.verify_memory().await.expect("verify");
    assert!(!report.ok, "P20-c: 快照根篡改必报不一致");
    assert!(report.content_mismatches.is_empty(), "此为根不一致，非列级");
}

/// 空库 verify：ok=true，重算根 = blake3("") hex（空树定义，P20-a 边界）。
#[tokio::test]
async fn empty_store_verify_ok() {
    let store = seeded_store().await;
    let report = store.verify_memory().await.expect("verify");
    assert!(report.ok);
    assert_eq!(report.memory_count, 0);
    assert!(report.snapshot_root.is_none(), "首写前无快照");
    assert_eq!(
        report.recomputed_root,
        blake3::hash(b"").to_hex().to_string(),
        "空树根 = blake3(\"\")"
    );
}

/// P20-b：跨 n 边界全叶扫描（n=1..70）——MTH k 分裂在 2 的幂边界与奇右界
/// 处最易出错（n=40/idx=32 类缺陷回归面）。
#[test]
fn p20b_exhaustive_across_tree_sizes() {
    for n in 1..=70usize {
        let rows: Vec<partisync_graph::MemoryRow> = (0..n)
            .map(|i| partisync_graph::MemoryRow {
                memory_id: memory_identity(&format!("f{i}"), "[]", "{}"),
                content: format!("f{i}"),
                content_hash: partisync_cas_content_digest(&format!("f{i}")),
                tags: "[]".into(),
                metadata: "{}".into(),
                created_ns: i as i64,
                origin_device: "dev".into(),
                hlc: None,
                deleted: 0,
            })
            .collect();
        let hashes: Vec<[u8; 32]> = rows
            .iter()
            .map(partisync_graph::memory::leaf_hash)
            .collect();
        let root = tree_root(&hashes);
        for i in 0..n {
            let path = audit_path(&hashes, i).expect("路径存在");
            assert!(
                verify_inclusion(&hashes[i], &path, i, n, &root),
                "n={n} idx={i} 包含验证必成立"
            );
        }
        // 非本集叶必败
        let outsider = [0u8; 32];
        assert!(!verify_inclusion(
            &outsider,
            &audit_path(&hashes, 0).unwrap(),
            0,
            n,
            &root
        ));
    }
}

/// 根重算性能记录（SPEC §3：10⁴ 叶全量重算 ≤1s，本机记值入任务卡，
/// 不作 CI 门——沿 WP01 bench 判例）。直插持久层绕过 memory_write
/// （避免逐笔 refresh 的 O(n²)），测的是 refresh_memory_root 全量重算面。
#[tokio::test]
async fn perf_root_recompute_10k_leaves() {
    let store = seeded_store().await;
    let mut tx = store.pool_ref().begin().await.expect("开事务");
    for i in 0..10_000i64 {
        let content = format!("perf-{i}: 根重算性能探针行");
        let tags = r#"["perf"]"#;
        let metadata = "{}";
        let id = memory_identity(&content, tags, metadata);
        sqlx::query(
            "INSERT INTO memory
                (memory_id, content, content_hash, tags, metadata, created_ns, origin_device, hlc, deleted)
             VALUES (?, ?, ?, ?, ?, ?, 'dev-a', NULL, 0)",
        )
        .bind(&id)
        .bind(&content)
        .bind(partisync_cas_content_digest(&content))
        .bind(tags)
        .bind(metadata)
        .bind(1_700_000_000_000_000_000i64 + i)
        .execute(&mut *tx)
        .await
        .expect("直插 perf 行");
    }
    tx.commit().await.expect("提交事务");

    let t0 = std::time::Instant::now();
    let snap = store.refresh_memory_root().await.expect("重算");
    let ms = t0.elapsed().as_millis();
    assert_eq!(snap.memory_count, 10_000);
    assert!(ms <= 1000, "10⁴ 叶根重算 {ms}ms 超 1s 口径");
    eprintln!("10⁴ 叶 refresh_memory_root 全量重算: {ms} ms");
}

/// blake3(content) hex（与 store 列级哈希同源口径）。
fn partisync_cas_content_digest(content: &str) -> String {
    blake3::hash(content.as_bytes()).to_hex().to_string()
}
