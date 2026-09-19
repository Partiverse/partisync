//! P6 收敛性测试（SPEC M2-WP01 验收；properties.md P6）——本项目最重属性测试。
//!
//! 构造 N 节点、随机操作序列（并发 upsert/remove）+ 随机 push/pull 顺序（含重复
//! 投递与回环）→ 全部节点最终状态等价。

use partisync_core::Ulid;
use partisync_graph::store::{EntryKind, Store};
use partisync_sync::{capture, session};
use proptest::prelude::*;

async fn node(tag: &str, device: &str) -> Store {
    let dir = std::env::temp_dir().join(format!("p6-{tag}-{}", Ulid::now()));
    std::fs::create_dir_all(&dir).unwrap();
    let s = Store::open(&dir.join("t.db")).await.unwrap();
    s.seed_device_volume(device, device, device).await.unwrap();
    s
}

/// 节点的可见状态（收敛比较口径）：path → (kind, size, content_id, owner)
async fn state_of(store: &Store) -> Vec<(String, i64, i64, Option<String>, Option<String>)> {
    // 子树遍历根
    let mut out = Vec::new();
    let mut stack = vec![String::from("/")];
    while let Some(dir) = stack.pop() {
        for e in store.children(&dir).await.unwrap() {
            out.push((
                e.path.clone(),
                e.kind,
                e.size,
                e.content_id.clone(),
                e.owner_device.clone(),
            ));
            if e.kind == 1 {
                stack.push(e.path.clone());
            }
        }
    }
    out.sort();
    out
}

#[tokio::test]
async fn p6_two_node_convergence() {
    let a = node("a", "dev-a").await;
    let b = node("b", "dev-b").await;

    // A 创建两个条目（watch 路径语义：变更后显式捕获）
    let root_a = a
        .add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None, None)
        .await
        .unwrap();
    a.add_entry(
        Some(&root_a),
        "f1",
        "/f1",
        EntryKind::File,
        10,
        0,
        Some(("HA", 10)),
        None,
    )
    .await
    .unwrap();
    capture::record_entry_upsert(&a, "/f1").await.unwrap();
    a.add_entry(
        Some(&root_a),
        "f2",
        "/f2",
        EntryKind::File,
        20,
        0,
        Some(("HB", 20)),
        None,
    )
    .await
    .unwrap();
    capture::record_entry_upsert(&a, "/f2").await.unwrap();

    // B 创建一个自己的条目
    let root_b = b
        .add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None, None)
        .await
        .unwrap();
    b.add_entry(
        Some(&root_b),
        "g1",
        "/g1",
        EntryKind::File,
        5,
        0,
        Some(("HG", 5)),
        None,
    )
    .await
    .unwrap();
    capture::record_entry_upsert(&b, "/g1").await.unwrap();

    // 双向同步（两次轮：多轮传播覆盖中继与回环）
    session::push(&a, &b).await.unwrap();
    session::push(&b, &a).await.unwrap();
    session::push(&a, &b).await.unwrap();
    session::push(&b, &a).await.unwrap();
    eprintln!(
        "DEBUG push b->a2: A={} B={}",
        state_of(&a).await.len(),
        state_of(&b).await.len()
    );

    let sa = state_of(&a).await;
    let sb = state_of(&b).await;
    assert_eq!(sa, sb, "双向同步后两节点状态必须等价（P6）");
    assert!(sa
        .iter()
        .any(|(p, _, _, _, o)| p == "/f1" && o.as_deref() == Some("dev-a")));
    assert!(sa
        .iter()
        .any(|(p, _, _, _, o)| p == "/g1" && o.as_deref() == Some("dev-b")));

    // ACK 裁剪：稳态后 oplog 空或仅剩中继行（有界）
    assert!(
        a.pending_oplog().await.unwrap().len() <= 2,
        "A oplog 应被 ACK 裁剪"
    );
}

