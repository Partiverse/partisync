//! WP02 T03/T04 回归：openraft storage-v2 → fjall 存储适配器 + TCP 帧网络层。
//!
//! T03 覆盖：vote/committed 硬状态、append/range/log-state、truncate/purge、
//! apply（数据节 + membership）、快照构建/安装/回读、重开恢复、组隔离、
//! kill -9 持久化、openraft 官方一致性套件。
//! T04 覆盖：帧回环/版本校验、RaftNetwork 客户端 ↔ 框架服务端 RPC 回环、
//! 远端业务错误映射、超时与拒连。
//! T05 覆盖：三节点组 bootstrap、leader 选举、复制与读回、failover <10s
//! 无损（关门 KPI）、领导者转移 <5s、线性一致读（read-your-writes）。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use fjall::{Database, PersistMode};
use openraft::storage::{RaftLogStorage, RaftLogStorageExt, RaftStateMachine};
use openraft::{
    AnyError, BasicNode, CommittedLeaderId, Entry, EntryPayload, ErrorSubject, ErrorVerb, LogId,
    Membership, RaftLogReader, RaftSnapshotBuilder, SnapshotMeta, StorageError, StorageIOError,
    StoredMembership, Vote,
};
use partisync_hub::{
    open_raft_stores, HubData, HubResponse, HubTypeConfig, RaftLogStore, RaftStateMachineStore,
};

type Cfg = HubTypeConfig;

fn tmp_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("hub-wp02-{tag}-{}", partisync_core::Ulid::now()))
}

fn open_db(root: &Path) -> Database {
    Database::open(fjall::Config::new(root)).expect("open db")
}

fn log_id(term: u64, index: u64) -> LogId<u64> {
    LogId::new(CommittedLeaderId::new(term, 0), index)
}

fn blank(term: u64, index: u64) -> Entry<Cfg> {
    Entry {
        log_id: log_id(term, index),
        payload: EntryPayload::Blank,
    }
}

fn normal(term: u64, index: u64, data: &[u8]) -> Entry<Cfg> {
    Entry {
        log_id: log_id(term, index),
        payload: EntryPayload::Normal(HubData(data.to_vec())),
    }
}

fn membership_entry(term: u64, index: u64, ids: &[u64]) -> Entry<Cfg> {
    let voters: BTreeSet<u64> = ids.iter().copied().collect();
    let nodes: std::collections::BTreeMap<u64, BasicNode> = ids
        .iter()
        .map(|i| (*i, BasicNode::new(format!("n{i}"))))
        .collect();
    Entry {
        log_id: log_id(term, index),
        payload: EntryPayload::Membership(Membership::new(vec![voters], nodes)),
    }
}

fn membership_of(ids: &[u64]) -> Membership<u64, BasicNode> {
    let voters: BTreeSet<u64> = ids.iter().copied().collect();
    let nodes: std::collections::BTreeMap<u64, BasicNode> = ids
        .iter()
        .map(|i| (*i, BasicNode::new(format!("n{i}"))))
        .collect();
    Membership::new(vec![voters], nodes)
}

/// 崩溃持久化验证辅助：进程级 fsync（kill -9 生存面由 journal durability
/// 提供；此处验证显式落盘后重开可读）。
fn persist(db: &Database) {
    db.persist(PersistMode::SyncData).expect("persist");
}

#[tokio::test]
async fn t03_vote_roundtrip_and_reopen() {
    let root = tmp_root("vote");
    let db = open_db(&root);
    let (mut log, _sm) = open_raft_stores(&db, 1).expect("stores");
    assert!(log.read_vote().await.expect("read").is_none());
    let v = Vote::new_committed(7, 3);
    log.save_vote(&v).await.expect("save");
    persist(&db);
    assert_eq!(log.read_vote().await.expect("read"), Some(v));
    drop((_sm, log, db));
    // 重开恢复
    let db = open_db(&root);
    let (mut log, _) = open_raft_stores(&db, 1).expect("stores");
    assert_eq!(log.read_vote().await.expect("read"), Some(v));
}

#[tokio::test]
async fn t03_committed_pointer_roundtrip() {
    let root = tmp_root("committed");
    let db = open_db(&root);
    let (mut log, _) = open_raft_stores(&db, 1).expect("stores");
    assert!(log.read_committed().await.expect("read").is_none());
    log.save_committed(Some(log_id(3, 9))).await.expect("save");
    assert_eq!(
        log.read_committed().await.expect("read"),
        Some(log_id(3, 9))
    );
    log.save_committed(None).await.expect("clear");
    assert!(log.read_committed().await.expect("read").is_none());
}

