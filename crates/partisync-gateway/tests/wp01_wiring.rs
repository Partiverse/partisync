//! M9-WP01-T02 装配层探针（P19；SPEC M9-WP01 §2.4/§3）：
//!
//! - [P19-a] 折叠/直写同库同根（wiring_e2e 判据生产化——经真实
//!   `WiringSession::serve` 链而非测试手搓事件流）+ bisync 不动点
//!   `pending_oplog` 双端清零；
//! - [P19-b] origin 剪枝：对端回流行 `skipped_self_origin` 计数；
//! - [P19-c] 同一事件重复投递 → graph 行幂等（entry 数稳定、终态一致）；
//! - Rename 折叠 Created 带 size（T05 叶哈希判例）；
//! - 同根校验拒绝（graph 卷指纹 ≠ backing）。
//!
//! 不依赖真实挂载（/dev/fuse）——事件经 channel 直接注入，与 fuse
//! 钩子位同型（probe_mount 容器探针覆盖真实挂载面）。

use std::path::Path;
use std::time::Duration;

use partisync_fuse::events::FuseWriteEvent;
use partisync_gateway::wiring::{WiringOpts, WiringSession};
use partisync_graph::merkle::{entry_leaf, merkle_root, Leaf};
use partisync_graph::store::{EntryKind, Store};
use partisync_sync::session;

/// backing 文件 metadata → (size, mtime_ns)（fold 同款换算）。
fn md_props(p: &Path) -> (u64, u64) {
    let md = std::fs::symlink_metadata(p).expect("metadata");
    let mtime = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(0));
    (md.len(), mtime)
}

async fn store_root(store: &Store) -> String {
    let rows = store.entry_state_leaves().await.expect("leaves");
    let leaves: Vec<Leaf> = rows
        .iter()
        .map(|(path, kind, content, owner, size, mtime)| {
            entry_leaf(
                path,
                *kind,
                content.as_deref(),
                owner.as_deref(),
                u64::try_from(*size).unwrap_or(0),
                u64::try_from(*mtime).unwrap_or(0),
            )
        })
        .collect();
    merkle_root(&leaves)
}

/// 启动装配会话（serve 后台循环；返回事件汇端）。
async fn spawn_session(opts: WiringOpts) -> tokio::sync::mpsc::UnboundedSender<FuseWriteEvent> {
    let session = WiringSession::init(&opts).await.expect("init");
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(session.serve(rx));
    tx
}

/// 轮询直到 `f` 为真（超时 2s panic——不做 sleep 定时，消除 flake）。
async fn wait_until(f: impl Fn() -> bool) {
    for _ in 0..200 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("wait_until 超时（2s）");
}

fn opts(backing: &Path, db: &Path) -> WiringOpts {
    WiringOpts {
        backing: backing.to_path_buf(),
        graph_db: db.to_path_buf(),
        peer_db: None,
        tick_ms: 60_000, // 测试手动驱动 push/bisync，tick 不参与
    }
}

