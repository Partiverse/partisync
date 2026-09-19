//! M2-WP06 验收测试（SPEC 验收标准）：placeholder 创建/hydrate/pin/oplog 跨节点传播。

use partisync_core::Ulid;
use partisync_graph::store::{EntryState, Store};
use partisync_sync::{capture, session};
use proptest::prelude::*;

async fn node(tag: &str, device: &str) -> Store {
    let dir = std::env::temp_dir().join(format!("wp6-{tag}-{}", Ulid::now()));
    std::fs::create_dir_all(&dir).unwrap();
    let s = Store::open(&dir.join("t.db")).await.unwrap();
    s.seed_device_volume(device, device, device).await.unwrap();
    s
}

#[tokio::test]
async fn placeholder_creation_and_hydration() {
    let s = node("ph-c", "dev-x").await;
    let root = s.entry_by_path("/").await.unwrap().map(|e| e.id);
    let id = s
        .add_placeholder(root.as_deref(), "doc", "/doc", 4096, 0)
        .await
        .unwrap();
    assert!(!id.is_empty());
    let row = s.entry_by_path("/doc").await.unwrap().unwrap();
    assert_eq!(row.state, EntryState::Placeholder as i64);
    assert!(row.content_id.is_none());
    assert!(row.content_hydrated_at_ns.is_none());

    s.hydrate_entry("/doc", ("HDOC", 4096)).await.unwrap();
    let row = s.entry_by_path("/doc").await.unwrap().unwrap();
    assert_eq!(row.state, EntryState::Materialized as i64);
    assert_eq!(row.content_id.as_deref(), Some("HDOC"));
    assert!(row.content_hydrated_at_ns.is_some());
}

#[tokio::test]
async fn pin_is_idempotent_and_unpin_clamps() {
    let s = node("pn-c", "dev-x").await;
    let root = s.entry_by_path("/").await.unwrap().map(|e| e.id);
    s.add_placeholder(root.as_deref(), "f", "/f", 1, 0)
        .await
        .unwrap();
    s.pin("/f").await.unwrap();
    s.pin("/f").await.unwrap();
    s.pin("/f").await.unwrap();
    let row = s.entry_by_path("/f").await.unwrap().unwrap();
    assert_eq!(row.pin_count, 1, "重复 pin 不累加");

    s.unpin("/f").await.unwrap();
    let row = s.entry_by_path("/f").await.unwrap().unwrap();
    assert_eq!(row.pin_count, 0);
    s.unpin("/f").await.unwrap(); // 不可降到负
    let row = s.entry_by_path("/f").await.unwrap().unwrap();
    assert_eq!(row.pin_count, 0, "unpin 不溢出");
}

#[tokio::test]
async fn placeholder_propagates_via_oplog_with_state_marker() {
    // a 创建占位 → push → b 收到 placeholder（content_id=NULL、state=1）
    let a = node("op-a", "dev-a").await;
    let b = node("op-b", "dev-b").await;
    let root = a.entry_by_path("/").await.unwrap().map(|e| e.id);
    // 本地以 watch 路径模拟：content 不在 → 用 add_placeholder + 单独 capture
    // capture 内 record_entry_upsert 取 row.content_id；placeholder 行
    // content_id=NULL → payload 的 content_id = NULL。
    a.add_placeholder(root.as_deref(), "media", "/media", 99999, 0)
        .await
        .unwrap();
    // oplog 推送
    capture::record_entry_upsert(&a, "/media").await.unwrap();
    session::push(&a, &b).await.unwrap();
    let row = b.entry_by_path("/media").await.unwrap().unwrap();
    assert_eq!(
        row.state,
        EntryState::Placeholder as i64,
        "对端应保留 placeholder 态"
    );
    assert!(row.content_id.is_none(), "对端 content_id 缺位");
    assert_eq!(row.owner_device.as_deref(), Some("dev-a"));

    // 本地 hydrate（content 由 WP05 块传输到位，本测试直接调 hydrate_entry 模拟）
    b.hydrate_entry("/media", ("HMEDIA", 99999)).await.unwrap();
    let row = b.entry_by_path("/media").await.unwrap().unwrap();
    assert_eq!(row.state, EntryState::Materialized as i64);
}

#[tokio::test]
async fn add_placeholder_is_idempotent_on_path_collision() {
    let s = node("idem", "dev-x").await;
    let root = s.entry_by_path("/").await.unwrap().map(|e| e.id);
    let id1 = s
        .add_placeholder(root.as_deref(), "f", "/f", 1, 0)
        .await
        .unwrap();
    let id2 = s
        .add_placeholder(root.as_deref(), "f", "/f", 2, 0)
        .await
        .unwrap();
    assert_eq!(id1, id2, "同路径幂等：返回既有 id");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8))]

    /// 随机序列：place + hydrate/pin/unpin 操作，状态始终合法（state ∈ {0,1}，
    /// pin_count ∈ [0, +∞)，不可降到负）。
    #[test]
    fn prop_placeholder_state_invariants(
        ops in prop::collection::vec((0u8..4, 0u16..32), 1..12),
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _: Result<(), proptest::test_runner::TestCaseError> = rt.block_on(async move {
            let s = node("inv", "dev-x").await;
            let root = s.entry_by_path("/").await.unwrap().map(|e| e.id);
            for (op, seed) in ops {
                let path = format!("/p{seed}");
                match op {
                    0 => { let _ = s.add_placeholder(root.as_deref(), &path[1..], &path, u64::from(seed) + 1, 0).await; }
                    1 => { let _ = s.hydrate_entry(&path, (&format!("H{seed}"), u64::from(seed) + 1)).await; }
                    2 => { let _ = s.pin(&path).await; }
                    _ => { let _ = s.unpin(&path).await; }
                }
            }
            // 全表扫描断言不变量
            let mut stack = vec![String::from("/")];
            while let Some(d) = stack.pop() {
                for e in s.children(&d).await.unwrap() {
                    assert!(e.state == 0 || e.state == 1, "state ∈ {{0,1}}");
                    assert!(e.pin_count >= 0, "pin_count 不可负");
                    if e.kind == 1 {
                        stack.push(e.path.clone());
                    }
                }
            }
            Ok(())
        });
    }
}
