//! M2-WP02 验收测试（SPEC 验收标准）：P11 冲突血缘模型、Tag 共享域收敛、
//! 墓碑防复活、LWW、max-delete 安全阈、bisync 定衬。

use partisync_core::Ulid;
use partisync_graph::store::{EntryKind, Store};
use partisync_sync::{capture, session};
use proptest::prelude::*;

async fn node(tag: &str, device: &str) -> Store {
    let dir = std::env::temp_dir().join(format!("p11-{tag}-{}", Ulid::now()));
    std::fs::create_dir_all(&dir).unwrap();
    let s = Store::open(&dir.join("t.db")).await.unwrap();
    s.seed_device_volume(device, device, device).await.unwrap();
    s
}

/// 在节点上创建文件条目并捕获（watch 路径语义）。
async fn put(store: &Store, path: &str, hash: &str, size: u64) {
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
}

/// 节点的共享域可观察状态：(tag id, name, color) 全集 + 每条目标签名集。
async fn tag_state_of(
    store: &Store,
    paths: &[String],
) -> (Vec<(String, String, Option<String>)>, Vec<Vec<String>>) {
    let mut tags: Vec<_> = store
        .list_tags()
        .await
        .unwrap()
        .into_iter()
        .map(|t| (t.id, t.name, t.color))
        .collect();
    tags.sort();
    let mut per_path = Vec::new();
    for p in paths {
        let mut names: Vec<String> = store
            .tags_of_entry(p)
            .await
            .unwrap()
            .into_iter()
            .map(|t| t.name)
            .collect();
        names.sort();
        per_path.push(names);
    }
    (tags, per_path)
}

// ---------- P11：冲突血缘模型（SPEC 验收第 1 条） ----------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// 两节点对同一路径随机并发双写 → bisync 后两版本皆可寻址、内容与各自
    /// 所写一致、血缘记录齐全（P11）。
    #[test]
    fn prop_p11_conflict_lineage(
        a_writes in prop::collection::vec((1u8..64, 1u64..999), 1..4),
        b_writes in prop::collection::vec((1u8..64, 1u64..999), 1..4),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _ = rt.block_on(async move {
            let a = node("m-a", "dev-a").await;
            let b = node("m-b", "dev-b").await;
            // 双端各自独立创建 /x 并随机覆写若干次（内容哈希唯一化）
            let mut last_a = String::new();
            for (i, (h, s)) in a_writes.iter().enumerate() {
                last_a = format!("A{i}-{h}");
                put(&a, "/x", &last_a, *s).await;
            }
            let mut last_b = String::new();
            for (i, (h, s)) in b_writes.iter().enumerate() {
                last_b = format!("B{i}-{h}");
                put(&b, "/x", &last_b, *s).await;
            }
            let stats = session::bisync(&a, &b, session::BisyncOpts::default()).await.unwrap();
            prop_assert!(stats.rounds <= 8);

            for (me, _other, dev_me, dev_other, last_me, last_other) in
                [(&a, &b, "dev-a", "dev-b", &last_a, &last_b), (&b, &a, "dev-b", "dev-a", &last_b, &last_a)]
            {
                // 本位版本：属主不变、内容为最后一次本端写入
                let base = me.entry_by_path("/x").await.unwrap().unwrap();
                prop_assert_eq!(base.owner_device.as_deref(), Some(dev_me));
                prop_assert_eq!(base.content_id.as_deref(), Some(last_me.as_str()));
                // 来方版本可寻址且内容 == 对端最后写入（宁可多一份副本）
                let conflicts = me.list_conflicts(100).await.unwrap();
                prop_assert!(!conflicts.is_empty(), "{dev_me} 应有血缘记录");
                let mut found_other = false;
                for c in &conflicts {
                    prop_assert_eq!(c.base_path.as_str(), "/x");
                    prop_assert_eq!(c.origin_device.as_str(), dev_other);
                    if let Some(row) = me.entry_by_path(&c.incoming_path).await.unwrap() {
                        if row.content_id.as_deref() == Some(last_other.as_str()) {
                            found_other = true;
                        }
                    }
                }
                prop_assert!(found_other, "{dev_me} 上应有 {dev_other} 最终版本的可寻址副本");
            }
            Ok(())
        });
    }
}

