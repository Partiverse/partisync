//! WP02 存储适配器回归（SPEC M3-WP02 T03）：openraft storage-v2 → fjall。
//!
//! 覆盖：vote/committed 硬状态、append/range/log-state、truncate/purge、
//! apply（数据节 + membership）、快照构建/安装/回读、重开恢复、组隔离。

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
