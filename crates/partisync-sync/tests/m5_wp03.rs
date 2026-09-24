//! M5-WP03 集成测试：分布式扫描调度器（SPEC docs/specs/M5-WP03.md）。
//!
//! T02：分片模型与清单源——`impl ListSource for Provider`（fs tmpdir
//! 真实树：文件/子目录混合、空目录、根前缀、路径无前导 `/`）。

use std::path::PathBuf;

use partisync_provider::config::{ProviderConfig, ProviderScheme};
use partisync_provider::Provider;
use partisync_sync::scan::{ListSource, ListedNode};

fn tmp_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sync-m5wp03-{tag}-{}", partisync_core::Ulid::now()))
}

/// fs Provider 指向 tmpdir。
fn fs_provider(root: &std::path::Path) -> Provider {
    let mut params = serde_json::Map::new();
    params.insert("root".into(), serde_json::json!(root.to_string_lossy()));
    Provider::from_config(&ProviderConfig {
        scheme: ProviderScheme::Fs,
        params,
    })
    .unwrap()
}

/// 建确定性测试树：
/// ```text
/// root/
///   a.txt              (文件, 3B)
///   dir1/              (2 文件)
///     f1.txt  f2.txt
///   dir2/              (空目录)
///   dir2x/             (嵌套)
///     deep/
///       leaf.txt
/// ```
fn seed_tree(root: &std::path::Path) {
    std::fs::create_dir_all(root.join("dir1")).unwrap();
    std::fs::create_dir_all(root.join("dir2")).unwrap();
    std::fs::create_dir_all(root.join("dir2x/deep")).unwrap();
    std::fs::write(root.join("a.txt"), b"abc").unwrap();
    std::fs::write(root.join("dir1/f1.txt"), b"1234").unwrap();
    std::fs::write(root.join("dir1/f2.txt"), b"56").unwrap();
    std::fs::write(root.join("dir2x/deep/leaf.txt"), b"x").unwrap();
}

fn sorted_names(nodes: &[ListedNode]) -> Vec<(String, bool)> {
    let mut v: Vec<(String, bool)> = nodes.iter().map(|n| (n.path.clone(), n.is_dir)).collect();
    v.sort();
    v
}

#[tokio::test]
async fn t02_provider_list_dir_root_level() {
    let root = tmp_root("root-level");
    seed_tree(&root);
    let src = fs_provider(&root);

    let nodes = src.list_dir("").await.unwrap();
    assert_eq!(
        sorted_names(&nodes),
        vec![
            ("a.txt".into(), false),
            ("dir1".into(), true),
            ("dir2".into(), true),
            ("dir2x".into(), true),
        ]
    );
    // 路径无前导 `/`（口径与 ProviderEntry 一致）
    assert!(nodes.iter().all(|n| !n.path.starts_with('/')));
    let a = nodes.iter().find(|n| n.path == "a.txt").unwrap();
    assert_eq!(a.size, 3);
}

#[tokio::test]
async fn t02_provider_list_dir_subtree_and_empty() {
    let root = tmp_root("subtree");
    seed_tree(&root);
    let src = fs_provider(&root);

    let dir1 = src.list_dir("dir1").await.unwrap();
    assert_eq!(
        sorted_names(&dir1),
        vec![("dir1/f1.txt".into(), false), ("dir1/f2.txt".into(), false)]
    );

    // 空目录 = 空清单（合法，BFS 展开）
    assert!(src.list_dir("dir2").await.unwrap().is_empty());

    // 嵌套子目录前缀（无前导 /）
    let deep = src.list_dir("dir2x/deep").await.unwrap();
    assert_eq!(
        sorted_names(&deep),
        vec![("dir2x/deep/leaf.txt".into(), false)]
    );
    assert_eq!(deep[0].parent_dir(), "dir2x/deep");
}

#[tokio::test]
async fn t02_listed_node_parent_dir() {
    let n = ListedNode {
        path: "a/b/c.txt".into(),
        is_dir: false,
        size: 1,
        mtime_ns: 0,
    };
    assert_eq!(n.parent_dir(), "a/b");
    let top = ListedNode {
        path: "top.txt".into(),
        is_dir: false,
        size: 0,
        mtime_ns: 0,
    };
    assert_eq!(top.parent_dir(), "");
}