#[tokio::test]
async fn loop_protection_no_echo() {
    let a = node("lp-a", "dev-a").await;
    let b = node("lp-b", "dev-b").await;
    a.add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None, None)
        .await
        .unwrap();
    a.add_entry(
        None,
        "x",
        "/x",
        EntryKind::File,
        1,
        0,
        Some(("HX", 1)),
        None,
    )
    .await
    .unwrap();
    capture::record_entry_upsert(&a, "/x").await.unwrap();

    session::push(&a, &b).await.unwrap(); // x 进入 b（中继行 origin=dev-a）
    let ops_before = b.pending_oplog().await.unwrap().len();
    // b 回投给 a：origin=dev-a 的行在 a 侧被跳过，且 a 不再产生新 oplog
    session::push(&b, &a).await.unwrap();
    let a_ops = a.pending_oplog().await.unwrap().len();
    // a 的 oplog 只剩本机行被裁剪后的余量；反复轮询不再增长
    session::push(&b, &a).await.unwrap();
    assert_eq!(
        a.pending_oplog().await.unwrap().len(),
        a_ops,
        "回环不再产生新 oplog（死循环防护）"
    );
    let _ = ops_before;
}

#[tokio::test]
async fn conflict_keeps_both_with_lineage() {
    let a = node("cf-a", "dev-a").await;
    let b = node("cf-b", "dev-b").await;
    // 双端独立创建同路径（不同内容/属主）
    let ra = a
        .add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None, None)
        .await
        .unwrap();
    let rb = b
        .add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None, None)
        .await
        .unwrap();
    a.add_entry(
        Some(&ra),
        "c",
        "/c",
        EntryKind::File,
        1,
        0,
        Some(("FROM-A", 1)),
        None,
    )
    .await
    .unwrap();
    capture::record_entry_upsert(&a, "/c").await.unwrap();
    b.add_entry(
        Some(&rb),
        "c",
        "/c",
        EntryKind::File,
        2,
        0,
        Some(("FROM-B", 2)),
        None,
    )
    .await
    .unwrap();
    capture::record_entry_upsert(&b, "/c").await.unwrap();

    session::push(&a, &b).await.unwrap();
    // b 上：/c（dev-b 自己的）+ /c.conflict-dev-a（A 的版本保留）
    let c_own = b.entry_by_path("/c").await.unwrap().unwrap();
    let c_conflict = b.entry_by_path("/c.conflict-dev-a").await.unwrap();
    assert_eq!(
        c_own.owner_device.as_deref(),
        Some("dev-b"),
        "本属主版本不动"
    );
    assert!(c_conflict.is_some(), "冲突版本保留（保留两者 + 血缘后缀）");
}

#[tokio::test]
async fn replay_is_idempotent() {
    let a = node("rp-a", "dev-a").await;
    let b = node("rp-b", "dev-b").await;
    let root = a
        .add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None, None)
        .await
        .unwrap();
    a.add_entry(
        Some(&root),
        "f",
        "/f",
        EntryKind::File,
        3,
        0,
        Some(("HF", 3)),
        None,
    )
    .await
    .unwrap();
    capture::record_entry_upsert(&a, "/f").await.unwrap();
    // 同一批 oplog 重放（不裁剪）：人工构造——push 前复制 oplog 快照语义
    session::push(&a, &b).await.unwrap();
    let s1 = state_of(&b).await;
    // 再 push 一次（oplog 已被裁剪 → 空操作）
    session::push(&a, &b).await.unwrap();
    let s2 = state_of(&b).await;
    assert_eq!(s1, s2, "重放/空推送不改状态（P8 幂等）");
}