/// [P19-a] 真实 serve 链折叠：挂载写事件流（Upsert→Rename）与 CLI 直写
/// 同库形态下 Merkle 同根；capture 进 oplog；push 收敛后对端可见且
/// 不动点（二次 push applied=0）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn p19a_fold_matches_direct_write_and_converges() {
    let tmp = tempfile::tempdir().expect("tmp");
    let backing = tmp.path().join("backing");
    std::fs::create_dir_all(backing.join("docs")).expect("dir");
    std::fs::write(backing.join("docs/a.txt"), b"v1").expect("file");

    let db = tmp.path().join("a.db");
    let tx = spawn_session(opts(&backing, &db)).await;

    // 事件流：create 落盘 → Upsert；随后 backing 内改名 → Rename
    std::fs::rename(backing.join("docs/a.txt"), backing.join("docs/b.txt")).expect("rename");
    tx.send(FuseWriteEvent::Upsert {
        path: "docs/a.txt".into(),
    })
    .expect("send");
    tx.send(FuseWriteEvent::Rename {
        from: "docs/a.txt".into(),
        to: "docs/b.txt".into(),
    })
    .expect("send");

    let store = Store::open(&db).await.expect("store");
    wait_until(|| {
        // serve 异步应用——轮询终态行出现
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                store
                    .entry_by_path("/docs/b.txt")
                    .await
                    .ok()
                    .flatten()
                    .is_some()
            })
        })
    })
    .await;
    let gone = store.entry_by_path("/docs/a.txt").await.expect("q");
    assert!(gone.is_none(), "Rename 源行应已移除");

    // 折叠 vs CLI 直写同根（T05 判据；size/mtime 取 backing 终态）。
    // 直写侧须播种同款 device——add_entry 会把 owner=None 盖成本机
    // device（wiring_e2e 判例同款），owner 进叶哈希。
    let (size, mtime) = md_props(&backing.join("docs/b.txt"));
    let direct_db = tmp.path().join("direct.db");
    let direct = Store::open(&direct_db).await.expect("direct");
    direct
        .seed_device_volume("partifuse", "PartiFuse 本机", "direct-fp")
        .await
        .expect("seed direct");
    let parent = direct
        .add_entry(None, "docs", "/docs", EntryKind::Dir, 0, 0, None, None)
        .await
        .expect("dir");
    direct
        .add_entry(
            Some(&parent),
            "b.txt",
            "/docs/b.txt",
            EntryKind::File,
            size,
            mtime,
            None,
            None,
        )
        .await
        .expect("file");
    let root_fold = store_root(&store).await;
    let root_direct = store_root(&direct).await;
    assert_eq!(
        root_fold,
        root_direct,
        "[P19-a] 折叠与直写必须同根\nfold={:?}\ndirect={:?}",
        store.entry_state_leaves().await.expect("leaves"),
        direct.entry_state_leaves().await.expect("leaves"),
    );

    // push 收敛：对端可见 + 不动点（二次 push applied=0）
    let peer_db = tmp.path().join("peer.db");
    let peer = Store::open(&peer_db).await.expect("peer");
    peer.seed_device_volume("peer-b", "对端", "peer-fp")
        .await
        .expect("seed peer");
    session::push(&store, &peer).await.expect("push");
    assert!(
        peer.entry_by_path("/docs/b.txt")
            .await
            .expect("q")
            .is_some(),
        "push 后对端应可见挂载写产物"
    );
    let again = session::push(&store, &peer).await.expect("push2");
    assert_eq!(again.applied, 0, "[P19-a] 不动点：二次 push 零应用");
}

/// [P19-b] origin 剪枝：对端中继回流行不回流应用。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn p19b_relayed_rows_pruned_by_origin() {
    let tmp = tempfile::tempdir().expect("tmp");
    let backing = tmp.path().join("backing");
    std::fs::create_dir_all(&backing).expect("dir");
    std::fs::write(backing.join("f.txt"), b"hello").expect("file");

    let db = tmp.path().join("a.db");
    let tx = spawn_session(opts(&backing, &db)).await;
    tx.send(FuseWriteEvent::Upsert {
        path: "f.txt".into(),
    })
    .expect("send");

    let store = Store::open(&db).await.expect("store");
    wait_until(|| {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { store.entry_by_path("/f.txt").await.ok().flatten().is_some() })
        })
    })
    .await;

    let peer_db = tmp.path().join("peer.db");
    let peer = Store::open(&peer_db).await.expect("peer");
    peer.seed_device_volume("peer-b", "对端", "peer-fp")
        .await
        .expect("seed peer");
    session::push(&store, &peer).await.expect("push");
    // 中继：peer 应用成功 → 进 peer oplog（origin 仍 = 源设备）→ 回推
    // 时被 origin 剪枝（skipped_self_origin），不产生二次应用。
    let back = session::push(&peer, &store).await.expect("push back");
    assert!(
        back.skipped_self_origin >= 1,
        "[P19-b] 回流行必须被 origin 剪枝"
    );
    // 回推后 A 无新应用（该行 A 已有），oplog 稳定。
    let again = session::push(&store, &peer).await.expect("push3");
    assert_eq!(again.applied, 0, "剪枝后不再产生交叉应用");
}

