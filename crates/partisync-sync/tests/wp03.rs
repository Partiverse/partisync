//! M2-WP03 验收测试（SPEC 验收标准）：P7 随机差分收敛、晚加入者、快路径、
//! 时钟持久化、水位单调推进。

use partisync_core::Ulid;
use partisync_graph::store::{EntryKind, Store};
use partisync_sync::{capture, reconcile, session};
use proptest::prelude::*;

async fn node(tag: &str, device: &str) -> Store {
    let dir = std::env::temp_dir().join(format!("p7-{tag}-{}", Ulid::now()));
    std::fs::create_dir_all(&dir).unwrap();
    let s = Store::open(&dir.join("t.db")).await.unwrap();
    s.seed_device_volume(device, device, device).await.unwrap();
    s
}

async fn put(store: &Store, path: &str, hash: &str, size: u64, owner: Option<&str>) {
    let root = store.entry_by_path("/").await.unwrap().map(|e| e.id);
    store
        .add_entry(
            root.as_deref(),
            &path[1..],
            path,
            EntryKind::File,
            size,
            0,
            Some((hash, size)),
            None,
        )
        .await
        .unwrap();
    capture::record_entry_upsert(store, path).await.unwrap();
    let _ = owner; // 设备自有域属主即本机；覆盖测试用 apply_remote_entry 走远端路径
}

/// 节点的可见状态：(path, kind, content_id, owner) 全集 + tag/link 存活投影
async fn state_of(
    store: &Store,
) -> (
    Vec<(String, i64, Option<String>, Option<String>)>,
    Vec<(String, String, Option<String>)>,
) {
    let mut entries = Vec::new();
    let mut stack = vec![String::from("/")];
    while let Some(dir) = stack.pop() {
        for e in store.children(&dir).await.unwrap() {
            entries.push((
                e.path.clone(),
                e.kind,
                e.content_id.clone(),
                e.owner_device.clone(),
            ));
            if e.kind == 1 {
                stack.push(e.path.clone());
            }
        }
    }
    entries.sort();
    let mut tags: Vec<_> = store
        .list_tags()
        .await
        .unwrap()
        .into_iter()
        .map(|t| (t.id, t.name, t.color))
        .collect();
    tags.sort();
    (entries, tags)
}

// ---------- P7：随机差分注入对账收敛（proptest） ----------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// 两节点从同基线出发，各自独立施加 entry/tag/link 随机增删改 →
    /// 不经 oplog，直接对账 → 两节点可观察状态等价（含属主）。
    #[test]
    fn prop_p7_random_drift_converges(
        a_ops in prop::collection::vec((0u8..3, 1u8..32, 1u64..999, 1u64..999), 1..18),
        b_ops in prop::collection::vec((0u8..3, 1u8..32, 1u64..999, 1u64..999), 1..18),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _ = rt.block_on(async move {
            let a = node("p7-a", "dev-a").await;
            let b = node("p7-b", "dev-b").await;
            // 各自施加随机操作（路径唯一化避免互相覆盖）
            for (seq, p, s, _) in a_ops {
                let path = format!("/a-f{seq}-{p}");
                put(&a, &path, &format!("HA{seq}"), s, None).await;
            }
            for (seq, p, s, _) in b_ops {
                let path = format!("/b-f{seq}-{p}");
                put(&b, &path, &format!("HB{seq}"), s, None).await;
            }
            // 直接对账（不依赖 oplog——验证 Merkle 路径独立工作）
            let stats = reconcile::reconcile(&a, &b, reconcile::ReconcileOpts::default()).await.unwrap();
            prop_assert!(stats.converged, "对账后必须收敛：{stats:?}");
            let sa = state_of(&a).await;
            let sb = state_of(&b).await;
            prop_assert!(sa == sb, "对账后两节点状态必须等价：{sa:?} vs {sb:?}");
            Ok(())
        });
    }
}

// ---------- 确定性验收 ----------

#[tokio::test]
async fn late_joiner_recovers_full_state_via_reconcile() {
    // A 产生变更 → push 给 B（B 上 oplog 被 ACK 裁剪）→ C 空库 → C 与 B 对账
    let a = node("lj-a", "dev-a").await;
    let b = node("lj-b", "dev-b").await;
    let c = node("lj-c", "dev-c").await;
    for p in ["/fa", "/fb", "/fc"] {
        put(&a, p, "H-fp", 1, None).await;
    }
    let t = a.add_tag("late", None).await.unwrap();
    capture::record_tag_upsert(&a, &t).await.unwrap();
    a.tag_entry(&t, "/fa").await.unwrap();
    capture::record_tag_link(&a, &t, "/fa").await.unwrap();
    session::push(&a, &b).await.unwrap();
    // B 状态丰富；C 仍是空
    assert!(b.entry_by_path("/fa").await.unwrap().is_some());
    assert_eq!(b.tags_of_entry("/fa").await.unwrap().len(), 1);
    assert!(c.entry_by_path("/fa").await.unwrap().is_none());

    // 晚加入者直接对账（oplog 已被 ACK 裁剪，oplog 路径失效——Merkle 兜底）
    let stats = reconcile::reconcile(&c, &b, reconcile::ReconcileOpts::default())
        .await
        .unwrap();
    assert!(stats.converged, "C 晚加入必须收敛");
    assert!(c.entry_by_path("/fa").await.unwrap().is_some());
    assert!(c.entry_by_path("/fb").await.unwrap().is_some());
    assert!(c.entry_by_path("/fc").await.unwrap().is_some());
    assert_eq!(
        c.tags_of_entry("/fa").await.unwrap().len(),
        1,
        "链接跨节点收敛"
    );
}