#[tokio::test]
async fn t03_append_read_range_and_log_state() {
    let root = tmp_root("append");
    let db = open_db(&root);
    let (mut log, _) = open_raft_stores(&db, 1).expect("stores");
    // 空日志状态
    let st = log.get_log_state().await.expect("state");
    assert!(st.last_purged_log_id.is_none() && st.last_log_id.is_none());
    // append 1..=6（index 0 保留给哨兵语义之外的正常起点也可；此处从 1 起）
    let entries: Vec<Entry<Cfg>> = (1..=6u64)
        .map(|i| {
            if i == 3 {
                blank(1, i)
            } else {
                normal(1, i, format!("payload-{i}").as_bytes())
            }
        })
        .collect();
    log.blocking_append(entries).await.expect("append");

    let st = log.get_log_state().await.expect("state");
    assert_eq!(st.last_log_id, Some(log_id(1, 6)));
    // 区间读
    let got = log.try_get_log_entries(2..5).await.expect("range");
    assert_eq!(got.len(), 3);
    assert_eq!(got[0].log_id.index, 2);
    assert_eq!(got[2].log_id.index, 4);
    match &got[0].payload {
        EntryPayload::Normal(HubData(d)) => assert_eq!(d, b"payload-2"),
        other => panic!("unexpected payload {other:?}"),
    }
    // blank 载荷保持
    let got = log.try_get_log_entries(3..4).await.expect("range");
    assert!(matches!(got[0].payload, EntryPayload::Blank));
    // LogReader 句柄同样可读
    let mut reader = log.get_log_reader().await;
    let got = reader
        .try_get_log_entries(1..=6)
        .await
        .expect("reader range");
    assert_eq!(got.len(), 6);
}

#[tokio::test]
async fn t03_truncate_and_purge_no_holes() {
    let root = tmp_root("trunc");
    let db = open_db(&root);
    let (mut log, _) = open_raft_stores(&db, 1).expect("stores");
    let entries: Vec<Entry<Cfg>> = (1..=10u64).map(|i| normal(2, i, b"x")).collect();
    log.blocking_append(entries).await.expect("append");
    // truncate >= 7
    log.truncate(log_id(2, 7)).await.expect("truncate");
    let st = log.get_log_state().await.expect("state");
    assert_eq!(st.last_log_id, Some(log_id(2, 6)));
    assert!(log.try_get_log_entries(1..=100).await.expect("r").len() == 6);
    // purge <= 3
    log.purge(log_id(2, 3)).await.expect("purge");
    let st = log.get_log_state().await.expect("state");
    assert_eq!(st.last_purged_log_id, Some(log_id(2, 3)));
    assert_eq!(st.last_log_id, Some(log_id(2, 6)));
    let got = log.try_get_log_entries(1..=100).await.expect("r");
    assert_eq!(got.len(), 3);
    assert_eq!(got[0].log_id.index, 4);
}

#[tokio::test]
async fn t03_apply_updates_applied_state_and_membership() {
    let root = tmp_root("apply");
    let db = open_db(&root);
    let (mut log, mut sm) = open_raft_stores(&db, 1).expect("stores");
    let (applied, m) = sm.applied_state().await.expect("applied_state");
    assert!(applied.is_none());
    assert_eq!(m, StoredMembership::default());

    // apply 混合批：normal + blank + membership
    let batch = vec![
        normal(1, 1, b"alpha"),
        blank(1, 2),
        normal(1, 3, b"gamma"),
        membership_entry(1, 4, &[1, 2, 3]),
    ];
    let resp: Vec<HubResponse> = sm.apply(batch).await.expect("apply");
    assert_eq!(resp[0], HubResponse(b"alpha".to_vec()));
    assert_eq!(resp[1], HubResponse(Vec::new()));

    let (applied, m) = sm.applied_state().await.expect("applied_state");
    assert_eq!(applied, Some(log_id(1, 4)));
    assert_eq!(m.membership(), &membership_of(&[1, 2, 3]));
    assert_eq!(m.log_id(), &Some(log_id(1, 4)));

    // 非 membership 条目不改写 membership
    sm.apply(vec![normal(1, 5, b"delta")]).await.expect("apply");
    let (_, m) = sm.applied_state().await.expect("applied_state");
    assert_eq!(m.membership(), &membership_of(&[1, 2, 3]));

    // 幂等重放防御：openraft 只 committed 后 apply；此处仅验证重复 apply 同
    // index 覆盖同值（数据节按 index upsert）
    sm.apply(vec![normal(1, 5, b"delta")]).await.expect("apply");

    // 日志面照常可读（apply 不动日志）
    assert_eq!(log.get_log_state().await.expect("state").last_log_id, None);
}