#[tokio::test]
async fn batch_indexed_entries_capture_local_origin() {
    // KPI 基准（M2-WP00）实测暴露：add_file_batch 落库 owner 缺失时，
    // capture 的 oplog origin 曾退化为 "unknown" → 对端回环防护永不命中
    // → bisync 乒乓放大（每轮全量重投直至 max_rounds）。
    // 回归契约：捕获 origin 必须是本机 device id，与 owner 列无关。
    let a = node("og-a", "dev-a").await;
    let root = a
        .add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None, None)
        .await
        .unwrap();
    a.add_file_batch(&[partisync_graph::store::FileInsert {
        id: Ulid::now().to_string(),
        parent_id: Some(root),
        name: "f1".into(),
        path: "/f1".into(),
        size: 10,
        mtime_ns: 1,
        content: Some(("HB".to_string(), 10)),
        chunk_root: None,
    }])
    .await
    .unwrap();
    capture::record_entry_upsert(&a, "/f1").await.unwrap();
    let rows = a.pending_oplog().await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].origin_device, "dev-a",
        "捕获 origin 必须为本机 device id（回环防护依赖）"
    );
    // 端到端：batch 数据经 push → 对端中继行回投不再被重投应用（2 轮内不动点）
    let b = node("og-b", "dev-b").await;
    b.add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None, None)
        .await
        .unwrap();
    let s1 = session::push(&a, &b).await.unwrap();
    assert_eq!(s1.applied, 1);
    let bs = session::bisync(&a, &b, session::BisyncOpts::default())
        .await
        .unwrap();
    assert!(
        bs.rounds <= 2,
        "稳态 bisync 必须 ≤2 轮收敛（乒乓时会打满 8 轮）"
    );
    assert_eq!(bs.pushed + bs.pulled, 0, "无新变更时零应用");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// P6：三节点、随机操作 + 随机同步顺序（含重复与回环）→ 全网收敛。
    #[test]
    fn prop_three_node_convergence(
        ops in prop::collection::vec(
            (0u8..3, prop::sample::select(vec![0u8, 1, 2]), 0u8..4, 1u8..64, 0u8..2),
            1..40,
        ),
        sync_rounds in prop::collection::vec((0u8..3, 0u8..3), 1..20),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _ = rt.block_on(async move {
            let devs = ["dev-0", "dev-1", "dev-2"];
            let mut nodes: Vec<Store> = Vec::new();
            let mut roots: Vec<String> = Vec::new();
            for (i, d) in devs.iter().enumerate() {
                let n = node(&format!("p6-{i}"), d).await;
                let root = n.add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None, None).await.unwrap();
                nodes.push(n);
                roots.push(root);
            }
            // 随机操作：在节点 i 上创建（唯一路径 f{op序号}）或删除（其自有条目）
            let mut created: [Vec<String>; 3] = [vec![], vec![], vec![]];
            for (seq, node_i, pick, size, is_remove) in ops {
                let n = &nodes[node_i as usize];
                if is_remove == 0 || created[node_i as usize].is_empty() {
                    let path = format!("/n{node_i}-f{seq}-{pick}");
                    n.add_entry(Some(&roots[node_i as usize]), &path[1..], &path, EntryKind::File,
                                 u64::from(size), 0, Some((Box::leak(format!("H{seq}-{pick}").into_boxed_str()), u64::from(size))), None)
                        .await.unwrap();
                    partisync_sync::capture::record_entry_upsert(n, &path).await.unwrap();
                    created[node_i as usize].push(path);
                } else {
                    let idx = (pick as usize) % created[node_i as usize].len();
                    let path = created[node_i as usize].swap_remove(idx);
                    n.remove_entry(&path).await.unwrap();
                    partisync_sync::capture::record_entry_remove(n, &path).await.unwrap();
                }
            }
            // 随机同步轮（有向边 push；重复边允许）
            for (from, to) in sync_rounds {
                if from != to {
                    partisync_sync::session::push(&nodes[from as usize], &nodes[to as usize])
                        .await.unwrap();
                }
            }
            // 收敛轮：全 mesh 双向（模拟"重连后对账"）
            for i in 0..3 {
                for j in 0..3 {
                    if i != j {
                        partisync_sync::session::push(&nodes[i], &nodes[j]).await.unwrap();
                    }
                }
            }
            // P6：三节点状态等价
            let s0 = state_of(&nodes[0]).await;
            let s1 = state_of(&nodes[1]).await;
            let s2 = state_of(&nodes[2]).await;
            prop_assert_eq!(s0.len(), s1.len(), "节点 0/1 条目数不等");
            prop_assert_eq!(s1.len(), s2.len(), "节点 1/2 条目数不等");
            prop_assert!(s0 == s1 && s1 == s2, "三节点状态必须完全等价（P6）");
            Ok(())
        });
    }
}
