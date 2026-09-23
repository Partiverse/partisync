//! M5-WP02 集成测试：联邦路由协议（SPEC docs/specs/M5-WP02.md）。
//!
//! T02：路由视图 raft 化——`RegistryCmd::SetRoute` 经注册表组（pid=0）
//! 日志落盘、crash 重开不丢、线性一致读、合并规则确定性
//! （epoch 大者胜 / 同 epoch hub_id 小者胜 / 相等 no-op）。

use std::path::PathBuf;
use std::sync::Arc;

use partisync_hub::registry::{merge_route, RegistryError, RegistryService, RouteRow};

fn tmp_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("hub-m5wp02-{tag}-{}", partisync_core::Ulid::now()))
}

/// 打开单节点注册表组（选举/心跳 = HubService 默认 300-600/50ms）。
async fn open_registry(root: &std::path::Path) -> Result<RegistryService, RegistryError> {
    let db = fjall::Database::open(fjall::Config::new(root))
        .map_err(|e| RegistryError::Io(e.to_string()))?;
    RegistryService::open_on(&db, root, (300, 600), 50).await
}

fn route(space: &str, hub_id: u64, epoch: u64) -> RouteRow {
    RouteRow {
        space_id: space.to_owned(),
        hub_id,
        addr: format!("127.0.0.1:9{hub_id:03}"),
        epoch,
    }
}

// ---------- 合并规则（SPEC 裁定 4/5，纯函数） ----------

#[test]
fn t02_merge_no_local_adopts_incoming() {
    assert_eq!(merge_route(None, &route("s", 1, 1)), Some(route("s", 1, 1)));
}

#[test]
fn t02_merge_higher_epoch_wins() {
    let local = route("s", 1, 1);
    let incoming = route("s", 2, 2); // 更高 epoch 胜，与 hub_id 无关
    assert_eq!(merge_route(Some(&local), &incoming), Some(incoming.clone()));
    // 持高 epoch 行的视图收到陈旧行：保持（None）——双侧收敛于高 epoch 行
    assert_eq!(merge_route(Some(&incoming), &local), None);
}

#[test]
fn t02_merge_tie_breaks_lower_hub_id() {
    let a = route("s", 1, 3);
    let b = route("s", 2, 3); // 同 epoch：hub_id 小者胜
    assert_eq!(merge_route(Some(&b), &a), Some(a.clone()));
    // 持 a 的视图收到 b：a 胜且 a == 本地 → None——双侧收敛于 a
    assert_eq!(merge_route(Some(&a), &b), None);
}

#[test]
fn t02_merge_identical_is_noop_and_stale_loses() {
    let row = route("s", 1, 3);
    assert_eq!(merge_route(Some(&row), &row), None); // 幂等：相等 no-op
    let stale = route("s", 9, 2);
    assert_eq!(merge_route(Some(&row), &stale), None); // 陈旧提案落败
}

#[test]
fn t02_merge_transfer_via_epoch_bump() {
    // 归属转移（裁定 4）：新持有者以 epoch+1 提案即胜出
    let old = route("s", 1, 1);
    let mut claim = route("s", 2, 1);
    claim = claim.with_epoch_bumped();
    assert_eq!(claim.epoch, 2);
    assert_eq!(merge_route(Some(&old), &claim), Some(claim));
}

// ---------- 路由视图 raft 化（裁定 2） ----------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t02_set_route_roundtrip_and_full_view() {
    let root = tmp_root("roundtrip");
    let reg = Arc::new(open_registry(&root).await.unwrap());
    reg.set_route_async(route("space-a", 1, 1)).await.unwrap();
    reg.set_route_async(route("space-b", 2, 1)).await.unwrap();

    let row = reg.route_async("space-a").await.unwrap().unwrap();
    assert_eq!(row.hub_id, 1);
    assert_eq!(row.addr, "127.0.0.1:9001");

    // upsert 同键覆盖
    reg.set_route_async(route("space-a", 3, 2)).await.unwrap();
    assert_eq!(reg.route_async("space-a").await.unwrap().unwrap().hub_id, 3);

    let mut spaces: Vec<String> = reg
        .routes_async()
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.space_id)
        .collect();
    spaces.sort();
    assert_eq!(spaces, vec!["space-a".to_owned(), "space-b".to_owned()]);
    assert!(reg.route_async("missing").await.unwrap().is_none());
}

#[test]
fn t02_routes_survive_crash_and_reopen() {
    // 同步形态：注册表组的 Runtime 归测试自管（Handle::block_on 不得进
    // 异步上下文——wp03 HubService::open 同款模式）
    let root = tmp_root("crash-reopen");
    {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let reg = rt.block_on(open_registry(&root)).unwrap();
        reg.set_route(route("space-x", 7, 5)).unwrap();
        reg.set_route(route("space-y", 8, 1)).unwrap();
        assert_eq!(reg.route("space-x").unwrap().unwrap().epoch, 5);
        reg.crash();
        drop(reg);
        drop(rt);
    }
    // 重开：r-route 业务节随组 0 日志重放恢复（已 ACK 写不丢）
    let rt = tokio::runtime::Runtime::new().unwrap();
    let reg = rt.block_on(open_registry(&root)).unwrap();
    let x = reg.route("space-x").unwrap().unwrap();
    assert_eq!((x.hub_id, x.epoch), (7, 5));
    assert_eq!(reg.route("space-y").unwrap().unwrap().hub_id, 8);
    assert_eq!(reg.routes().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t02_federation_view_narrow_face() {
    let root = tmp_root("view");
    let reg = Arc::new(open_registry(&root).await.unwrap());
    let view = partisync_hub::registry::FederationView::new(reg.clone());
    view.set_route_async(route("space-v", 4, 1)).await.unwrap();
    assert_eq!(
        view.route_async("space-v").await.unwrap().unwrap().hub_id,
        4
    );
    assert_eq!(view.rows_async().await.unwrap().len(), 1);
}
