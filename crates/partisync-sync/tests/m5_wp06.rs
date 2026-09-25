//! M5-WP06 sync 混沌补缺（SPEC docs/specs/M5-WP06.md 裁定 3）。
//!
//! 多层故障叠加（scan + event failpoint 并发启用）+ chaos.rs 环形分区
//! 三节点编排（A↔B 断、B↔C 通、C↔A 断 → 逐对恢复 → 全网收敛）。

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use partisync_graph::store::EntryKind;
use partisync_sync::chaos::ChaosSim;
use partisync_sync::event::{
    EventDrain, EventKind, EventOpts, EventRecord, EventSource, FileEventJournal,
};
use partisync_sync::failpoint;
use partisync_sync::scan::{FileJournal, ListSource, ListedNode, ScanOpts, ScanScheduler};

/// dry-run 计数 sink（scan 调度器消费面）。
#[derive(Default)]
struct CountSink {
    entries: std::sync::atomic::AtomicU64,
}

impl partisync_sync::scan::EntrySink for CountSink {
    async fn apply(
        &self,
        _dir: &str,
        entries: &[ListedNode],
    ) -> Result<(), partisync_core::error::PartisyError> {
        self.entries
            .fetch_add(entries.len() as u64, std::sync::atomic::Ordering::Relaxed);
        let _ = entries;
        Ok(())
    }
}

fn tmp_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sync-m5wp06-{tag}-{}", partisync_core::Ulid::now()))
}

// ---------- 多层故障叠加（裁定 3） ----------

struct FakeSource {
    tree: Mutex<VecDeque<ListedNode>>,
}

impl ListSource for FakeSource {
    async fn list_dir(
        &self,
        _dir: &str,
    ) -> Result<Vec<ListedNode>, partisync_core::error::PartisyError> {
        let mut q = self.tree.lock().unwrap();
        Ok(q.drain(..).collect())
    }
}

struct MockEventSource {
    queue: Mutex<VecDeque<EventRecord>>,
}

impl EventSource for MockEventSource {
    async fn poll_batch(
        &self,
        max: usize,
        _deadline: Duration,
    ) -> Result<Vec<EventRecord>, partisync_sync::event::EventError> {
        let mut q = self.queue.lock().unwrap();
        let n = max.min(q.len());
        let out: Vec<EventRecord> = q.drain(..n).collect();
        Ok(out)
    }
    async fn commit_cursor(&self, _cursor: &str) -> Result<(), partisync_sync::event::EventError> {
        Ok(())
    }
    fn name(&self) -> &str {
        "chaos"
    }
}

fn ev(path: &str, cursor: &str) -> EventRecord {
    EventRecord {
        provider: "minio".into(),
        space: "sp".into(),
        path: path.into(),
        kind: EventKind::Created,
        cursor: cursor.into(),
        payload: serde_json::json!({}),
    }
}

/// 多层故障叠加：scan failpoint（`scan.shard_done`）与 event failpoint
/// （`event.before_apply`）**同时启用**——两个子系统的注入 panic 各自
/// 被捕获（scan 经 worker join，event 经 task join），互不妨碍；failpoint
/// 清除后各自重跑收敛（恢复语义独立成立）。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t03_layered_failpoints_recover_independently() {
    failpoint::clear();
    let scan_root = tmp_root("t03-layer-scan");
    let event_journal = tmp_root("t03-layer-event").join("e.json");

    // 两套子系统各自队列
    let scan_src = FakeSource {
        tree: Mutex::new(
            vec![ListedNode {
                path: "f0.txt".into(),
                is_dir: false,
                size: 1,
                mtime_ns: 0,
            }]
            .into_iter()
            .collect(),
        ),
    };
    let event_src = MockEventSource {
        queue: Mutex::new(vec![ev("a.txt", "c1")].into_iter().collect()),
    };

    // 双 failpoint 并发启用（裁定 3：叠加注入）
    failpoint::enable("scan.shard_done");
    failpoint::enable("event.before_apply");

    // scan：failpoint panic 经 worker join 浮出
    let scan_err = ScanScheduler::with_opts(
        scan_src,
        CountSink::default(),
        ScanOpts {
            concurrency: 1,
            ..ScanOpts::default()
        },
    )
    .run_journal("", FileJournal::new(scan_root.join("j.json")))
    .await
    .unwrap_err();
    assert!(
        scan_err.to_string().contains("join"),
        "scan panic 应经 join 浮出: {scan_err}"
    );

    // event：failpoint panic 经 task join 浮出（run_once 内联调用 apply——
    // panic 直接传播，须 spawn 捕获）
    let drain = EventDrain::with_opts(
        event_src,
        FileEventJournal::new(&event_journal),
        EventOpts::default(),
    );
    let jh = tokio::spawn(async move { drain.run_once().await });
    let event_err = jh.await;
    assert!(
        event_err.is_err(),
        "event panic 应使 task 失败: {event_err:?}"
    );

    // 清除 failpoint → 各自重跑收敛（互不干扰的证据：两个子系统在同一
    // 进程/同一 failpoint 表下先后恢复）
    failpoint::clear();

    let scan_src = FakeSource {
        tree: Mutex::new(
            vec![ListedNode {
                path: "f0.txt".into(),
                is_dir: false,
                size: 1,
                mtime_ns: 0,
            }]
            .into_iter()
            .collect(),
        ),
    };
    let stats = ScanScheduler::with_opts(
        scan_src,
        CountSink::default(),
        ScanOpts {
            concurrency: 2,
            ..ScanOpts::default()
        },
    )
    .run("")
    .await
    .unwrap();
    assert_eq!(stats.entries, 1, "scan 恢复后条目落账");

    let event_src = MockEventSource {
        queue: Mutex::new(vec![ev("a.txt", "c1")].into_iter().collect()),
    };
    let drain = EventDrain::with_opts(
        event_src,
        FileEventJournal::new(&event_journal),
        EventOpts::default(),
    );
    let stats = drain.run_once().await.unwrap();
    assert_eq!(stats.applied, 1, "event 恢复后事件落账");
}