// ---------- 确定性验收 ----------

#[tokio::test]
async fn conflict_suffix_never_overwrites_earlier_copy() {
    let a = node("sf-a", "dev-a").await;
    let b = node("sf-b", "dev-b").await;
    put(&a, "/x", "HA1", 1).await;
    put(&b, "/x", "HB", 2).await;
    session::push(&a, &b).await.unwrap(); // b: /x.conflict-dev-a = HA1
    put(&a, "/x", "HA2", 3).await;
    session::push(&a, &b).await.unwrap(); // 后缀被占 → -2
    let x = b.entry_by_path("/x").await.unwrap().unwrap();
    let c1 = b.entry_by_path("/x.conflict-dev-a").await.unwrap().unwrap();
    let c2 = b
        .entry_by_path("/x.conflict-dev-a-2")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(x.content_id.as_deref(), Some("HB"), "本位版本不动");
    assert_eq!(c1.content_id.as_deref(), Some("HA1"), "首版冲突副本不丢");
    assert_eq!(c2.content_id.as_deref(), Some("HA2"), "次版落新后缀");
}

#[tokio::test]
async fn tombstone_resists_late_link_and_late_upsert() {
    let a = node("tb-a", "dev-a").await;
    let b = node("tb-b", "dev-b").await;
    put(&a, "/f1", "HF", 1).await;
    let t = a.add_tag("work", Some("red")).await.unwrap();
    capture::record_tag_upsert(&a, &t).await.unwrap();
    a.tag_entry(&t, "/f1").await.unwrap();
    capture::record_tag_link(&a, &t, "/f1").await.unwrap();
    session::push(&a, &b).await.unwrap();
    assert_eq!(b.tags_of_entry("/f1").await.unwrap().len(), 1);

    // b 摘标签（phys 更晚 ⇒ HLC 更大——sleep 拉开毫秒位）
    tokio::time::sleep(std::time::Duration::from_millis(3)).await;
    b.untag_entry(&t, "/f1").await.unwrap();
    capture::record_tag_unlink(&b, &t, "/f1").await.unwrap();
    session::push(&b, &a).await.unwrap(); // 墓碑传到 a
    session::push(&a, &b).await.unwrap(); // a 的晚到 link 行（旧 HLC）被拒
    assert!(
        a.tags_of_entry("/f1").await.unwrap().is_empty(),
        "晚到 link 不得复活已 unlink 的链接"
    );
    assert!(b.tags_of_entry("/f1").await.unwrap().is_empty());

    // 墓碑 tag：b 删除后，a 的旧 upsert 行不得复活
    tokio::time::sleep(std::time::Duration::from_millis(3)).await;
    b.delete_tag(&t).await.unwrap();
    capture::record_tag_remove(&b, &t).await.unwrap();
    session::push(&b, &a).await.unwrap();
    session::push(&a, &b).await.unwrap();
    assert!(
        a.list_tags().await.unwrap().is_empty(),
        "墓碑后旧 upsert 不复活"
    );
    assert!(b.list_tags().await.unwrap().is_empty());
}

#[tokio::test]
async fn lww_concurrent_rename_converges() {
    let a = node("lw-a", "dev-a").await;
    let b = node("lw-b", "dev-b").await;
    let t = a.add_tag("orig", None).await.unwrap();
    capture::record_tag_upsert(&a, &t).await.unwrap();
    session::push(&a, &b).await.unwrap();
    a.update_tag(&t, "from-a", Some("blue")).await.unwrap();
    capture::record_tag_upsert(&a, &t).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(3)).await;
    b.update_tag(&t, "from-b", None).await.unwrap();
    capture::record_tag_upsert(&b, &t).await.unwrap();
    session::bisync(&a, &b, session::BisyncOpts::default())
        .await
        .unwrap();
    for s in [&a, &b] {
        let row = s.tag_by_id(&t).await.unwrap().unwrap();
        assert_eq!(row.name, "from-b", "HLC 大者胜（phys 更晚的写入）");
    }
}

