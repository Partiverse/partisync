//! 真挂载 `--graph` 装配 e2e（M9-WP01-T03；SPEC M9-WP01 §3 首条 +
//! P19-a 生产链版）：环境门控——有 `/dev/fuse` 且可挂载才执行（Linux
//! 容器 `--device /dev/fuse --cap-add SYS_ADMIN` 实测路径，M7/M8 判例）；
//! 无环境（GitHub runner / 本地 mac 无 FUSE）则跳过并 stderr 留痕。
//!
//! 场景：backing 预置 docs/old.txt → 挂载（`with_wiring`，无 peer）→
//! VFS 写 create/rename/unlink → 事件经装配层进 graph/oplog → **事件源
//! 排空**（drop 汇端 → serve 收尾，连接关 → WAL checkpoint——容器多
//! pool 读可见性竞态判例的确定性替代：run3-14 十轮证据，跨 pool 轮询
//! 60s 窗仍可能不可见，改「写方收尾后断言」）→ A 侧折叠与 CLI 直写
//! 同根 + 手动 `session::push` 第二端可见（tick 驱动语义由 diag 对照
//! 测试 + wp01_wiring 覆盖）。

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

use partisync_fuse::events::FuseWriteEvent;
use partisync_gateway::wiring::{WiringOpts, WiringSession};
use partisync_graph::merkle::{entry_leaf, merkle_root, Leaf};
use partisync_graph::store::{EntryKind, Store};
use partisync_sync::session;

struct Mount {
    #[allow(dead_code)] // session 存活即保持挂载
    session: fuser::BackgroundSession,
    mp: PathBuf,
}