/// [P19-c] 同一事件重复投递 → graph 行幂等（entry 数稳定、终态一致）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn p19c_duplicate_delivery_idempotent() {
    let tmp = tempfile::tempdir().expect("tmp");
    let backing = tmp.path().join("backing");
    std::fs::create_dir_all(&backing).expect("dir");
    std::fs::write(backing.join("f.txt"), b"payload").expect("file");

    let db = tmp.path().join("a.db");
    let tx = spawn_session(opts(&backing, &db)).await;
    for _ in 0..3 {
        tx.send(FuseWriteEvent::Upsert {
            path: "f.txt".into(),
        })
        .expect("send");
    }

    let store = Store::open(&db).await.expect("store");
    wait_until(|| {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { store.entry_by_path("/f.txt").await.ok().flatten().is_some() })
        })
    })
    .await;
    // 应用循环排空后终态断言（重复投递不增行）
    tokio::time::sleep(Duration::from_millis(100)).await;
    let rows = store.entry_state_leaves().await.expect("leaves");
    assert_eq!(rows.len(), 1, "[P19-c] 重复投递 entry 数必须稳定");
    let row = store
        .entry_by_path("/f.txt")
        .await
        .expect("q")
        .expect("row");
    let (size, _) = md_props(&backing.join("f.txt"));
    assert_eq!(
        u64::try_from(row.size).unwrap_or(0),
        size,
        "[P19-c] 终态 size 一致"
    );
}

/// 同根校验拒绝：graph 已绑定其他卷指纹 → init 失败（SPEC §2.1）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn root_mismatch_rejects_assembly() {
    let tmp = tempfile::tempdir().expect("tmp");
    let backing_a = tmp.path().join("backing-a");
    let backing_b = tmp.path().join("backing-b");
    std::fs::create_dir_all(&backing_a).expect("dir");
    std::fs::create_dir_all(&backing_b).expect("dir");

    let db = tmp.path().join("a.db");
    let _ = spawn_session(opts(&backing_a, &db)).await; // 绑定 A 卷
    match WiringSession::init(&opts(&backing_b, &db)).await {
        Err(e) => assert!(e.contains("同根校验失败"), "错误面须点明同根校验: {e}"),
        Ok(_) => panic!("同根校验必须拒绝异卷装配"),
    }
}

/// 对照实验（T03 探针定位）：同 wp01_wiring 结构 + **tick(bisync) 并发**——
/// 若容器内本测试同样「serve 自见、外部 pool 不可见」，则钉在 tick 与
/// 应用路径的交互而非挂载面。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn diag_tick_visibility_control() {
    let tmp = tempfile::tempdir().expect("tmp");
    let backing = tmp.path().join("backing");
    std::fs::create_dir_all(&backing).expect("dir");
    std::fs::write(backing.join("f.txt"), b"hello").expect("file");

    let db_a = tmp.path().join("a.db");
    let db_b = tmp.path().join("b.db");
    let opts = WiringOpts {
        backing: backing.clone(),
        graph_db: db_a.clone(),
        peer_db: Some(db_b.clone()),
        tick_ms: 20,
    };
    let session = WiringSession::init(&opts).await.expect("init");
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(session.serve(rx));
    tx.send(FuseWriteEvent::Upsert {
        path: "f.txt".into(),
    })
    .expect("send");

    for _ in 0..200 {
        let store = Store::open(&db_a).await.expect("open");
        if store.entry_by_path("/f.txt").await.ok().flatten().is_some() {
            return; // 可见 = tick 不破坏外部可见性
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("tick 并发下外部 pool 10s 不可见");
}