#[tokio::test]
async fn max_delete_refuses_batch_before_applying() {
    let a = node("md-a", "dev-a").await;
    let b = node("md-b", "dev-b").await;
    for p in ["/d1", "/d2", "/d3"] {
        put(&a, p, "HX", 1).await;
    }
    session::push(&a, &b).await.unwrap();
    for p in ["/d1", "/d2", "/d3"] {
        a.remove_entry(p).await.unwrap();
        capture::record_entry_remove(&a, p).await.unwrap();
    }
    let b_paths_before = b.entry_by_path("/d3").await.unwrap().is_some();
    let b_oplog_before = b.pending_oplog().await.unwrap().len();

    let err = session::push_opts(
        &a,
        &b,
        session::PushOpts {
            max_delete: Some(2),
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.to_string().contains("max-delete"),
        "应报安全阈错误: {err}"
    );
    assert_eq!(
        b.entry_by_path("/d3").await.unwrap().is_some(),
        b_paths_before,
        "拒绝后对端状态不变（先拒后用）"
    );
    assert_eq!(
        b.pending_oplog().await.unwrap().len(),
        b_oplog_before,
        "拒绝后对端 oplog 不变"
    );

    let ok = session::push_opts(
        &a,
        &b,
        session::PushOpts {
            max_delete: Some(3),
        },
    )
    .await
    .unwrap();
    assert_eq!(ok.applied, 3);
    assert!(
        b.entry_by_path("/d3").await.unwrap().is_none(),
        "阈值内正常删除"
    );
}

#[tokio::test]
async fn bisync_reaches_fixed_point_and_quiesces() {
    let a = node("bq-a", "dev-a").await;
    let b = node("bq-b", "dev-b").await;
    put(&a, "/fa", "HA", 1).await;
    put(&b, "/fb", "HB", 2).await;
    let t = a.add_tag("shared", None).await.unwrap();
    capture::record_tag_upsert(&a, &t).await.unwrap();
    b.tag_entry(&t, "/fb").await.unwrap();
    capture::record_tag_link(&b, &t, "/fb").await.unwrap();

    let s1 = session::bisync(&a, &b, session::BisyncOpts::default())
        .await
        .unwrap();
    assert!(s1.rounds <= 8, "定衬轮数有界");
    assert!(a.entry_by_path("/fb").await.unwrap().is_some());
    assert!(b.entry_by_path("/fa").await.unwrap().is_some());
    assert_eq!(
        b.tags_of_entry("/fb").await.unwrap().len(),
        1,
        "跨端链接收敛"
    );

    let s2 = session::bisync(&a, &b, session::BisyncOpts::default())
        .await
        .unwrap();
    assert_eq!(
        (s2.pushed, s2.pulled, s2.conflicts),
        (0, 0, 0),
        "稳态后 bisync 零应用（不动点）"
    );
}

// ---------- Tag 共享域收敛（P6 扩展到共享域） ----------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// 三节点随机 tag/link/unlink/update + 随机同步 → 全网共享域可观察状态等价。
    #[test]
    fn prop_tag_shared_domain_convergence(
        ops in prop::collection::vec((1u8..64, 0u8..3, 1u8..16), 1..14),
        sync_rounds in prop::collection::vec((0u8..3, 0u8..3), 1..10),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _ = rt.block_on(async move {
            let devs = ["dev-0", "dev-1", "dev-2"];
            let mut nodes = Vec::new();
            let mut files_per_node: [Vec<String>; 3] = [vec![], vec![], vec![]];
            let mut tags_per_node: [Vec<(String, bool)>; 3] = [vec![], vec![], vec![]]; // (id, alive)
            for (i, d) in devs.iter().enumerate() {
                let n = node(&format!("tg-{i}"), d).await;
                for j in 0u64..3 {
                    let p = format!("/n{i}-f{j}");
                    put(&n, &p, &format!("H{i}-{j}"), j + 1).await;
                    files_per_node[i].push(p);
                }
                nodes.push(n);
            }
            for (seq, ni, pick) in ops {
                let n = &nodes[ni as usize];
                let i = ni as usize;
                match pick % 4 {
                    // 新建 tag
                    0 => {
                        let id = n.add_tag(&format!("t{seq}"), None).await.unwrap();
                        capture::record_tag_upsert(n, &id).await.unwrap();
                        tags_per_node[i].push((id, true));
                    }
                    // 更新（本节点尚存活的 tag）
                    1 if tags_per_node[i].iter().any(|(_, alive)| *alive) => {
                        let idx = (pick as usize)
                            % tags_per_node[i].iter().filter(|(_, a)| *a).count();
                        let (id, _) = tags_per_node[i].iter().filter(|(_, a)| *a).nth(idx).unwrap();
                        let id = id.clone();
                        n.update_tag(&id, &format!("t{seq}-u"), Some("c")).await.unwrap();
                        capture::record_tag_upsert(n, &id).await.unwrap();
                    }
                    // 删除
                    2 if tags_per_node[i].iter().any(|(_, alive)| *alive) => {
                        let idx = (pick as usize)
                            % tags_per_node[i].iter().filter(|(_, a)| *a).count();
                        let (id, _) = tags_per_node[i].iter().filter(|(_, a)| *a).nth(idx).unwrap();
                        let id = id.clone();
                        n.delete_tag(&id).await.unwrap();
                        capture::record_tag_remove(n, &id).await.unwrap();
                        tags_per_node[i] = tags_per_node[i]
                            .iter()
                            .map(|(x, alive)| if x == &id { (x.clone(), false) } else { (x.clone(), *alive) })
                            .collect();
                    }
                    // link / unlink
                    _ => {
                        let alive: Vec<(String, bool)> = tags_per_node[i]
                            .iter()
                            .filter(|(_, a)| *a)
                            .cloned()
                            .collect();
                        if alive.is_empty() { continue; }
                        let (id, _) = &alive[(pick as usize) % alive.len()];
                        let id = id.clone();
                        let p = files_per_node[i][(pick as usize) % files_per_node[i].len()].clone();
                        if pick % 2 == 0 {
                            n.tag_entry(&id, &p).await.unwrap();
                            capture::record_tag_link(n, &id, &p).await.unwrap();
                        } else {
                            n.untag_entry(&id, &p).await.unwrap();
                            capture::record_tag_unlink(n, &id, &p).await.unwrap();
                        }
                    }
                }
            }
            // 随机同步轮 + 全 mesh 双向收敛
            for (from, to) in sync_rounds {
                if from != to {
                    session::push(&nodes[from as usize], &nodes[to as usize]).await.unwrap();
                }
            }
            for i in 0..3 {
                for j in 0..3 {
                    if i != j {
                        session::push(&nodes[i], &nodes[j]).await.unwrap();
                    }
                }
            }
            // P6：三节点共享域状态等价
            let all_paths: Vec<String> = files_per_node.iter().flatten().cloned().collect();
            let s0 = tag_state_of(&nodes[0], &all_paths).await;
            let s1 = tag_state_of(&nodes[1], &all_paths).await;
            let s2 = tag_state_of(&nodes[2], &all_paths).await;
            prop_assert!(s0 == s1 && s1 == s2, "三节点 tag 状态必须等价：{s0:?} vs {s1:?} vs {s2:?}");
            Ok(())
        });
    }
}