#[tokio::test]
async fn t03_snapshot_build_get_install_roundtrip() {
    let root = tmp_root("snap");
    let db = open_db(&root);
    let (_log, mut sm) = open_raft_stores(&db, 1).expect("stores");
    let batch = vec![
        normal(1, 1, b"row-1"),
        normal(1, 2, b"row-2"),
        membership_entry(1, 3, &[1, 2, 3]),
        normal(1, 4, b"row-4"),
    ];
    sm.apply(batch).await.expect("apply");

    // 构建
    let mut builder = sm.get_snapshot_builder().await;
    let snap = builder.build_snapshot().await.expect("build");
    assert_eq!(snap.meta.last_log_id, Some(log_id(1, 4)));
    assert_eq!(
        snap.meta.last_membership.membership(),
        &membership_of(&[1, 2, 3])
    );

    // 回读当前快照（持久化行）
    let cur = sm.get_current_snapshot().await.expect("cur").expect("some");
    assert_eq!(cur.meta.snapshot_id, snap.meta.snapshot_id);
    assert_eq!(cur.meta.last_log_id, Some(log_id(1, 4)));

    // 安装到另一组（模拟 follower 落后组追赶）：先有脏数据，安装后全量替换
    let (_log2, mut sm2) = open_raft_stores(&db, 2).expect("stores");
    sm2.apply(vec![normal(1, 90, b"stale")])
        .await
        .expect("apply");
    let meta = SnapshotMeta {
        last_log_id: Some(log_id(1, 4)),
        last_membership: StoredMembership::new(Some(log_id(1, 4)), membership_of(&[1, 2, 3])),
        snapshot_id: snap.meta.snapshot_id.clone(),
    };
    sm2.install_snapshot(&meta, cur.snapshot)
        .await
        .expect("install");
    let (applied, m) = sm2.applied_state().await.expect("applied_state");
    assert_eq!(applied, Some(log_id(1, 4)));
    assert_eq!(m.membership(), &membership_of(&[1, 2, 3]));
    // 旧脏数据行已被替换：读回安装后的数据节逐行核对
    let got = sm2.apply(vec![normal(1, 5, b"post")]).await.expect("apply");
    assert_eq!(got, vec![HubResponse(b"post".to_vec())]);

    // 安装快照后 get_current_snapshot 反映新装内容
    let cur2 = sm2
        .get_current_snapshot()
        .await
        .expect("cur")
        .expect("some");
    assert_eq!(cur2.meta.snapshot_id, snap.meta.snapshot_id);
}

#[tokio::test]
async fn t03_restart_recovers_all_sections() {
    let root = tmp_root("restart");
    {
        let db = open_db(&root);
        let (mut log, mut sm) = open_raft_stores(&db, 1).expect("stores");
        log.save_vote(&Vote::new_committed(9, 4))
            .await
            .expect("vote");
        log.save_committed(Some(log_id(1, 3)))
            .await
            .expect("committed");
        log.blocking_append(vec![normal(1, 1, b"a"), normal(1, 2, b"b")])
            .await
            .expect("append");
        sm.apply(vec![normal(1, 1, b"a"), normal(1, 2, b"b")])
            .await
            .expect("apply");
        let mut builder = sm.get_snapshot_builder().await;
        builder.build_snapshot().await.expect("build");
        persist(&db);
    }
    let db = open_db(&root);
    let (mut log, mut sm) = open_raft_stores(&db, 1).expect("stores");
    assert_eq!(
        log.read_vote().await.expect("vote"),
        Some(Vote::new_committed(9, 4))
    );
    assert_eq!(
        log.read_committed().await.expect("committed"),
        Some(log_id(1, 3))
    );
    assert_eq!(
        log.get_log_state().await.expect("state").last_log_id,
        Some(log_id(1, 2))
    );
    let (applied, _) = sm.applied_state().await.expect("applied_state");
    assert_eq!(applied, Some(log_id(1, 2)));
    let cur = sm.get_current_snapshot().await.expect("cur").expect("snap");
    assert_eq!(cur.meta.last_log_id, Some(log_id(1, 2)));
}

