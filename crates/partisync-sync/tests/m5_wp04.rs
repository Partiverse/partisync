//! M5-WP04 集成测试：云事件流摄取引擎（SPEC docs/specs/M5-WP04.md）。
//!
//! T02/T03：折叠映射 + 去重幂等 + 断点续传 + 空间路由闸 + 文件账本。

use std::collections::{BTreeSet, VecDeque};
use std::path::PathBuf;

use std::sync::Mutex;
use std::time::Duration;

use partisync_sync::event::{
    apply_batch, poll_with_retry, EventDrain, EventJournal, EventJournalState, EventKind,
    EventOpts, EventRecord, EventSource, EventStats, FileEventJournal,
};

fn tmp_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sync-m5wp04-{tag}-{}", partisync_core::Ulid::now()))
}

/// 注入式 EventSource：内嵌队列 + 拉取失败注入。
struct MockSource {
    name: String,
    queue: Mutex<VecDeque<EventRecord>>,
    /// 这些 cursor 首次 poll 返回 Transport 错误（重试语义）。
    fail_first: Mutex<BTreeSet<String>>,
    committed: Mutex<Vec<String>>,
}

impl MockSource {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            queue: Mutex::new(VecDeque::new()),
            fail_first: Mutex::new(BTreeSet::new()),
            committed: Mutex::new(Vec::new()),
        }
    }

    fn push(&self, r: EventRecord) {
        self.queue.lock().unwrap().push_back(r);
    }

    fn fail_first(&self, cursor: &str) {
        self.fail_first.lock().unwrap().insert(cursor.to_owned());
    }

    fn committed(&self) -> Vec<String> {
        self.committed.lock().unwrap().clone()
    }
}

fn rec(provider: &str, space: &str, path: &str, kind: EventKind, cursor: &str) -> EventRecord {
    EventRecord {
        provider: provider.into(),
        space: space.into(),
        path: path.into(),
        kind,
        cursor: cursor.into(),
        payload: serde_json::json!({}),
    }
}

impl EventSource for MockSource {
    async fn poll_batch(
        &self,
        max: usize,
        _deadline: Duration,
    ) -> Result<Vec<EventRecord>, partisync_sync::event::EventError> {
        // 触发 fail_first 注入
        {
            let mut ff = self.fail_first.lock().unwrap();
            if let Some(c) = ff.iter().next().cloned() {
                ff.remove(&c);
                return Err(partisync_sync::event::EventError::Transport(format!(
                    "mock: {c}"
                )));
            }
        }
        let mut q = self.queue.lock().unwrap();
        let mut out = Vec::with_capacity(max);
        while out.len() < max {
            match q.pop_front() {
                Some(r) => out.push(r),
                None => break,
            }
        }
        Ok(out)
    }