// ---------- chaos.rs 环形分区编排（裁定 3） ----------

/// 环形分区：A↔B 断、B↔C 通、C↔A 断——B 中继期间 A/C 独立演进；
/// 逐对恢复后全网收敛、无数据丢失。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_ring_partition_relay_then_full_convergence() {
    let mut sim = ChaosSim::new(&["a", "b", "c"]).await;
    // 各节点本地文件（挂根 "/" 子下——session state 收集依赖根子树）
    for (dev, name) in [("a", "fa.txt"), ("b", "fb.txt"), ("c", "fc.txt")] {
        let store = sim.node(dev);
        let root_id = store
            .entry_by_path("/")
            .await
            .unwrap()
            .expect("根占位存在")
            .id;
        let path = format!("/{name}");
        store
            .add_entry(
                Some(root_id.as_str()),
                name,
                &path,
                EntryKind::File,
                3,
                1,
                None,
                None,
            )
            .await
            .unwrap();
        // add_entry 不记 oplog（capture.rs 口径：索引场景不捕获）——
        // bisync 交换依赖 oplog 水位，须显式捕获（wp09 同款）
        partisync_sync::capture::record_entry_upsert(store, &path)
            .await
            .unwrap();
    }

    // 环形分区：A↔B 断、C↔A 断（B↔C 默认通）
    sim.set_partition("a", "b", false);
    sim.set_partition("c", "a", false);

    // B↔C 双向 push（分区窗口内可达对收敛）
    let s1 = sim.push("b", "c").await.unwrap();
    let s2 = sim.push("c", "b").await.unwrap();
    assert!(
        s1.applied + s2.applied >= 2,
        "B↔C 应至少互传 1 条: {s1:?} {s2:?}"
    );

    // 逐对恢复 + push：**已知协议语义**（M2-WP09 裁定）——oplog 行应用后
    // 全局 trim（无中继转发），环形分区期间 A 漏看的行经 push 不再可达；
    // 漏行由 reconcile 全量对账收口（WP03 对账兜底的混沌验证）
    sim.set_partition("a", "b", true);
    sim.push("b", "a").await.unwrap();
    sim.push("a", "b").await.unwrap();
    sim.set_partition("c", "a", true);
    sim.push("c", "a").await.unwrap();
    sim.push("a", "c").await.unwrap();

    // reconcile 全量对账：逐对收口（push 漏行在此补齐）
    sim.reconcile("a", "b").await.unwrap();
    sim.reconcile("a", "c").await.unwrap();
    sim.reconcile("b", "c").await.unwrap();

    // 全网收敛断言：三节点均见三个文件（无数据丢失）
    let expect: Vec<String> = vec!["/fa.txt".into(), "/fb.txt".into(), "/fc.txt".into()];
    for dev in ["a", "b", "c"] {
        let store = sim.node(dev);
        for path in &expect {
            let row = store.entry_by_path(path).await.unwrap();
            assert!(row.is_some(), "节点 {dev} 缺 {path}");
        }
    }
}