/// 异步装配：init（同根校验/播种）→ serve spawn → with_wiring →
/// spawn_mount。fuse 全生命周期（含 drop——PartiFuse 内嵌 cas_rt
/// Runtime，async 上下文 drop 会 panic）都在 blocking 线程。
/// 返回 (挂载, 事件汇端, **写方 store 克隆**——checkpoint 断言面)。
#[allow(clippy::type_complexity)]
async fn mount_wired(
    session: WiringSession,
    backing: PathBuf,
    tag: &str,
) -> Option<(
    Mount,
    tokio::sync::mpsc::UnboundedSender<FuseWriteEvent>,
    Store,
    Arc<AtomicU64>,
)> {
    let mp = std::env::temp_dir().join(format!("partifuse-wp01-{tag}-{}", std::process::id()));
    fs::create_dir_all(&mp).expect("mountpoint dir");
    let store = session.store().clone();
    let applied = session.applied_counter();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(session.serve(rx));
    let tx_caller = tx.clone();
    let mount_mp = mp.clone();
    let mounted = tokio::task::spawn_blocking(move || {
        let fuse = partisync_fuse::PartiFuse::with_wiring(backing, None, tx);
        partisync_fuse::spawn_mount(fuse, &mount_mp).map(|bg| Mount {
            session: bg,
            mp: mount_mp,
        })
    })
    .await
    .expect("mount join");
    let mounted = match mounted {
        Ok(bg) => bg,
        Err(e) => {
            eprintln!("[SKIP] 环境不可挂载（{e}）——探针跳过，报告按「未执行」登记");
            return None;
        }
    };
    for _ in 0..50 {
        if fs::read_dir(&mounted.mp).is_ok() {
            return Some((mounted, tx_caller, store, applied));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    eprintln!("[SKIP] 挂载后 5s 未就绪");
    // 句柄归还 blocking 线程 drop
    tokio::task::spawn_blocking(move || drop(mounted))
        .await
        .expect("drop mount");
    None
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn wp01_mount_write_reaches_peer() {
    let tmp = tempfile::tempdir().expect("tmp");
    let backing_a = tmp.path().join("backing-a");
    fs::create_dir_all(backing_a.join("docs")).expect("dir");
    fs::write(backing_a.join("docs/old.txt"), b"old").expect("file");

    let db_a = tmp.path().join("a.db");
    let opts = WiringOpts {
        backing: backing_a.clone(),
        graph_db: db_a.clone(),
        peer_db: None,
        tick_ms: 100,
    };
    let session = WiringSession::init(&opts).await.expect("init");
    let Some((mount, _tx, wiring_store, applied)) =
        mount_wired(session, backing_a.clone(), "e2e").await
    else {
        return; // 门控跳过（stderr 已留痕）
    };

    // ── VFS 写：create 新文件 → 顺序写 → release；rename；unlink ──
    let p_new = mount.mp.join("docs/new.txt");
    fs::write(&p_new, b"hello-new").expect("vfs create+write");
    let p_ren = mount.mp.join("docs/renamed.txt");
    fs::rename(&p_new, &p_ren).expect("vfs rename");
    fs::remove_file(mount.mp.join("docs/old.txt")).expect("vfs unlink");

    // ── 事件收敛（两段确定性）：① 等装配层应用完 3 个事件（纯原子
    // 计数，无 DB）；② **写方主动 checkpoint**（TRUNCATE 合回主库）→
    // 任意新连接必见全量提交（容器多 pool WAL 读竞态判例）──
    for _ in 0..100 {
        if applied.load(std::sync::atomic::Ordering::Relaxed) >= 3 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        applied.load(std::sync::atomic::Ordering::Relaxed) >= 3,
        "装配层 10s 未处理完 3 个挂载写事件"
    );
    wiring_store.wal_checkpoint().await.expect("checkpoint");
    let store_a = Store::open(&db_a).await.expect("store a");
    let leaves_a: Vec<String> = store_a
        .entry_state_leaves()
        .await
        .expect("leaves")
        .iter()
        .map(|(p, ..)| p.clone())
        .collect();
    assert_eq!(
        leaves_a,
        vec!["/docs".to_string(), "/docs/renamed.txt".to_string()],
        "A 侧终态：old.txt 移除 + renamed.txt 就位（事件全应用）"
    );

    // 折叠 vs CLI 直写同根（P19-a；owner 盖章判例——直写侧播种同款 device）
    let md = fs::symlink_metadata(backing_a.join("docs/renamed.txt").as_path()).expect("md");
    let mtime = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(0));
    let direct_db = tmp.path().join("direct.db");
    let direct = Store::open(&direct_db).await.expect("direct");
    direct
        .seed_device_volume("partifuse", "PartiFuse 本机", "direct-fp")
        .await
        .expect("seed");
    let parent = direct
        .add_entry(None, "docs", "/docs", EntryKind::Dir, 0, 0, None, None)
        .await
        .expect("dir");
    direct
        .add_entry(
            Some(&parent),
            "renamed.txt",
            "/docs/renamed.txt",
            EntryKind::File,
            md.len(),
            mtime,
            None,
            None,
        )
        .await
        .expect("file");
    let root_mount = store_root(&store_a).await;
    let root_direct = store_root(&direct).await;
    assert_eq!(
        root_mount,
        root_direct,
        "[P19-a] 真挂载折叠与 CLI 直写必须同根\nmount={:?}\ndirect={:?}",
        store_a.entry_state_leaves().await.expect("leaves"),
        direct.entry_state_leaves().await.expect("leaves"),
    );

    // ── 第二端交付（手动 push——tick 驱动语义由 diag 对照 + wp01_wiring
    // 覆盖）：可见 + size 一致 + ACK 裁剪后 A oplog 排空（不动点）──
    let peer_db = tmp.path().join("peer.db");
    let store_b = Store::open(&peer_db).await.expect("peer");
    store_b
        .seed_device_volume("peer-b", "对端", "peer-fp")
        .await
        .expect("seed peer");
    session::push(&store_a, &store_b).await.expect("push");
    let row = store_b
        .entry_by_path("/docs/renamed.txt")
        .await
        .expect("q")
        .expect("row");
    assert_eq!(
        u64::try_from(row.size).unwrap_or(0),
        b"hello-new".len() as u64,
        "对端 size 必须与挂载写内容一致"
    );
    assert!(
        store_b
            .entry_by_path("/docs/old.txt")
            .await
            .expect("q")
            .is_none(),
        "对端不得见到已 unlink 的文件"
    );
    let again = session::push(&store_a, &store_b).await.expect("push2");
    assert_eq!(again.applied, 0, "不动点：二次 push 零应用");
    assert!(
        store_a.pending_oplog().await.expect("pending").is_empty(),
        "ACK 裁剪后 A oplog 排空"
    );

    // 挂载句柄归还阻塞线程再 drop——PartiFuse 内嵌 cas_rt（tokio
    // Runtime），async 上下文 drop runtime 会 panic（运行时判例）。
    drop(store_a);
    drop(store_b);
    drop(direct);
    tokio::task::spawn_blocking(move || drop(mount))
        .await
        .expect("drop mount on blocking thread");
}
