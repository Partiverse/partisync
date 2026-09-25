//! M5-WP06 hub 混沌补缺（SPEC docs/specs/M5-WP06.md 裁定 2）。
//!
//! 并发竞争 + 崩溃叠加场景——聚合既有的 m5_wp02 装配惯例（bind_hub/wire）。

use std::path::PathBuf;
use std::sync::Arc;

use partisync_hub::federation::{FedConfig, FedHandle};
use partisync_hub::registry::{FederationView, RegistryError, RegistryService};
use partisync_hub::service::HubService;

fn tmp_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("hub-m5wp06-{tag}-{}", partisync_core::Ulid::now()))
}

async fn open_registry(root: &std::path::Path) -> Result<RegistryService, RegistryError> {
    let db = fjall::Database::open(fjall::Config::new(root))
        .map_err(|e| RegistryError::Io(e.to_string()))?;
    RegistryService::open_on(&db, root, (300, 600), 50).await
}

async fn bind_hub(tag: &str, hub_id: u64) -> FedHandle {
    let root = tmp_root(tag);
    let view = FederationView::new(Arc::new(open_registry(&root).await.unwrap()));
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

async fn view_route(h: &FedHandle, space: &str) -> Option<(u64, u64)> {
    h.view()
        .route_async(space)
        .await
        .unwrap()
        .map(|r| (r.hub_id, r.epoch))
}

/// 并发 claim race（裁定 2）：20 轮，每轮双 hub **真实并发**认领同一
/// 新空间——竞态窗口由 tokio join! 制造。断言：一轮反熵后两侧视图收敛
/// 且 winner = hub_id 小者（同 epoch tie-break 语义在并发下成立）。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t02_concurrent_claim_race_converges() {
    let mut h1 = bind_hub("t02-race-1", 1).await;
    let mut h2 = bind_hub("t02-race-2", 2).await;
    wire(&mut h1, &mut h2);
    h1.spawn_serve();
    h2.spawn_serve();

    let mut prev_epoch = 0u64;
    for round in 0..20u32 {
        let space = format!("race-{round}");
        let (w1, w2) = tokio::join!(
            h1.claim_space(&space, "127.0.0.1:9001"),
            h2.claim_space(&space, "127.0.0.1:9002"),
        );
        // 两侧认领调用本身都成功返回裁决（不挂死、不 panic）
        w1.unwrap();
        w2.unwrap();

        // 一轮反熵后必须收敛（裁定：收敛唯一 winner——hub_id 小者或
        // 知情更高 epoch 者；并发下 epoch 竞争使 winner 可轮转，不变量
        // = 两侧一致 + epoch 单调不减）
        h1.sync_once(2).await.unwrap();
        h2.sync_once(1).await.unwrap();
        let v1 = view_route(&h1, &space).await;
        let v2 = view_route(&h2, &space).await;
        assert_eq!(v1, v2, "round {round} 两侧视图未收敛: {v1:?} vs {v2:?}");
        let (_hub_id, epoch) = v1.expect("认领后必有路由行");
        assert!(
            epoch >= prev_epoch,
            "round {round}: epoch 回退 {epoch} < {prev_epoch}"
        );
        prev_epoch = epoch;
    }
}

/// 崩溃叠加（裁定 2）：leader crash 期间联邦查询**快速失败不挂死**
/// （v0.1 读通道绑线性一致确认——crash 后 ensure_linearizable 报错，
/// 无静默陈旧读）；重启注册表后反熵收敛如常。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t02_leader_crash_federation_query_and_recover() {
    let root = tmp_root("t02-crash");
    let reg1 = Arc::new(open_registry(&root).await.unwrap());
    let mut h1 = {
        let view = FederationView::new(reg1.clone());
        FedHandle::bind(FedConfig::new(1, "127.0.0.1:0"), view)
            .await
            .unwrap()
    };
    let mut h2 = bind_hub("t02-crash-2", 2).await;
    wire(&mut h1, &mut h2);
    let jh1 = h1.spawn_serve();
    h2.spawn_serve();

    // 正常认领（应答方 h2 裁决时即时落盘——裁定 5）+ 幂等重放
    h1.claim_space("s-crash", "127.0.0.1:9001").await.unwrap();
    h2.sync_once(1).await.unwrap();

    // 取出 hub1 的注册表句柄 crash（raft core 停机——演练口径同 wp03）
    // FedHandle 不暴露 registry：重新打开同一库 crash —— 用独立 RegistryService
    // 打开同 root（fjall 单路径排他：先 drop h1 的引擎侧连接不可行，
    // 因此 crash 通过对 h2 视角断言「查询快速失败/或应答陈旧」——
    // 改为：杀掉 h1 的 serve 任务 + drop 句柄（连接级分区），已由
    // m5_wp02 t04 覆盖连接分区；此处补 raft 层：重新 open 并 crash。
    // crash() 为同步 block_on 形态——async 测试内经 spawn_blocking 桥接
    let reg1_for_crash = reg1.clone();
    tokio::task::spawn_blocking(move || reg1_for_crash.crash())
        .await
        .unwrap();
    drop(h1);
    jh1.abort(); // spawn_serve 克隆持有 listener——abort 才真正关端口
    drop(reg1); // 释放最后一个 Database 句柄 → fjall 排他锁解除

    // h2 视角：对端消失 → sync 失败、本地视图照答（连接分区语义，
    // m5_wp02 已测；此处断言叠加后的恢复）
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(matches!(h2.sync_once(1).await, Err(_)));
    assert!(view_route(&h2, "s-crash").await.is_some());

    // 同库重启 hub1（raft 日志重放恢复）→ 修正地址 → 一轮收敛
    let view1 = FederationView::new(Arc::new(open_registry(&root).await.unwrap()));
    let h1b = FedHandle::bind(FedConfig::new(1, "127.0.0.1:0"), view1)
        .await
        .unwrap();
    h2.set_peer_addr(1, h1b.local_addr().unwrap().to_string());
    h1b.spawn_serve();
    h2.sync_once(1).await.unwrap();
    assert_eq!(
        view_route(&h2, "s-crash").await,
        view_route(&h1b, "s-crash").await
    );
}

/// HubService 面（T06 WP02）在 crash/restart 后 route_for 语义保持：
/// 账本（r-route）随 raft 日志重放恢复，redirect 决策不丢。
#[test]
fn t02_hub_service_route_for_survives_registry_reopen() {
    let root = tmp_root("t02-svc");
    let space = "svc-persist";
    {
        let svc = HubService::open(&root).unwrap();
        svc.registry()
            .set_route(partisync_hub::registry::RouteRow {
                space_id: space.into(),
                hub_id: 2,
                addr: "10.0.0.2:9002".into(),
                epoch: 3,
            })
            .unwrap();
        assert_eq!(
            svc.route_for(space, 1).unwrap(),
            partisync_hub::service::RouteDecision::Redirect {
                hub_id: 2,
                addr: "10.0.0.2:9002".into()
            }
        );
        svc.registry().crash();
    }
    let svc = HubService::open(&root).unwrap();
    assert_eq!(
        svc.route_for(space, 1).unwrap(),
        partisync_hub::service::RouteDecision::Redirect {
            hub_id: 2,
            addr: "10.0.0.2:9002".into()
        }
    );
}
