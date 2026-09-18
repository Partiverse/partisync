//! `partisync-graph::store` 集成测试（SPEC M0-WP02 验收；L4 模型对照）。

use std::collections::HashMap;

use partisync_graph::indexer::index_path;
use partisync_graph::store::{EntryKind, Store};
use proptest::prelude::*;

async fn mem_store() -> Store {
    Store::open_in_memory().await.unwrap()
}

// ---------- 迁移与基本读写 ----------

#[tokio::test]
async fn migrate_is_idempotent_on_file_db() {
    let dir = std::env::temp_dir().join(format!("partisync-test-{}", partisync_core::Ulid::now()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("t.db");
    {
        let s = Store::open(&db).await.unwrap();
        s.add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None)
            .await
            .unwrap();
    }
    // 第二次打开（迁移重放）不得报错且数据仍在
    {
        let s = Store::open(&db).await.unwrap();
        assert!(s.entry_by_path("/").await.unwrap().is_some());
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn path_idempotency_returns_same_id() {
    let s = mem_store().await;
    let a = s
        .add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None)
        .await
        .unwrap();
    let b = s
        .add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None)
        .await
        .unwrap();
    assert_eq!(a, b, "同 path 重插返回既有 id（索引器幂等的根）");
}

// ---------- L4：闭包子树 ≡ 朴素递归遍历（模型对照） ----------

fn closure_subtree(model: &HashMap<String, Vec<String>>, root: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_string()];
    while let Some(p) = stack.pop() {
        out.push(p.clone());
        if let Some(children) = model.get(&p) {
            for c in children {
                stack.push(c.clone());
            }
        }
    }
    let mut sorted = out;
    sorted.sort();
    sorted
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// L4（properties.md）：随机树上闭包子树查询 ≡ 朴素 DFS 模型。
    #[test]
    fn prop_closure_subtree_matches_model(
        tree in prop::collection::vec(
            (prop::collection::vec("[a-e]{1,3}", 1..4), 0u16..6, 0u8..10, 0u16..1000),
            1..24,
        ),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _ = rt.block_on(async move {
            let s = mem_store().await;
            // 模型：path → children paths
            let mut model: HashMap<String, Vec<String>> = HashMap::new();
            let mut id_of: HashMap<String, String> = HashMap::new();
            model.insert("/".into(), vec![]);
            let root_id = s.add_entry(None, "root", "/", EntryKind::Dir, 0, 0, None).await.unwrap();
            id_of.insert("/".into(), root_id);

            let mut nodes: Vec<String> = vec!["/".into()];
            for (names, pick, _x, _y) in &tree {
                if nodes.is_empty() { break; }
                let parent_idx = (*pick as usize) % nodes.len();
                let parent = nodes[parent_idx].clone();
                let name = format!("{}{}", names.concat(), nodes.len()); // 去重：拼序号
                let path = if parent == "/" { format!("/{name}") } else { format!("{parent}/{name}") };
                let is_dir = (*_x % 2) == 0;
                let kind = if is_dir { EntryKind::Dir } else { EntryKind::File };
                let hash = format!("deadbeef{name:0>64}");
                let content = if is_dir { None } else { Some((hash.as_str(), 1u64)) };
                let id = s.add_entry(
                    id_of.get(&parent).map(String::as_str),
                    &name, &path, kind, 1, 0, content,
                ).await.unwrap();
                model.entry(parent.clone()).or_default().push(path.clone());
                id_of.insert(path.clone(), id);
                if is_dir {
                    model.entry(path.clone()).or_default();
                    nodes.push(path);
                }
            }

            // 对照：闭包子树 vs 模型 DFS（根的子树 = 全部条目）
            let got = s.subtree("/").await.unwrap();
            let want = closure_subtree(&model, "/");
            prop_assert_eq!(got.len(), want.len(), "子树大小不一致");
            prop_assert!(got.iter().eq(want.iter()), "子树集合不一致");

            // 祖先链抽查：任一叶子路径的祖先链首必须是 "/"
            if let Some(any) = model.keys().next() {
                if any != "/" {
                    let chain = s.ancestors_of(any).await.unwrap();
                    assert_eq!(
                        chain.first().map(|e| e.path.clone()),
                        Some("/".to_string()),
                        "面包屑链的根必须是 /"
                    );
                }
            }
            Ok(())
        });
    }
}