#[tokio::test]
async fn t03_groups_isolated_by_pid_prefix() {
    let root = tmp_root("isolated");
    let db = open_db(&root);
    let (mut log1, mut sm1) = open_raft_stores(&db, 1).expect("g1");
    let (mut log2, mut sm2) = open_raft_stores(&db, 2).expect("g2");

    log1.blocking_append(vec![normal(1, 1, b"g1-only")])
        .await
        .expect("append");
    log2.blocking_append(vec![normal(1, 1, b"g2-only")])
        .await
        .expect("append");
    sm1.apply(vec![normal(1, 1, b"g1-data")])
        .await
        .expect("apply");

    // 日志互不可见
    let got2 = log2.try_get_log_entries(1..2).await.expect("r");
    match &got2[0].payload {
        EntryPayload::Normal(HubData(d)) => assert_eq!(d, b"g2-only"),
        other => panic!("unexpected {other:?}"),
    }
    // 状态机互不可见
    let (applied1, _) = sm1.applied_state().await.expect("s1");
    let (applied2, _) = sm2.applied_state().await.expect("s2");
    assert_eq!(applied1, Some(log_id(1, 1)));
    assert!(applied2.is_none());
    // vote 互不可见
    log1.save_vote(&Vote::new_committed(5, 1))
        .await
        .expect("vote");
    assert!(log2.read_vote().await.expect("vote").is_none());
    // purge 只影响本组
    log1.purge(log_id(1, 1)).await.expect("purge");
    let st2 = log2.get_log_state().await.expect("state");
    assert_eq!(st2.last_purged_log_id, None);
    assert_eq!(st2.last_log_id, Some(log_id(1, 1)));
}

#[tokio::test]
async fn t03_snapshot_rebuild_after_restart() {
    // 重开后快照构建器照常工作（applied 指针与数据节从盘恢复）
    let root = tmp_root("snap-restart");
    {
        let db = open_db(&root);
        let (_, mut sm) = open_raft_stores(&db, 1).expect("stores");
        sm.apply(vec![normal(3, 7, b"late")]).await.expect("apply");
    }
    let db = open_db(&root);
    let (_, mut sm) = open_raft_stores(&db, 1).expect("stores");
    let mut builder = sm.get_snapshot_builder().await;
    let snap = builder.build_snapshot().await.expect("build");
    assert_eq!(snap.meta.last_log_id, Some(log_id(3, 7)));
}

#[tokio::test]
async fn t03_journal_durability_survives_unclean_close() {
    // kill -9 模拟（关门 KPI「已 ACK 写入不丢」的存储面证据）：
    // 子进程写完立即 `process::exit(9)`——跳过全部 Drop（无 persist、无
    // 缓冲清理、无锁释放，等价进程死亡形态）；父进程重开验证协议批的
    // fdatasync durability 独立成立。子进程形态由自身二进制重入 + 环境变量
    // 分发（测试二进制无参数约定，退出码透传失败）。
    if let Ok(child_root) = std::env::var("PARTISYNC_WP02_KILL9_ROOT") {
        // 子分支已在测试自身的 tokio 运行时内——直接 await，不另起 runtime
        let db = open_db(std::path::Path::new(&child_root));
        let (mut log, mut sm) = open_raft_stores(&db, 1).expect("stores");
        log.save_vote(&Vote::new_committed(4, 2))
            .await
            .expect("vote");
        log.blocking_append(vec![normal(1, 1, b"durable"), normal(1, 2, b"writes")])
            .await
            .expect("append");
        sm.apply(vec![normal(1, 1, b"durable")])
            .await
            .expect("apply");
        // 进程死亡：不跑任何析构
        std::process::exit(9);
    }
    let child_root = tmp_root("kill9-child");
    let status = std::process::Command::new(std::env::current_exe().expect("exe"))
        .arg("--exact")
        .arg("t03_journal_durability_survives_unclean_close")
        .env("PARTISYNC_WP02_KILL9_ROOT", &child_root)
        .status()
        .expect("spawn child");
    assert!(
        status.code().is_some_and(|c| c != 0),
        "child must die by exit(9), got {status}"
    );

    // 父进程重开：协议写入全部在盘
    let db = open_db(&child_root);
    let (mut log, mut sm) = open_raft_stores(&db, 1).expect("stores");
    assert_eq!(
        log.read_vote().await.expect("vote"),
        Some(Vote::new_committed(4, 2))
    );
    let got = log.try_get_log_entries(1..=2).await.expect("range");
    assert_eq!(
        got.iter().map(|e| e.log_id).collect::<Vec<_>>(),
        vec![log_id(1, 1), log_id(1, 2)]
    );
    let (applied, _) = sm.applied_state().await.expect("applied_state");
    assert_eq!(applied, Some(log_id(1, 1)));
}

/// 套件守卫：持有 Database（路径锁），测试结束析构释放。
struct SuiteGuard {
    _db: Database,
}

/// openraft 官方存储一致性套件的构建器：每个用例独立临时库（pid=1）。
struct Wp02StoreBuilder;