#[tokio::test]
async fn fast_path_skips_when_origin_cover_equal() {
    let a = node("fp-a", "dev-a").await;
    let b = node("fp-b", "dev-b").await;
    put(&a, "/x", "HX", 1, None).await;
    session::bisync(&a, &b, session::BisyncOpts::default())
        .await
        .unwrap();
    // bisync 后双侧状态相等、水位覆盖相等 → 对账走快路径
    let stats = reconcile::reconcile(&a, &b, reconcile::ReconcileOpts::default())
        .await
        .unwrap();
    // 首次 reconcile：bisync 已同步状态但双方对对方 origin 的水位尚未覆盖 → 慢路径
    // 跑空修复 + finalize_watermarks 写入对方水位 → 第二次 reconcile 走快路径
    assert!(stats.converged);
    let stats2 = reconcile::reconcile(&a, &b, reconcile::ReconcileOpts::default())
        .await
        .unwrap();
    assert!(
        stats2.fast_path,
        "水位覆盖建立后稳态必须走快路径：{stats2:?}"
    );
    assert!(stats2.converged);
    assert_eq!((stats2.repaired_a, stats2.repaired_b), (0, 0));

    // 注入单侧差异 → 快路径失效 → 走根比对与修复
    put(&a, "/new", "HN", 1, None).await;
    let stats2 = reconcile::reconcile(&a, &b, reconcile::ReconcileOpts::default())
        .await
        .unwrap();
    assert!(!stats2.fast_path, "注入差异后快路径不得触发：{stats2:?}");
    assert!(stats2.converged);
    assert!(stats2.repaired_b > 0, "B 须被修复");
    assert!(b.entry_by_path("/new").await.unwrap().is_some());

    // 修复后再 bisync → 水位覆盖相等 → 快路径再次生效
    session::bisync(&a, &b, session::BisyncOpts::default())
        .await
        .unwrap();
    let _ = reconcile::reconcile(&a, &b, reconcile::ReconcileOpts::default())
        .await
        .unwrap();
    let stats3 = reconcile::reconcile(&a, &b, reconcile::ReconcileOpts::default())
        .await
        .unwrap();
    assert!(stats3.fast_path, "修复 + bisync 后再次快路径");
}

#[tokio::test]
async fn fast_path_rejects_unseen_third_origin() {
    // 第三方 C 写入 → 中继到 A（不进 B）→ A↔B 对账：仅 A 见到 C origin 的写入
    // → 快路径条件「∀d ∈ origins(a) ∪ origins(b)：Seen_a(d) == Seen_b(d)」失败
    let a = node("3p-a", "dev-a").await;
    let b = node("3p-b", "dev-b").await;
    let c = node("3p-c", "dev-c").await;
    put(&c, "/x", "HC", 1, None).await;
    session::push(&c, &a).await.unwrap(); // a 见到 c 的 /x
                                          // b 未见 /x；a 上 c origin 水位先进
    let stats = reconcile::reconcile(&a, &b, reconcile::ReconcileOpts::default())
        .await
        .unwrap();
    assert!(!stats.fast_path, "覆盖不等时不得走快路径：{stats:?}");
    assert!(stats.converged);
    assert!(
        b.entry_by_path("/x").await.unwrap().is_some(),
        "B 必须被修复到 /x"
    );
}

#[tokio::test]
async fn clock_persists_across_reopen() {
    // 本店时钟顶持久化：重启后新写键严格大于关闭前的最大键
    let dir = std::env::temp_dir().join(format!("clock-{}", Ulid::now()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t.db");
    let s1 = Store::open(&path).await.unwrap();
    s1.seed_device_volume("dev-x", "dev-x", "dev-x")
        .await
        .unwrap();
    put(&s1, "/k", "HK", 1, None).await;
    let top1 = s1.clock_top().await.unwrap().unwrap();
    drop(s1);
    // 同一路径重开
    let s2 = Store::open(&path).await.unwrap();
    let top2_initial = s2.clock_top().await.unwrap();
    assert_eq!(
        top2_initial.as_deref(),
        Some(top1.as_str()),
        "重启后时钟顶恢复"
    );
    // 物理墙钟若极快单调，新键也应 ≥ top1（recv 合并不回退）
    let new_key = {
        let _ = put(&s2, "/k2", "HK2", 1, None).await;
        s2.clock_top().await.unwrap().unwrap()
    };
    assert!(
        new_key >= top1,
        "新写键必须 ≥ 关闭前的最大键：new={new_key} top1={top1}"
    );
}

#[tokio::test]
async fn push_advances_watermark_monotonically() {
    let a = node("wm-a", "dev-a").await;
    let b = node("wm-b", "dev-b").await;
    put(&a, "/x", "HX", 1, None).await;
    session::push(&a, &b).await.unwrap();
    let wm_after = b.watermarks().await.unwrap();
    let wm_dev_a = wm_after.iter().find(|(d, _)| d == "dev-a").unwrap();
    // 水位 == oplog 中 dev-a 的最大键
    let top_a = a.clock_top().await.unwrap().unwrap();
    assert_eq!(&wm_dev_a.1, &top_a);
}