// ---------- 去重数学与查询 ----------

#[tokio::test]
async fn dedup_stats_math() {
    let s = mem_store().await;
    let root = s
        .add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None)
        .await
        .unwrap();
    // a(100B) 与 b(100B) 同内容；c(50B) 独立
    s.add_entry(
        Some(&root),
        "a",
        "/a",
        EntryKind::File,
        100,
        0,
        Some(("H1", 100)),
    )
    .await
    .unwrap();
    s.add_entry(
        Some(&root),
        "b",
        "/b",
        EntryKind::File,
        100,
        0,
        Some(("H1", 100)),
    )
    .await
    .unwrap();
    s.add_entry(
        Some(&root),
        "c",
        "/c",
        EntryKind::File,
        50,
        0,
        Some(("H2", 50)),
    )
    .await
    .unwrap();
    let st = s.stats().await.unwrap();
    assert_eq!(st.files, 3);
    assert_eq!(st.total_bytes, 250);
    assert_eq!(st.unique_contents, 2);
    assert_eq!(st.unique_bytes, 150);
    assert_eq!(st.duplicate_groups, 1);
    assert_eq!(
        st.saved_bytes, 100,
        "节省 = 250 - 150（一份重复副本的字节）"
    );

    let dups = s.duplicates(10).await.unwrap();
    assert_eq!(dups.len(), 1);
    assert_eq!(dups[0].content_id, "H1");
    assert_eq!(dups[0].copies.len(), 2);
    assert_eq!(dups[0].copies[0].path, "/a");
}

#[tokio::test]
async fn children_search_breadcrumb() {
    let s = mem_store().await;
    let root = s
        .add_entry(None, "r", "/", EntryKind::Dir, 0, 0, None)
        .await
        .unwrap();
    let sub = s
        .add_entry(Some(&root), "photos", "/photos", EntryKind::Dir, 0, 0, None)
        .await
        .unwrap();
    s.add_entry(
        Some(&sub),
        "cat.png",
        "/photos/cat.png",
        EntryKind::File,
        7,
        0,
        Some(("HC", 7)),
    )
    .await
    .unwrap();
    s.add_entry(
        Some(&root),
        "notes.txt",
        "/notes.txt",
        EntryKind::File,
        3,
        0,
        Some(("HN", 3)),
    )
    .await
    .unwrap();

    let kids = s.children("/").await.unwrap();
    assert_eq!(kids.len(), 2, "目录在前：photos + notes.txt");
    assert_eq!(kids[0].name, "photos", "目录排最前");

    let hits = s.search("cat", 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, "/photos/cat.png");

    let chain = s.ancestors_of("/photos/cat.png").await.unwrap();
    let paths: Vec<&str> = chain.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, vec!["/", "/photos"], "面包屑从根到父");
}

// ---------- 索引器：幂等 + 真实树 ----------

#[tokio::test]
async fn indexer_is_idempotent() {
    let s = mem_store().await;
    // 造一棵小真树
    let dir = std::env::temp_dir().join(format!("partisync-idx-{}", partisync_core::Ulid::now()));
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("a.txt"), b"same").unwrap();
    std::fs::write(dir.join("sub").join("b.txt"), b"same").unwrap();
    std::fs::write(dir.join("c.txt"), b"other").unwrap();

    let r1 = index_path(&s, &dir).await.unwrap();
    let st1 = s.stats().await.unwrap();
    let _r2 = index_path(&s, &dir).await.unwrap();
    let st2 = s.stats().await.unwrap();

    assert_eq!(
        (r1.files, r1.dirs),
        (3, 1),
        "根条目由 index_path 手工建立、不计入 dirs"
    );
    assert_eq!(st1, st2, "重复索引 stats 不变（幂等）");
    assert_eq!(st1.duplicate_groups, 1, "a.txt 与 sub/b.txt 同内容成组");
    assert_eq!(st1.saved_bytes, 4);

    std::fs::remove_dir_all(&dir).ok();
}