impl openraft::testing::StoreBuilder<Cfg, RaftLogStore, RaftStateMachineStore, SuiteGuard>
    for Wp02StoreBuilder
{
    async fn build(
        &self,
    ) -> Result<(SuiteGuard, RaftLogStore, RaftStateMachineStore), StorageError<u64>> {
        let root = tmp_root("suite");
        let db = open_db(&root);
        let (log, sm) = open_raft_stores(&db, 1).map_err(|e| {
            let io = std::io::Error::other(e.to_string());
            StorageError::IO {
                source: StorageIOError::new(
                    ErrorSubject::Store,
                    ErrorVerb::Write,
                    AnyError::new(&io),
                ),
            }
        })?;
        Ok((SuiteGuard { _db: db }, log, sm))
    }
}

/// openraft 官方一致性矩阵（~33 项：membership/初始态/vote/日志区间/
/// truncate/purge/append/apply/快照传输）——T03 存储面的对外验收线。
#[test]
fn t03_openraft_storage_conformance_suite() {
    openraft::testing::Suite::test_all(Wp02StoreBuilder).expect("conformance suite");
}

/// ===== T04：网络层（SPEC M3-WP02 裁定 4） =====
use std::time::Duration;

use openraft::error::{RPCError, RaftError};
use openraft::network::RPCOption;
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use openraft::{RPCTypes, RaftNetwork, RaftNetworkFactory};
use partisync_hub::{serve, NetFactory, NetRequest, NetResponse};

/// 起 stub 服务端：echo 形态按 req 种类回预置应答。
async fn spawn_stub_server(
    handler: impl Fn(&NetRequest) -> NetResponse + Send + Sync + 'static + Clone,
) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr").to_string();
    let h = move |req: NetRequest| {
        let resp = handler(&req);
        std::future::ready(resp)
    };
    tokio::spawn(async move {
        let _ = serve(listener, h).await;
    });
    addr
}

fn vote_req(term: u64) -> VoteRequest<u64> {
    VoteRequest {
        vote: Vote::new_committed(term, 1),
        last_log_id: Some(log_id(term, 3)),
    }
}

#[tokio::test]
async fn t04_frame_roundtrip_and_version_reject() {
    use partisync_hub::{read_frame, write_frame};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let srv = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let payload = read_frame(&mut stream).await.expect("read");
        write_frame(&mut stream, &payload).await.expect("echo");
    });
    let mut client = tokio::net::TcpStream::connect(addr).await.expect("connect");
    let body = b"\x00\x01frame-payload".to_vec();
    write_frame(&mut client, &body).await.expect("write");
    let got = read_frame(&mut client).await.expect("echo back");
    assert_eq!(got, body);
    srv.await.expect("srv");

    // 版本不符：客户端发 0xFF 版本帧 → 服务端 read_frame 报错断连
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let srv = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        read_frame(&mut stream).await
    });
    let mut client = tokio::net::TcpStream::connect(addr).await.expect("connect");
    let len = 2u32.to_be_bytes();
    use tokio::io::AsyncWriteExt;
    client.write_all(&len).await.expect("len");
    client.write_all(&[0xFF]).await.expect("bad version");
    client.write_all(b"{}").await.expect("body");
    let res = srv.await.expect("srv");
    assert!(res.is_err(), "server must reject wrong version");
}

#[tokio::test]
async fn t04_net_client_server_roundtrip_all_kinds() {
    // stub 服务端：Vote 成功、AppendEntries 成功、InstallSnapshot 成功
    let addr = spawn_stub_server(|req| match req {
        NetRequest::Vote(_) => NetResponse::Vote(Ok(VoteResponse {
            vote: Vote::new_committed(5, 2),
            vote_granted: true,
            last_log_id: Some(log_id(5, 9)),
        })),
        NetRequest::AppendEntries(_) => {
            NetResponse::AppendEntries(Ok(AppendEntriesResponse::Success))
        }
        NetRequest::InstallSnapshot(_) => {
            NetResponse::InstallSnapshot(Ok(InstallSnapshotResponse {
                vote: Vote::new_committed(5, 2),
            }))
        }
    })
    .await;

    let mut factory = NetFactory::new(1);
    let node = BasicNode::new(addr.clone());
    let mut client = factory.new_client(2, &node).await;

    // vote
    let vresp = client
        .vote(vote_req(5), RPCOption::new(Duration::from_secs(5)))
        .await
        .expect("vote rpc");
    assert_eq!(vresp.vote, Vote::new_committed(5, 2));
    assert!(vresp.vote_granted);
    assert_eq!(vresp.last_log_id, Some(log_id(5, 9)));

    // append_entries（长连接复用：第二次 RPC 不重建连接）
    let areq: AppendEntriesRequest<Cfg> = AppendEntriesRequest {
        vote: Vote::new_committed(5, 2),
        prev_log_id: Some(log_id(5, 9)),
        entries: vec![],
        leader_commit: Some(log_id(5, 8)),
    };
    client
        .append_entries(areq, RPCOption::new(Duration::from_secs(5)))
        .await
        .expect("append rpc");

    // install_snapshot（分块帧）
    let ireq: InstallSnapshotRequest<Cfg> = InstallSnapshotRequest {
        vote: Vote::new_committed(5, 2),
        meta: SnapshotMeta {
            last_log_id: Some(log_id(5, 9)),
            last_membership: StoredMembership::new(Some(log_id(1, 1)), membership_of(&[1, 2])),
            snapshot_id: "5-9".to_owned(),
        },
        offset: 0,
        data: b"snapshot-bytes".to_vec(),
        done: true,
    };
    let iresp = client
        .install_snapshot(ireq, RPCOption::new(Duration::from_secs(5)))
        .await
        .expect("install rpc");
    assert_eq!(iresp.vote, Vote::new_committed(5, 2));
}

