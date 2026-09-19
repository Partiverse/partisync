//! M2-WP08 验收测试（SPEC 验收标准）：staggered K 滚动裁剪、trash+resurrect、GC、pin 防回收。

use partisync_core::Ulid;
use partisync_graph::store::{EntryKind, Store};
use proptest::prelude::*;

async fn node(tag: &str, device: &str) -> Store {
    let dir = std::env::temp_dir().join(format!("wp8-{tag}-{}", Ulid::now()));
    std::fs::create_dir_all(&dir).unwrap();
    let s = Store::open(&dir.join("t.db")).await.unwrap();
    s.seed_device_volume(device, device, device).await.unwrap();
    s
}

async fn overwrite(store: &Store, path: &str, hash: &str, size: u64) {
    let _ = store.retire_for_overwrite(path).await;
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
}

#[tokio::test]
async fn staggered_keeps_last_5_versions() {
    let s = node("stg", "dev-x").await;
    for i in 0..7 {
        overwrite(&s, "/v", &format!("H{i}"), 1).await;
    }
    // 7 个退役版本 + 1 个现行 entry = 7 staggered；调 GC 应滚动裁剪到 5
    s.gc_expired().await.unwrap();
    let versions = s.list_versions("/v").await.unwrap();
    assert_eq!(versions.len(), 5, "超出 K=5 的旧版本应被 GC 滚动裁剪");
    // 7 次写入产生 6 次退役（H0..H5），现行 H6；GC K=5 应保留 H1..H5（H0 最旧被裁剪）
    let hashes: Vec<_> = versions
        .iter()
        .map(|v| v.content_id.clone().unwrap_or_default())
        .collect();
    assert!(hashes.contains(&"H1".to_string()));
    assert!(hashes.contains(&"H5".to_string()));
    assert!(!hashes.contains(&"H0".to_string()), "最旧版本应被裁剪");
}

#[tokio::test]
async fn gc_removes_expired_trash() {
    let s = node("gce", "dev-x").await;
    overwrite(&s, "/t", "HT", 1).await;
    // ttl = -1s：已过期
    s.trash_entry("/t", -1_000_000_000).await.unwrap();
    assert!(
        s.entry_by_path("/t").await.unwrap().is_none(),
        "trash 后 entry 已删"
    );
    let versions_before = s.list_versions("/t").await.unwrap();
    assert_eq!(versions_before.len(), 1);
    assert_eq!(versions_before[0].state, 1);

    let n = s.gc_expired().await.unwrap();
    assert_eq!(n, 1, "过期 trashed 行被删");
    assert!(s.list_versions("/t").await.unwrap().is_empty());
}

#[tokio::test]
async fn pin_blocks_trash() {
    let s = node("pn", "dev-x").await;
    overwrite(&s, "/p", "HP", 1).await;
    s.pin("/p").await.unwrap();
    s.trash_entry("/p", 30 * 24 * 3600 * 1_000_000_000)
        .await
        .unwrap();
    assert!(
        s.entry_by_path("/p").await.unwrap().is_some(),
        "pin 防回收：trash 失效"
    );
    assert!(
        s.list_versions("/p").await.unwrap().is_empty(),
        "无 trashed 版本行"
    );
}

#[tokio::test]
async fn trash_then_resurrect_restores_entry() {
    let s = node("rsc", "dev-x").await;
    overwrite(&s, "/r", "HR", 42).await;
    s.trash_entry("/r", 30 * 24 * 3600 * 1_000_000_000)
        .await
        .unwrap();
    assert!(s.entry_by_path("/r").await.unwrap().is_none());
    let id = s.resurrect("/r").await.unwrap();
    assert!(!id.is_empty());
    let row = s.entry_by_path("/r").await.unwrap().unwrap();
    assert_eq!(row.size, 42);
    assert_eq!(row.content_id.as_deref(), Some("HR"));
}

#[tokio::test]
async fn resurrect_rejects_already_existing_path() {
    let s = node("rscx", "dev-x").await;
    overwrite(&s, "/a", "HA", 1).await;
    overwrite(&s, "/b", "HB", 1).await;
    // /a 已 trash，路径上现有 /b → resurrect 不应覆盖
    s.trash_entry("/a", 30 * 24 * 3600 * 1_000_000_000)
        .await
        .unwrap();
    // 试图 resurrect /a 时 /a 实际已删：此情况合法（返回既有 trash 行的 id）
    let id = s.resurrect("/a").await.unwrap();
    assert!(!id.is_empty());
    // 但如果路径上已有 entry，应 Fatal
    overwrite(&s, "/c", "HC", 1).await;
    s.trash_entry("/c", 30 * 24 * 3600 * 1_000_000_000)
        .await
        .unwrap();
    overwrite(&s, "/c", "HC2", 2).await;
    let err = s.resurrect("/c").await.unwrap_err();
    assert!(
        err.to_string().contains("已存在"),
        "resurrect 不得覆盖现有路径: {err}"
    );
}

#[tokio::test]
async fn gc_keeps_unexpired_trash() {
    let s = node("gcun", "dev-x").await;
    overwrite(&s, "/u", "HU", 1).await;
    // 30 天 = 30×86400×10⁹ ns，尚未到期
    s.trash_entry("/u", 30 * 86_400_000_000_000).await.unwrap();
    let n = s.gc_expired().await.unwrap();
    assert_eq!(n, 0, "未到期 trashed 不应被 GC");
    assert_eq!(s.list_versions("/u").await.unwrap().len(), 1);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8))]

    /// 随机覆盖写序列 → 每 path 至多 K=5 staggered 版本行。
    #[test]
    fn prop_staggered_bound_is_k(
        writes in prop::collection::vec((1u8..4, 1u16..32), 1..30),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _: Result<(), proptest::test_runner::TestCaseError> = rt.block_on(async move {
            let s = node("p-k", "dev-x").await;
            for (i, p) in writes {
                let path = format!("/p{p}");
                overwrite(&s, &path, &format!("HK{i}"), u64::from(i) + 1).await;
            }
            s.gc_expired().await.unwrap();
            // 抽样几条路径验证 K=5 上界
            let mut stack = vec![String::from("/")];
            while let Some(d) = stack.pop() {
                for e in s.children(&d).await.unwrap() {
                    if e.kind == 0 {
                        let v = s.list_versions(&e.path).await.unwrap();
                        assert!(v.len() <= 5, "{} 版本数 {} > K=5", e.path, v.len());
                    }
                    if e.kind == 1 {
                        stack.push(e.path.clone());
                    }
                }
            }
            Ok(())
        });
    }
}
