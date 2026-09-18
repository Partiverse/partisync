//! 事件管线集成测试（SPEC M0-WP04 验收：幂等应用 / 级联删除 / 崩溃重放）。

use partisync_core::Ulid;
use partisync_graph::indexer::index_path;
use partisync_graph::journal::{apply_pending, record, EventKind};
use partisync_graph::store::{EntryKind, Store};

async fn file_store() -> (Store, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("journal-{}", Ulid::now()));
    std::fs::create_dir_all(&dir).unwrap();
    let store = Store::open(&dir.join("t.db")).await.unwrap();
    (store, dir)
}

fn make_tree(root: &std::path::Path) {
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("a.txt"), b"hello").unwrap();
    std::fs::write(root.join("sub").join("b.txt"), b"world").unwrap();
}

#[tokio::test]
async fn apply_lifecycle_and_replay_idempotent() {
    let (store, db_dir) = file_store().await;
    let fs = std::env::temp_dir().join(format!("journal-fs-{}", Ulid::now()));
    make_tree(&fs);
    index_path(&store, None, &fs).await.unwrap();
    let st0 = store.stats().await.unwrap();

    // fs 变化：改 a.txt 内容；新增 new/new.txt；删除 sub/b.txt
    std::fs::write(fs.join("a.txt"), b"HELLO-LONGER").unwrap();
    std::fs::create_dir_all(fs.join("new")).unwrap();
    std::fs::write(fs.join("new").join("new.txt"), b"brand new").unwrap();
    std::fs::remove_file(fs.join("sub").join("b.txt")).unwrap();

    // 注入 journal 事件（模拟 watcher 去抖后的批次）
    record(&store, "/a.txt", EventKind::Modified).await.unwrap();
    record(&store, "/new/new.txt", EventKind::Created)
        .await
        .unwrap();
    record(&store, "/sub/b.txt", EventKind::Removed)
        .await
        .unwrap();

    let applied = apply_pending(&store, None, &fs).await.unwrap();
    assert_eq!(
        (applied.modified, applied.created, applied.removed),
        (1, 1, 1),
        "应用摘要与注入事件一致"
    );
    let st1 = store.stats().await.unwrap();
    assert_eq!(st1.files, st0.files, "-1 +1 净零");
    assert_eq!(st1.unique_contents, st0.unique_contents + 1, "-1 +1 净零");
    // 修改生效：a.txt 的 content 已更新（大小变化可查）
    let a = store.entry_by_path("/a.txt").await.unwrap().unwrap();
    assert_eq!(a.size, 12);

    // P8 重放：同一批事件再来一遍（幂等收敛）
    record(&store, "/a.txt", EventKind::Modified).await.unwrap();
    record(&store, "/new/new.txt", EventKind::Created)
        .await
        .unwrap();
    record(&store, "/sub/b.txt", EventKind::Removed)
        .await
        .unwrap();
    apply_pending(&store, None, &fs).await.unwrap();
    let st2 = store.stats().await.unwrap();
    assert_eq!(st2, st1, "重放不改图谱（幂等）");

    // journal 已消费
    assert_eq!(
        partisync_graph::journal::pending(&store)
            .await
            .unwrap()
            .len(),
        0
    );

    std::fs::remove_dir_all(&fs).ok();
    std::fs::remove_dir_all(&db_dir).ok();
}