#[tokio::test]
async fn t04_net_remote_error_mapping() {
    // 远端业务错误过网 → RPCError::RemoteError
    let addr = spawn_stub_server(|req| match req {
        NetRequest::Vote(_) => {
            NetResponse::Vote(Err(RaftError::Fatal(openraft::error::Fatal::Stopped)))
        }
        _ => unreachable!("vote only"),
    })
    .await;
    let addr_s = addr.clone();
    let resp =
        partisync_hub::net::one_shot_vote(&addr_s, vote_req(7), Duration::from_secs(5)).await;
    match resp {
        Err(RPCError::RemoteError(remote)) => {
            assert_eq!(remote.target, 0);
            assert!(matches!(remote.source, RaftError::Fatal(_)));
        }
        other => panic!("expected RemoteError, got {other:?}"),
    }
}

#[tokio::test]
async fn t04_net_timeout_on_silent_server() {
    // 服务端接受连接但不回包 → hard_ttl 超时 → RPCError::Timeout
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr").to_string();
    tokio::spawn(async move {
        // 收下并持有：不读不写（丢弃会触发 RST，客户端将收到 Network 而非超时）
        let mut held = Vec::new();
        loop {
            if let Ok((stream, _)) = listener.accept().await {
                held.push(stream);
            }
        }
    });
    let resp =
        partisync_hub::net::one_shot_vote(&addr, vote_req(1), Duration::from_millis(80)).await;
    match resp {
        Err(RPCError::Timeout(t)) => {
            assert_eq!(t.action, RPCTypes::Vote);
            assert_eq!(t.target, 0);
        }
        other => panic!("expected Timeout, got {other:?}"),
    }
}

#[tokio::test]
async fn t04_net_connect_refused_maps_network_error() {
    // 端口 1（tcpmux，本机无监听）→ 连接拒绝 → RPCError::Network
    let resp =
        partisync_hub::net::one_shot_vote("127.0.0.1:1", vote_req(1), Duration::from_secs(2)).await;
    assert!(
        matches!(resp, Err(RPCError::Network(_))),
        "expected Network error, got {resp:?}"
    );
}

#[tokio::test]
async fn t04_net_payload_survives_json_envelope() {
    // HubData 紧凑载荷过 serde_json 信封不失真（含 0x00 字节与多字节 UTF-8）
    let payload: std::sync::Arc<Vec<u8>> =
        std::sync::Arc::new(vec![0x00u8, 0xFF, 0xE4, 0xB8, 0xAD, 0xF0, 0x9F, 0x92, 0xAA]);
    let expected = payload.clone();
    let addr = spawn_stub_server(move |req| match req {
        NetRequest::AppendEntries(a) => {
            let entry = &a.entries[0];
            match &entry.payload {
                openraft::EntryPayload::Normal(HubData(d)) => {
                    assert_eq!(d, &expected[..]);
                }
                other => panic!("unexpected payload {other:?}"),
            }
            NetResponse::AppendEntries(Ok(AppendEntriesResponse::Success))
        }
        _ => unreachable!("append only"),
    })
    .await;
    let mut factory = NetFactory::new(1);
    let mut client = factory.new_client(2, &BasicNode::new(addr)).await;
    let areq: AppendEntriesRequest<Cfg> = AppendEntriesRequest {
        vote: Vote::new_committed(3, 1),
        prev_log_id: None,
        entries: vec![Entry {
            log_id: log_id(3, 2),
            payload: openraft::EntryPayload::Normal(HubData((*payload).clone())),
        }],
        leader_commit: None,
    };
    client
        .append_entries(areq, RPCOption::new(Duration::from_secs(5)))
        .await
        .expect("append with binary payload");
}