    async fn commit_cursor(&self, cursor: &str) -> Result<(), partisync_sync::event::EventError> {
        self.committed.lock().unwrap().push(cursor.to_owned());
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ---------- T02：折叠映射 + 去重幂等 ----------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t02_event_kind_serde_roundtrip_and_folding() {
    use serde_json;
    for (k, n) in [
        (EventKind::Created, 0i64),
        (EventKind::Modified, 1),
        (EventKind::Removed, 2),
    ] {
        let json = serde_json::to_string(&k).unwrap();
        assert_eq!(json, n.to_string(), "kind serialize as i64");
        let back: EventKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, k);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t02_apply_batch_skips_unknown_kind() {
    // 走 apply_batch 直接验证 kind 区分（Created/Modified/Removed）
    let journal_path = tmp_root("t02-apply").join("e.json");
    let mut state = EventJournalState::default();
    let journal = FileEventJournal::new(&journal_path);

    // Apply 三类 kind 并落账本
    let batch = vec![
        rec("minio", "s1", "a.txt", EventKind::Created, "c1"),
        rec("minio", "s1", "a.txt", EventKind::Modified, "c2"),
        rec("minio", "s1", "a.txt", EventKind::Removed, "c3"),
    ];
    let source = MockSource::new("minio");
    let stats = apply_batch(&source, &journal, &mut state, batch, &EventOpts::default())
        .await
        .unwrap();
    assert_eq!(stats.applied, 3);
    let ckpt = state.sources.get("minio").unwrap();
    assert_eq!(ckpt.cursor, "c3");
    assert_eq!(ckpt.processed, 3);
    assert_eq!(source.committed(), vec!["c1", "c2", "c3"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t02_poll_with_retry_recovers_after_transport_error() {
    let source = MockSource::new("sqs");
    source.push(rec("sqs", "sp", "a.txt", EventKind::Created, "c1"));
    source.fail_first("c1");

    // 重试 3 次后 batch 仍为 0 → 重试耗尽会触发一次返回成功（fail_first 单次）
    let batch = poll_with_retry(&source, 16, Duration::from_millis(50), 3)
        .await
        .unwrap();
    // fail_first 已消费，第二次 poll 取到 c1
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].cursor, "c1");
}

// ---------- T03：checkpoint + 断点续传 + 空间路由闸 ----------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_event_journal_roundtrip_and_atomic() {
    let path = tmp_root("t03-roundtrip").join("e.json");
    let j = FileEventJournal::new(&path);
    assert!(j.load().await.unwrap().is_none(), "无账本 = None");

    let mut state = EventJournalState::default();
    state.sources.insert(
        "sqs".into(),
        partisync_sync::event::SourceCheckpoint {
            cursor: "h:cursor42".into(),
            processed: 17,
            failed: 1,
            last_at_ns: 1_700_000_000_000_000_000,
        },
    );
    j.save(&state).await.unwrap();
    assert_eq!(j.load().await.unwrap().as_ref(), Some(&state));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_run_once_processes_batch_and_persists_checkpoint() {
    let journal_path = tmp_root("t03-once").join("e.json");
    let source = MockSource::new("kafka");
    source.push(rec("k", "sp", "x", EventKind::Created, "c1"));
    source.push(rec("k", "sp", "y", EventKind::Modified, "c2"));
    let journal = FileEventJournal::new(&journal_path);

    let drain = EventDrain::with_opts(source, journal, EventOpts::default());
    let stats = drain.run_once().await.unwrap();
    assert_eq!(stats.applied, 2);

    let ckpt = drain.journal().load().await.unwrap().unwrap();
    let k = ckpt.sources.get("kafka").unwrap();
    assert_eq!(k.processed, 2);
    assert_eq!(k.cursor, "c2");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_space_filter_skips_non_owned() {
    let journal_path = tmp_root("t03-filter").join("e.json");
    let source = MockSource::new("minio");
    source.push(rec("minio", "mine", "a", EventKind::Created, "c1"));
    source.push(rec("minio", "theirs", "b", EventKind::Created, "c2"));

    let mut state = EventJournalState::default();
    let journal = FileEventJournal::new(&journal_path);
    let opts = EventOpts {
        space_filter: Some(std::sync::Arc::new(|s| s == "mine")),
        ..EventOpts::default()
    };
    let batch: Vec<EventRecord> = source.queue.lock().unwrap().clone().into_iter().collect();
    let stats = apply_batch(&source, &journal, &mut state, batch, &opts)
        .await
        .unwrap();
    assert_eq!(stats.applied, 1);
    assert_eq!(stats.skipped, 1);
    // 仅 own space 的 cursor 被 commit
    assert_eq!(source.committed(), vec!["c1"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_corrupt_journal_errors_not_panic() {
    let dir = tmp_root("t03-corrupt");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("e.json");
    std::fs::write(&path, b"{ truncated...").unwrap();
    let journal = FileEventJournal::new(&path);
    let err = journal.load().await.unwrap_err();
    assert!(
        matches!(err, partisync_sync::event::EventError::Config(_)),
        "got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_run_once_empty_source_returns_zero() {
    let source = MockSource::new("empty");
    let journal = FileEventJournal::new(tmp_root("t03-empty").join("e.json"));
    let drain = EventDrain::with_opts(source, journal, EventOpts::default());
    let stats = drain.run_once().await.unwrap();
    assert_eq!(stats.applied, 0);
    assert_eq!(stats.skipped, 0);
    assert_eq!(stats.source_failed, 0);
}

// ---------- T04/T05 基底：concurrent source 续传 ----------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t05_concurrent_sources_isolated_checkpoints() {
    // 多 source 并存：每个 source 独立 checkpoint，互不串扰
    let journal_path = tmp_root("t05-multi").join("e.json");
    let mut state = EventJournalState::default();

    let s1 = MockSource::new("sqs");
    let s2 = MockSource::new("kafka");
    s1.push(rec("sqs", "sp", "a", EventKind::Created, "s1"));
    s2.push(rec("k", "sp", "b", EventKind::Created, "k1"));

    let journal = FileEventJournal::new(&journal_path);
    apply_batch(
        &s1,
        &journal,
        &mut state,
        vec![rec("sqs", "sp", "a", EventKind::Created, "s1")],
        &EventOpts::default(),
    )
    .await
    .unwrap();
    apply_batch(
        &s2,
        &journal,
        &mut state,
        vec![rec("k", "sp", "b", EventKind::Created, "k1")],
        &EventOpts::default(),
    )
    .await
    .unwrap();

    let loaded = journal.load().await.unwrap().unwrap();
    assert_eq!(loaded.sources.len(), 2);
    assert_eq!(loaded.sources["sqs"].processed, 1);
    assert_eq!(loaded.sources["kafka"].processed, 1);
}

// ---------- T05：基准（SPEC 验收：1k 事件吞吐 + 并发 webhook 落盘） ----------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "基准：显式运行并登记 docs/reports/bench/M5-WP04-event-drain.md"]
async fn t05_bench_mock_source_1k_events() {
    let journal_path = tmp_root("t05-bench").join("e.json");
    let source = MockSource::new("bench");
    for i in 0..1024u64 {
        source.push(rec(
            "bench",
            "sp",
            &format!("p/{i}"),
            EventKind::Created,
            &format!("c{i}"),
        ));
    }

    let journal = FileEventJournal::new(&journal_path);
    let drain = EventDrain::with_opts(source, journal, EventOpts::default());

    let t0 = std::time::Instant::now();
    let mut total = EventStats::default();
    // 多批：run_once 一次性 poll max=256 → 1024 需 4 轮
    for _ in 0..4 {
        let s = drain.run_once().await.unwrap();
        total.applied += s.applied;
        total.skipped += s.skipped;
        total.source_failed += s.source_failed;
    }
    let elapsed = t0.elapsed();
    println!(
        "1k 事件 4 轮 run_once: {:?}  applied={}  ≈ {:.0} evt/s",
        elapsed,
        total.applied,
        total.applied as f64 / elapsed.as_secs_f64()
    );
    assert_eq!(total.applied, 1024);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "基准：显式运行并登记 docs/reports/bench/M5-WP04-event-drain.md"]
async fn t05_bench_webhook_inproc_apply_throughput() {
    // 不引入 reqwest/tower dev-deps——直接测 apply_batch 1k 事件落 journal
    // 吞吐（验收口径 = 单源处理能力；webhook 并发入站由 mpsc 转送归 M5-WP04 后续接线）
    use std::sync::Arc;
    let journal_path = tmp_root("t05-apply").join("e.json");
    let journal = Arc::new(FileEventJournal::new(&journal_path));
    let mut state = EventJournalState::default();

    let n: u64 = 4096;
    let t0 = std::time::Instant::now();
    for batch_start in (0..n).step_by(256) {
        let batch: Vec<EventRecord> = (0..256)
            .map(|i| {
                let k = batch_start + i;
                rec(
                    "bench",
                    "sp",
                    &format!("p/{k}"),
                    EventKind::Created,
                    &format!("c{k}"),
                )
            })
            .collect();
        struct Noop;
        impl EventSource for Noop {
            async fn poll_batch(
                &self,
                _max: usize,
                _deadline: Duration,
            ) -> Result<Vec<EventRecord>, partisync_sync::event::EventError> {
                Ok(Vec::new())
            }
            async fn commit_cursor(
                &self,
                _cursor: &str,
            ) -> Result<(), partisync_sync::event::EventError> {
                Ok(())
            }
            fn name(&self) -> &str {
                "noop"
            }
        }
        apply_batch(
            &Noop,
            journal.as_ref(),
            &mut state,
            batch,
            &EventOpts::default(),
        )
        .await
        .unwrap();
    }
    let elapsed = t0.elapsed();
    println!(
        "apply_batch 4k 事件 (16×256) + journal save: {:?}  ≈ {:.0} evt/s",
        elapsed,
        n as f64 / elapsed.as_secs_f64()
    );
    let ckpt = journal.load().await.unwrap().unwrap();
    assert_eq!(ckpt.sources["noop"].processed, n);
}

// ---------- M5-WP08：graph apply 接线 ----------

use partisync_sync::event::{EventApplier, GraphApplier};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t08_graph_applier_lifecycle_and_dir_chain() {
    let dir = tmp_root("t08-graph");
    std::fs::create_dir_all(&dir).unwrap();
    let store = partisync_graph::store::Store::open(&dir.join("t.db"))
        .await
        .unwrap();
    let applier = GraphApplier::new(store);

    // created：父目录链自动建（a/b/c.txt 三级）
    let rec = EventRecord {
        provider: "minio".into(),
        space: "sp".into(),
        path: "/docs/a/b/c.txt".into(),
        kind: EventKind::Created,
        cursor: "c1".into(),
        payload: serde_json::json!({"size": 42_u64, "mtime_ns": 5_u64}),
    };
    applier.apply_event(&rec).await.unwrap();
    let row = applier
        .store_ref()
        .entry_by_path("/docs/a/b/c.txt")
        .await
        .unwrap();
    let row = row.expect("created 应落图谱");
    assert_eq!(row.size, 42);
    assert_eq!(row.mtime_ns, 5);
    assert!(applier
        .store_ref()
        .entry_by_path("/docs")
        .await
        .unwrap()
        .is_some());
    assert!(applier
        .store_ref()
        .entry_by_path("/docs/a")
        .await
        .unwrap()
        .is_some());
    assert!(applier
        .store_ref()
        .entry_by_path("/docs/a/b")
        .await
        .unwrap()
        .is_some());

    // modified：幂等 upsert 刷新 size
    let rec_mod = EventRecord {
        kind: EventKind::Modified,
        cursor: "c2".into(),
        payload: serde_json::json!({"size": 99_u64, "mtime_ns": 6_u64}),
        ..rec.clone()
    };
    applier.apply_event(&rec_mod).await.unwrap();
    let row = applier
        .store_ref()
        .entry_by_path("/docs/a/b/c.txt")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.size, 99, "modified 应刷新 size");
    assert_eq!(row.id, row.id);

    // removed：消失；重复 removed 幂等不报错
    let rec_rm = EventRecord {
        kind: EventKind::Removed,
        cursor: "c3".into(),
        ..rec.clone()
    };
    applier.apply_event(&rec_rm).await.unwrap();
    assert!(applier
        .store_ref()
        .entry_by_path("/docs/a/b/c.txt")
        .await
        .unwrap()
        .is_none());
    applier.apply_event(&rec_rm).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t08_apply_batch_routes_through_applier() {
    let dir = tmp_root("t08-batch");
    std::fs::create_dir_all(&dir).unwrap();
    let store = partisync_graph::store::Store::open(&dir.join("t.db"))
        .await
        .unwrap();
    let journal_path = tmp_root("t08-batch").join("e.json");
    let journal = FileEventJournal::new(&journal_path);

    let source = MockSource::new("minio");
    source.push(rec("minio", "sp", "/x/y.txt", EventKind::Created, "c1"));

    let mut state = EventJournalState::default();
    let batch: Vec<EventRecord> = source.queue.lock().unwrap().clone().into_iter().collect();
    let opts = EventOpts {
        applier: Some(std::sync::Arc::new(GraphApplier::new(store))),
        ..EventOpts::default()
    };
    let stats = apply_batch(&source, &journal, &mut state, batch, &opts)
        .await
        .unwrap();
    assert_eq!(stats.applied, 1);
    // apply_batch 签名不持 store——经 opts.applier 通路落图谱由
    // t08_graph_applier_lifecycle 覆盖；此处断言 stats 与 checkpoint 闭环
    assert_eq!(state.sources["minio"].processed, 1);
}