#[tokio::test]
async fn remove_dir_cascades_and_content_cleanup() {
    let (store, db_dir) = file_store().await;
    // 手工建：/d 目录下 f1(内容X) 与根下 f2(内容X 共享) 、f3(内容Y 独占)
    let root = store
        .add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None, None)
        .await
        .unwrap();
    let d = store
        .add_entry(Some(&root), "d", "/d", EntryKind::Dir, 0, 0, None, None)
        .await
        .unwrap();
    store
        .add_entry(
            Some(&d),
            "f1",
            "/d/f1",
            EntryKind::File,
            10,
            0,
            Some(("X", 10)),
            None,
        )
        .await
        .unwrap();
    store
        .add_entry(
            Some(&root),
            "f2",
            "/f2",
            EntryKind::File,
            10,
            0,
            Some(("X", 10)),
            None,
        )
        .await
        .unwrap();
    store
        .add_entry(
            Some(&root),
            "f3",
            "/f3",
            EntryKind::File,
            5,
            0,
            Some(("Y", 5)),
            None,
        )
        .await
        .unwrap();

    // fs 侧同步删掉 /d（apply 的 removed 分支只读 journal）
    record(&store, "/d", EventKind::Removed).await.unwrap();
    apply_pending(&store, None, &std::env::temp_dir())
        .await
        .unwrap();

    assert!(store.entry_by_path("/d").await.unwrap().is_none());
    assert!(
        store.entry_by_path("/d/f1").await.unwrap().is_none(),
        "子树级联"
    );
    // 闭包行随删：/d 的祖先链不存在 ⇒ 子树查询为空
    assert!(store.subtree("/d").await.unwrap().is_empty());
    // X 仍被 /f2 引用 → 保留（Y 不在子树，本就不该动）
    assert!(store.entry_by_path("/f2").await.unwrap().is_some());
    let st = store.stats().await.unwrap();
    assert_eq!(
        st.unique_contents, 2,
        "X（被 /f2 引用）与 Y（子树外）均保留"
    );

    // 删除 /f2 ⇒ X 成孤儿 → content 行清除（孤儿清理路径）
    record(&store, "/f2", EventKind::Removed).await.unwrap();
    apply_pending(&store, None, &std::env::temp_dir())
        .await
        .unwrap();
    let st = store.stats().await.unwrap();
    assert_eq!(st.unique_contents, 1, "X 成孤儿后清除，仅剩 Y");
    assert_eq!(st.duplicate_groups, 0);

    std::fs::remove_dir_all(&db_dir).ok();
}

#[tokio::test]
async fn crash_replay_converges() {
    // 崩溃口径（SPEC：apply 中途崩溃后重开库重放 → 最终一致）
    let (store, db_dir) = file_store().await;
    let fs = std::env::temp_dir().join(format!("journal-crash-{}", Ulid::now()));
    make_tree(&fs);
    index_path(&store, None, &fs).await.unwrap();
    std::fs::write(fs.join("c.txt"), b"crash test").unwrap();
    record(&store, "/c.txt", EventKind::Created).await.unwrap();
    // 模拟崩溃：不 apply，直接释放连接池后重开
    drop(store);
    let store2 = Store::open(&db_dir.join("t.db")).await.unwrap();
    assert_eq!(
        partisync_graph::journal::pending(&store2)
            .await
            .unwrap()
            .len(),
        1,
        "崩溃前未应用的事件仍在 journal"
    );
    apply_pending(&store2, None, &fs).await.unwrap();
    let c = store2.entry_by_path("/c.txt").await.unwrap();
    assert!(c.is_some(), "重放后收敛");
    std::fs::remove_dir_all(&fs).ok();
    std::fs::remove_dir_all(&db_dir).ok();
}

#[tokio::test]
async fn upsert_preserves_id_and_refreshes() {
    let (store, db_dir) = file_store().await;
    let id = store
        .add_entry(
            None,
            "f",
            "/f",
            EntryKind::File,
            1,
            100,
            Some(("OLD", 1)),
            None,
        )
        .await
        .unwrap();
    let id2 = store
        .add_entry(
            None,
            "f",
            "/f",
            EntryKind::File,
            9,
            200,
            Some(("NEW", 9)),
            None,
        )
        .await
        .unwrap();
    assert_eq!(id, id2, "upsert 保留 id");
    let e = store.entry_by_path("/f").await.unwrap().unwrap();
    assert_eq!(
        (e.size, e.content_id.as_deref()),
        (9, Some("NEW")),
        "新鲜度刷新"
    );
    std::fs::remove_dir_all(&db_dir).ok();
}