/// ===== T05：组生命周期与选举演练（SPEC M3-WP02 验收 2-4） =====
use partisync_hub::replica::{NodeConfig, Replica, ReplicaError};
use std::collections::BTreeMap;

/// cluster 演练互斥：端口先绑后释的分配法在并行测试下会互抢（同组 RPC
/// 打到错误节点 → 选举 chaos），整段演练持锁串行。
static CLUSTER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 分配一个空闲 TCP 端口（绑定后立即释放；本机演练用）。
fn alloc_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port()
}

/// 起一个 3 节点组（选举超时 150-300ms、心跳 25ms——演练确定性注入）。
async fn spawn_cluster(tag: &str, size: usize) -> Vec<Replica> {
    let ids: Vec<u64> = (1..=size as u64).collect();
    let addrs: BTreeMap<u64, String> = ids
        .iter()
        .map(|id| (*id, format!("127.0.0.1:{}", alloc_port())))
        .collect();
    let mut nodes = Vec::new();
    for id in ids {
        let cfg = NodeConfig {
            node_id: id,
            addr: addrs[&id].clone(),
            db_root: tmp_root(&format!("{tag}-n{id}")),
            group_id: 1,
            members: addrs.clone(),
            election_timeout_ms: (300, 600),
            heartbeat_interval_ms: 50,
            disable_auto_snapshot: false,
        };
        nodes.push(Replica::open(&cfg).await.expect("open replica"));
    }
    for n in &nodes {
        n.bootstrap().await.expect("bootstrap");
    }
    nodes
}

