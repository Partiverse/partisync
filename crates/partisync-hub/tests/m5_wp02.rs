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

// ---------- T03：联邦线协议与客户端（SPEC 契约 2） ----------

use partisync_hub::federation::{absorb_routes, FedClient, FedError, FedRequest, FedResponse};
use partisync_hub::net::{read_frame, write_frame};

/// loopback 桩服务器：按 `reply` 构造应答（帧协议与真服务端一致）。
async fn spawn_stub(
    reply: impl Fn(FedRequest) -> FedResponse + Send + Sync + 'static,
) -> std::net::SocketAddr {
    use std::sync::Arc;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let reply = Arc::new(reply);
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let reply = reply.clone();
            tokio::spawn(async move {
                loop {
                    let Ok(payload) = read_frame(&mut stream).await else {
                        return;
                    };
                    let Ok(req) = serde_json::from_slice::<FedRequest>(&payload) else {
                        return;
                    };
                    let body = match serde_json::to_vec(&reply(req)) {
                        Ok(b) => b,
                        Err(_) => return,
                    };
                    if write_frame(&mut stream, &body).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    addr
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_client_roundtrip_all_variants() {
    let addr = spawn_stub(|req| match req {
        FedRequest::Hello { hub_id, addr } => FedResponse::HelloAck {
            hub_id: hub_id + 100,
            addr,
            routes: vec![route(" echoed", 9, 1)],
        },
        FedRequest::RouteQuery { space_id } => FedResponse::RouteAnswer {
            route: Some(route(&space_id, 2, 1)),
        },
        FedRequest::RouteClaim { row } => FedResponse::ClaimVerdict {
            accepted: true,
            winner: Some(row),
        },
    })
    .await;

    let mut cli = FedClient::new(addr.to_string());

    let (peer_id, _, routes) = cli.hello(1, "127.0.0.1:9001").await.unwrap();
    assert_eq!(peer_id, 101);
    assert_eq!(routes.len(), 1);

    let ans = cli.route_query("space-q").await.unwrap();
    assert!(ans.is_some_and(|r| r.hub_id == 2));

    let (accepted, winner) = cli.route_claim(route("space-c", 3, 1)).await.unwrap();
    assert!(accepted);
    assert!(winner.is_some_and(|w| w.epoch == 1));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_variant_mismatch_is_protocol_error() {
    // 应答方回错变体（RouteQuery 收到 HelloAck）→ 协议错误、连接重置
    let addr = spawn_stub(|_| FedResponse::HelloAck {
        hub_id: 9,
        addr: "127.0.0.1:9".into(),
        routes: vec![],
    })
    .await;
    let mut cli = FedClient::new(addr.to_string());
    let err = cli.route_query("s").await.unwrap_err();
    assert!(matches!(err, FedError::Protocol(_)), "got {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_absorb_routes_merges_and_is_idempotent() {
    let root_a = tmp_root("absorb-a");
    let root_b = tmp_root("absorb-b");
    let view_a = partisync_hub::registry::FederationView::new(Arc::new(
        open_registry(&root_a).await.unwrap(),
    ));
    let view_b = partisync_hub::registry::FederationView::new(Arc::new(
        open_registry(&root_b).await.unwrap(),
    ));

    view_a.set_route_async(route("s1", 1, 1)).await.unwrap();
    view_a.set_route_async(route("s2", 1, 4)).await.unwrap();

    // B 吸收 A 的视图：两行全收
    let remote = view_a.rows_async().await.unwrap();
    assert_eq!(absorb_routes(&view_b, &remote).await.unwrap(), 2);
    assert_eq!(view_b.rows_async().await.unwrap().len(), 2);

    // 幂等重放：0 改写
    let remote = view_a.rows_async().await.unwrap();
    assert_eq!(absorb_routes(&view_b, &remote).await.unwrap(), 0);

    // 陈旧行（同空间更低 epoch）不落盘
    let stale = route("s2", 9, 3);
    assert_eq!(absorb_routes(&view_b, &[stale]).await.unwrap(), 0);
    assert_eq!(view_b.route_async("s2").await.unwrap().unwrap().epoch, 4);
}

// ---------- T04：联邦服务端与反熵视图同步（SPEC 裁定 6） ----------

use partisync_hub::federation::{FedConfig, FedHandle};
use partisync_hub::registry::FederationView;

/// 绑定一个带种子视图的 hub（`127.0.0.1:0`，advertise 由测试回填）。
async fn bind_hub(tag: &str, hub_id: u64, seed: Vec<RouteRow>) -> FedHandle {
    let root = tmp_root(tag);
    let view = FederationView::new(Arc::new(open_registry(&root).await.unwrap()));
    for r in seed {
        view.set_route_async(r).await.unwrap();
    }
    FedHandle::bind(FedConfig::new(hub_id, "127.0.0.1:0"), view)
        .await
        .unwrap()
}

fn wire(a: &mut FedHandle, b: &mut FedHandle) {
    let (addr_a, addr_b) = (a.local_addr().unwrap(), b.local_addr().unwrap());
    a.set_advertised_addr(addr_a.to_string());
    b.set_advertised_addr(addr_b.to_string());
    a.set_peer_addr(b.hub_id(), addr_b.to_string());
    b.set_peer_addr(a.hub_id(), addr_a.to_string());
}

async fn view_snapshot(view: &FederationView) -> Vec<(String, u64, u64)> {
    let mut rows: Vec<(String, u64, u64)> = view
        .rows_async()
        .await
        .unwrap()
        .into_iter()
        .map(|r| (r.space_id, r.hub_id, r.epoch))
        .collect();
    rows.sort();
    rows
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t04_two_hub_bidirectional_convergence_and_idempotence() {
    let mut h1 = bind_hub("t04-conv-1", 1, vec![route("s1", 1, 1)]).await;
    let mut h2 = bind_hub("t04-conv-2", 2, vec![route("s2", 2, 1)]).await;
    wire(&mut h1, &mut h2);
    h1.spawn_serve();
    h2.spawn_serve();

    // 双向各一轮：A 吸收 B，B 吸收 A
    assert_eq!(h1.sync_once(2).await.unwrap(), (2, 1));
    assert_eq!(h2.sync_once(1).await.unwrap(), (1, 1));
    let snap1 = view_snapshot(h1.view()).await;
    let snap2 = view_snapshot(h2.view()).await;
    assert_eq!(snap1, snap2);
    assert_eq!(snap1.len(), 2);

    // 幂等重放：0 改写、视图不再变更
    assert_eq!(h1.sync_once(2).await.unwrap(), (2, 0));
    assert_eq!(h2.sync_once(1).await.unwrap(), (1, 0));
    assert_eq!(view_snapshot(h1.view()).await, snap1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t04_server_serves_hello_and_route_query() {
    let h1 = bind_hub("t04-serve", 1, vec![route("s1", 1, 1)]).await;
    let addr = h1.local_addr().unwrap().to_string();
    let view = h1.view().clone();
    h1.spawn_serve();

    let mut cli = FedClient::new(addr);
    let (peer_id, _, routes) = cli.hello(9, "127.0.0.1:9").await.unwrap();
    assert_eq!(peer_id, 1);
    assert_eq!(routes.len(), view.rows_async().await.unwrap().len());

    assert!(cli.route_query("s1").await.unwrap().is_some());
    assert!(cli.route_query("not-created").await.unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t04_peer_down_serves_stale_view_then_rejoin_converges() {
    let mut h1 = bind_hub("t04-part-1", 1, vec![route("s1", 1, 1)]).await;
    let mut h2 = bind_hub("t04-part-2", 2, vec![]).await;
    wire(&mut h1, &mut h2);
    let view1 = h1.view().clone();
    let jh1 = h1.spawn_serve();

    // 分区前：h2 吸收 s1
    assert_eq!(h2.sync_once(1).await.unwrap(), (1, 1));

    // 分区：hub 1 监听关闭（abort 任务 + 丢弃句柄）→ 同步报 IO 错误，
    // h2 陈旧视图照常应答（裁定 9）
    drop(h1);
    jh1.abort();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(matches!(h2.sync_once(1).await, Err(FedError::Io(_))));
    assert!(h2
        .view()
        .route_async("s1")
        .await
        .unwrap()
        .is_some_and(|r| r.hub_id == 1));

    // 重连：hub 1 同一注册表库以新端口重开 → 修正对端地址后一轮收敛
    let h1b = FedHandle::bind(FedConfig::new(1, "127.0.0.1:0"), view1.clone())
        .await
        .unwrap();
    h2.set_peer_addr(1, h1b.local_addr().unwrap().to_string());
    h1b.spawn_serve();
    assert_eq!(h2.sync_once(1).await.unwrap(), (1, 0)); // 已一致：幂等 0 改写
    assert_eq!(view_snapshot(h2.view()).await, view_snapshot(&view1).await);
}