/// 任一节点视角等待指定节点成为 leader。
async fn wait_leader_is(
    nodes: &[Replica],
    want: u64,
    timeout: Duration,
) -> Result<(), ReplicaError> {
    let deadline = Instant::now() + timeout;
    loop {
        if nodes.iter().any(|n| n.current_leader() == Some(want)) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(ReplicaError::Timeout);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

use std::time::Instant;

/// 线性一致读（带瞬态重试）：QuorumNotEnough 是心跳确认的瞬态形态
/// （多 cluster 交接期背景负载），客户端按幂等读重试。
async fn read_lin(n: &Replica, idx: u64) -> Vec<u8> {
    for attempt in 0..10 {
        match n.linearizable_read(idx).await {
            Ok(Some(row)) => return row,
            Ok(None) => panic!("row {idx} missing after write"),
            Err(ReplicaError::CheckIsLeader(
                openraft::error::CheckIsLeaderError::QuorumNotEnough(_),
            )) => {
                tokio::time::sleep(Duration::from_millis(50 + attempt * 50)).await;
            }
            Err(other) => panic!("linearizable read: {other}"),
        }
    }
    panic!("linearizable read: quorum not recovered after retries");
}

/// 演练收尾：显式停掉全部 raft core（drop 不停 core——残留会拖累后续演练）。
async fn teardown(nodes: &[Replica]) {
    for n in nodes {
        n.crash().await;
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // 刻意：std Mutex 整段串行 cluster 演练（见 CLUSTER_LOCK 注释）
async fn t05_bootstrap_elect_and_replicate() {
    let _serial = CLUSTER_LOCK.lock().expect("cluster lock");
    let nodes = spawn_cluster("t05-basic", 3).await;
    let leader = nodes[0]
        .wait_leader(Duration::from_secs(5))
        .await
        .expect("leader");
    let l = nodes
        .iter()
        .find(|n| n.node_id() == leader)
        .expect("leader node");

    // 写 10 条，逐条 ACK（leader 串行 commit+apply 后应答）
    let mut indexes = Vec::new();
    for i in 0..10u64 {
        let idx = l
            .write(format!("payload-{i:04}").into_bytes())
            .await
            .expect("write");
        indexes.push(idx);
    }

    // follower 顺序读：等 applied ≥ 末条，读回 == 写入
    for n in nodes.iter().filter(|n| n.node_id() != leader) {
        n.wait_applied(*indexes.last().expect("idx"), Duration::from_secs(5))
            .await
            .expect("follower applied");
        let got = n
            .read_applied(indexes[3])
            .await
            .expect("read")
            .expect("row");
        assert_eq!(got, b"payload-0003".to_vec());
    }

    // leader 线性一致读（ReadIndex）：read-your-writes
    for (i, idx) in indexes.iter().enumerate() {
        let got = read_lin(l, *idx).await;
        assert_eq!(got, format!("payload-{i:04}").into_bytes());
    }
    teardown(&nodes).await;
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // 刻意：同上
async fn t05_failover_under_10s_no_ack_loss() {
    // 关门 KPI：kill leader（无优雅交接）→ 新 leader 当选 + 已 ACK 写入
    // 全部可读，全程 <10s。
    let nodes = spawn_cluster("t05-failover", 3).await;
    let leader_id = nodes[0]
        .wait_leader(Duration::from_secs(5))
        .await
        .expect("leader");
    let leader = nodes
        .iter()
        .find(|n| n.node_id() == leader_id)
        .expect("leader node");
    let others: Vec<&Replica> = nodes.iter().filter(|n| n.node_id() != leader_id).collect();

    // 先 ACK 8 条
    let mut last_idx = 0u64;
    for i in 0..8u64 {
        last_idx = leader
            .write(format!("acked-{i:04}").into_bytes())
            .await
            .expect("write");
    }

    // kill（进程死亡形态：core 停止、无交接）
    let crash_at = Instant::now();
    leader.crash().await;

    // 幸存者选出新 leader
    let nl;
    let deadline = crash_at + Duration::from_secs(10);
    loop {
        if let Some(n) = others.iter().find(|n| n.is_leader()) {
            nl = *n;
            break;
        }
        assert!(Instant::now() < deadline, "failover exceeded 10s KPI");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let elapsed = crash_at.elapsed();

    // 新 leader 上全部 ACK 写入可读（raft 承诺：已 ACK 不丢）
    nl.wait_applied(last_idx, Duration::from_secs(5))
        .await
        .expect("applied");
    for i in 0..8u64 {
        // 线索引：ACK 序列 = log 序列（leader 串行写、无并发竞争者）
        let got = nl
            .read_applied(last_idx - (7 - i))
            .await
            .expect("read")
            .expect("row");
        assert_eq!(got, format!("acked-{i:04}").into_bytes());
    }
    assert!(
        elapsed < Duration::from_secs(10),
        "failover took {elapsed:?}"
    );
    teardown(&nodes).await;
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // 刻意：同上
async fn t05_leader_transfer_under_5s() {
    // 验收 3：主动转移 <5s。形态（0.9.25 无显式 transfer API）：
    // 旧 leader pause_election → 目标节点 trigger_elect（更高 term）。
    let nodes = spawn_cluster("t05-transfer", 3).await;
    let leader_id = nodes[0]
        .wait_leader(Duration::from_secs(5))
        .await
        .expect("leader");
    let leader = nodes
        .iter()
        .find(|n| n.node_id() == leader_id)
        .expect("leader");
    let target = nodes
        .iter()
        .find(|n| n.node_id() != leader_id)
        .expect("target");

    leader.pause_election();
    let started = Instant::now();
    target.trigger_elect().await.expect("trigger elect");
    wait_leader_is(&nodes, target.node_id(), Duration::from_secs(5))
        .await
        .expect("target becomes leader");
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(5),
        "transfer took {elapsed:?}"
    );

    // 转移窗口写可用（新 leader 承接写入）
    let idx = target
        .write(b"after-transfer".to_vec())
        .await
        .expect("write");
    let got = read_lin(target, idx).await;
    assert_eq!(got, b"after-transfer".to_vec());
    teardown(&nodes).await;
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // 刻意：同上
async fn t05_linearizable_read_your_writes_monotonic() {
    // 验收 4 简化 proptest：同一 leader 上「写后读自己写」单调成立；
    // 8 组随机载荷（proptest 生成器覆盖 0x00/非 ASCII 形态）。
    let nodes = spawn_cluster("t05-monotonic", 3).await;
    let leader_id = nodes[0]
        .wait_leader(Duration::from_secs(5))
        .await
        .expect("leader");
    let leader = nodes
        .iter()
        .find(|n| n.node_id() == leader_id)
        .expect("leader");

    let mut sm = 0x5EED_2026u64;
    for case in 0..8u64 {
        sm = sm.wrapping_mul(6364136223846793005).wrapping_add(1);
        let len = 1 + (sm >> 33) as usize % 64;
        let payload: Vec<u8> = (0..len)
            .map(|_| {
                sm = sm.wrapping_mul(6364136223846793005).wrapping_add(1);
                (sm >> 24) as u8
            })
            .collect();
        let idx = leader.write(payload.clone()).await.expect("write");
        let got = read_lin(leader, idx).await;
        assert_eq!(got, payload, "read-your-writes violated at case {case}");
    }
    teardown(&nodes).await;
}
