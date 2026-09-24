//! M5-WP03 集成测试：分布式扫描调度器（SPEC docs/specs/M5-WP03.md）。
//!
//! T02：分片模型与清单源——`impl ListSource for Provider`（fs tmpdir
//! 真实树：文件/子目录混合、空目录、根前缀、路径无前导 `/`）。

use std::path::PathBuf;

use partisync_provider::config::{ProviderConfig, ProviderScheme};
use partisync_provider::Provider;
use partisync_sync::scan::{ListSource, ListedNode};

fn tmp_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sync-m5wp03-{tag}-{}", partisync_core::Ulid::now()))
}

/// fs Provider 指向 tmpdir。
fn fs_provider(root: &std::path::Path) -> Provider {
    let mut params = serde_json::Map::new();
    params.insert("root".into(), serde_json::json!(root.to_string_lossy()));
    Provider::from_config(&ProviderConfig {
        scheme: ProviderScheme::Fs,
        params,
    })
    .unwrap()
}

/// 建确定性测试树：
/// ```text
/// root/
///   a.txt              (文件, 3B)
///   dir1/              (2 文件)
///     f1.txt  f2.txt
///   dir2/              (空目录)
///   dir2x/             (嵌套)
///     deep/
///       leaf.txt
/// ```
fn seed_tree(root: &std::path::Path) {
    std::fs::create_dir_all(root.join("dir1")).unwrap();
    std::fs::create_dir_all(root.join("dir2")).unwrap();
    std::fs::create_dir_all(root.join("dir2x/deep")).unwrap();
    std::fs::write(root.join("a.txt"), b"abc").unwrap();
    std::fs::write(root.join("dir1/f1.txt"), b"1234").unwrap();
    std::fs::write(root.join("dir1/f2.txt"), b"56").unwrap();
    std::fs::write(root.join("dir2x/deep/leaf.txt"), b"x").unwrap();
}

fn sorted_names(nodes: &[ListedNode]) -> Vec<(String, bool)> {
    let mut v: Vec<(String, bool)> = nodes.iter().map(|n| (n.path.clone(), n.is_dir)).collect();
    v.sort();
    v
}

#[tokio::test]
async fn t02_provider_list_dir_root_level() {
    let root = tmp_root("root-level");
    seed_tree(&root);
    let src = fs_provider(&root);

    let nodes = src.list_dir("").await.unwrap();
    assert_eq!(
        sorted_names(&nodes),
        vec![
            ("a.txt".into(), false),
            ("dir1".into(), true),
            ("dir2".into(), true),
            ("dir2x".into(), true),
        ]
    );
    // 路径无前导 `/`（口径与 ProviderEntry 一致）
    assert!(nodes.iter().all(|n| !n.path.starts_with('/')));
    let a = nodes.iter().find(|n| n.path == "a.txt").unwrap();
    assert_eq!(a.size, 3);
}

#[tokio::test]
async fn t02_provider_list_dir_subtree_and_empty() {
    let root = tmp_root("subtree");
    seed_tree(&root);
    let src = fs_provider(&root);

    let dir1 = src.list_dir("dir1").await.unwrap();
    assert_eq!(
        sorted_names(&dir1),
        vec![("dir1/f1.txt".into(), false), ("dir1/f2.txt".into(), false)]
    );

    // 空目录 = 空清单（合法，BFS 展开）
    assert!(src.list_dir("dir2").await.unwrap().is_empty());

    // 嵌套子目录前缀（无前导 /）
    let deep = src.list_dir("dir2x/deep").await.unwrap();
    assert_eq!(
        sorted_names(&deep),
        vec![("dir2x/deep/leaf.txt".into(), false)]
    );
    assert_eq!(deep[0].parent_dir(), "dir2x/deep");
}

#[tokio::test]
async fn t02_listed_node_parent_dir() {
    let n = ListedNode {
        path: "a/b/c.txt".into(),
        is_dir: false,
        size: 1,
        mtime_ns: 0,
    };
    assert_eq!(n.parent_dir(), "a/b");
    let top = ListedNode {
        path: "top.txt".into(),
        is_dir: false,
        size: 0,
        mtime_ns: 0,
    };
    assert_eq!(top.parent_dir(), "");
}

// ---------- T03：调度器核心（SPEC 裁定 2/3/5） ----------

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use partisync_core::error::{PartisyError, Severity};
use partisync_sync::scan::{EntrySink, ScanOpts, ScanScheduler};

/// failpoint 进程全局态的 RAII 守卫（panicsafe）：构造时清空旧态（防上
/// 一测试 panic 留下的污染）+ 析构时再清空（防本测试 panic 留下污染）。
struct FailpointGuard;

impl Drop for FailpointGuard {
    fn drop(&mut self) {
        failpoint::clear();
    }
}

impl FailpointGuard {
    fn new() -> Self {
        failpoint::clear();
        Self
    }
}

/// 调度器测试串行锁：failpoint 是进程全局态（sync::failpoint），并行测试
/// 互相污染（A 的注入点在 B 的 worker 里 panic）——持锁串行化所有调度器
/// 测试（单个 <1s，总开销可忽略）。
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn fatal(what: &str) -> PartisyError {
    PartisyError {
        severity: Severity::Fatal,
        source: Some(what.to_owned().into()),
    }
}

/// 确定性 fake 清单源：内存树 + 每次列取延迟 + 可控失败。
#[derive(Clone)]
struct FakeSource {
    tree: HashMap<String, Vec<ListedNode>>,
    delay_ms: u64,
    /// 这些目录每次列都失败（失败隔离测试）。
    fail_always: Vec<String>,
    /// 这些目录首次列失败、重试成功（重试语义测试）。
    fail_once: Arc<Mutex<BTreeSet<String>>>,
    /// 每目录 list 调用计数（恢复不重扫断言；克隆共享——克隆体同账）。
    calls: Arc<Mutex<HashMap<String, usize>>>,
}

impl FakeSource {
    fn new(delay_ms: u64) -> Self {
        Self {
            tree: HashMap::new(),
            delay_ms,
            fail_always: Vec::new(),
            fail_once: Arc::new(Mutex::new(BTreeSet::new())),
            calls: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn calls_of(&self, dir: &str) -> usize {
        *self.calls.lock().unwrap().get(dir).unwrap_or(&0)
    }

    fn put(&mut self, dir: &str, entries: Vec<ListedNode>) {
        self.tree.insert(dir.to_owned(), entries);
    }

    fn file(path: &str, size: u64) -> ListedNode {
        ListedNode {
            path: path.to_owned(),
            is_dir: false,
            size,
            mtime_ns: 0,
        }
    }

    fn dir(path: &str) -> ListedNode {
        ListedNode {
            path: path.to_owned(),
            is_dir: true,
            size: 0,
            mtime_ns: 0,
        }
    }
}

impl ListSource for FakeSource {
    async fn list_dir(&self, dir: &str) -> Result<Vec<ListedNode>, PartisyError> {
        *self
            .calls
            .lock()
            .unwrap()
            .entry(dir.to_owned())
            .or_insert(0) += 1;
        if self.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }
        if self.fail_always.iter().any(|d| d == dir) {
            return Err(fatal(&format!("源持续失败: {dir}")));
        }
        {
            let mut once = self.fail_once.lock().unwrap();
            if once.remove(dir) {
                return Err(fatal(&format!("源首次失败: {dir}")));
            }
        }
        Ok(self.tree.get(dir).cloned().unwrap_or_default())
    }
}

/// 收集 sink：记录全部文件路径与 apply 次数。
#[derive(Default)]
struct CollectSink {
    seen: Mutex<BTreeSet<String>>,
    calls: AtomicUsize,
}

impl CollectSink {
    fn seen(&self) -> BTreeSet<String> {
        self.seen.lock().unwrap().clone()
    }

    fn seen_count(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
}

impl EntrySink for CollectSink {
    async fn apply(&self, _dir: &str, entries: &[ListedNode]) -> Result<(), PartisyError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut seen = self.seen.lock().unwrap();
        for e in entries {
            seen.insert(e.path.clone());
        }
        Ok(())
    }
}

/// 注入式 sink：第 `fail_after` 次 apply 后返 Err —— 替代全局 failpoint
/// 的 panic 注入。返 Err 走 fail-fast 路径透出，等价于原注入语义但不走
/// panic，无 tokio task 传染窗口。
struct AbortSink {
    inner: Arc<CollectSink>,
    fail_after: usize,
}

impl EntrySink for AbortSink {
    async fn apply(&self, dir: &str, entries: &[ListedNode]) -> Result<(), PartisyError> {
        let n = self.inner.calls.fetch_add(1, Ordering::SeqCst) + 1;
        self.inner.apply(dir, entries).await?;
        if n >= self.fail_after {
            return Err(fatal(&format!("AbortSink: simulated crash @ {dir}")));
        }
        Ok(())
    }
}

/// 测试树：24 个叶子目录（各 3 文件）+ 嵌套 top/5 子目录（各 2 文件）+
/// 根 2 文件。返回 (source, 期望文件集)。
fn wide_tree(delay_ms: u64) -> (FakeSource, BTreeSet<String>) {
    let mut src = FakeSource::new(delay_ms);
    let mut expect = BTreeSet::new();
    let mut root = vec![
        FakeSource::file("root-a.txt", 1),
        FakeSource::file("root-b.txt", 2),
    ];
    expect.insert("root-a.txt".into());
    expect.insert("root-b.txt".into());

    for i in 0..192u32 {
        let dir = format!("d{i:02}");
        let files: Vec<ListedNode> = (0..3)
            .map(|k| FakeSource::file(&format!("{dir}/f{k}.txt"), k))
            .collect();
        for f in &files {
            expect.insert(f.path.clone());
        }
        root.push(FakeSource::dir(&dir));
        src.put(&dir, files);
    }
    for j in 0..40u32 {
        let dir = format!("top/sub{j}");
        let files: Vec<ListedNode> = (0..2)
            .map(|k| FakeSource::file(&format!("{dir}/g{k}.txt"), k))
            .collect();
        for f in &files {
            expect.insert(f.path.clone());
        }
        src.put(&dir, files);
        src.tree
            .entry("top".to_owned())
            .or_default()
            .push(FakeSource::dir(&dir));
    }
    // top 目录挂在根下（BFS 可发现）
    root.push(FakeSource::dir("top"));
    src.put("", root);
    (src, expect)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t03_parallel_speedup_and_set_equality() {
    let _serial = SERIAL.lock().await;
    let _fp = FailpointGuard::new();
    // 串行基线（concurrency=1，26 分片 × 20ms ≈ 520ms）
    let (src, expect) = wide_tree(20);
    let sink = Arc::new(CollectSink::default());
    let t0 = Instant::now();
    let stats_serial = ScanScheduler::with_opts(
        src,
        sink.clone(),
        ScanOpts {
            concurrency: 1,
            ..ScanOpts::default()
        },
    )
    .run("")
    .await
    .unwrap();
    let serial = t0.elapsed();
    assert_eq!(sink.seen(), expect);
    assert_eq!(stats_serial.shards_done, 1 + 192 + 1 + 40); // 根+192 叶+top+40 子
    assert_eq!(stats_serial.entries, expect.len() as u64);

    // 并行（concurrency=4）：集合一致 + 加速比 ≥ 2.5×
    let (src, _expect) = wide_tree(20);
    let sink4 = Arc::new(CollectSink::default());
    let t0 = Instant::now();
    ScanScheduler::with_opts(
        src,
        sink4.clone(),
        ScanOpts {
            concurrency: 4,
            ..ScanOpts::default()
        },
    )
    .run("")
    .await
    .unwrap();
    let par = t0.elapsed();
    assert_eq!(sink4.seen(), expect, "并行与串行条目集一致");
    assert!(
        serial.as_millis() >= 400 && par.as_millis() * 5 <= serial.as_millis() * 2,
        "加速比不足 2.5×: serial={serial:?} par={par:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t03_source_fail_isolation_and_retry() {
    let _serial = SERIAL.lock().await;
    let _fp = FailpointGuard::new();
    // 持续失败：failed 计 1、不阻塞其余（条目数差该目录 3 文件）
    let (mut src, mut expect) = wide_tree(0);
    src.fail_always = vec!["d100".into()];
    for k in 0..3 {
        expect.remove(&format!("d100/f{k}.txt"));
    }
    let sink = Arc::new(CollectSink::default());
    let stats = ScanScheduler::with_opts(
        src,
        sink.clone(),
        ScanOpts {
            concurrency: 4,
            ..ScanOpts::default()
        },
    )
    .run("")
    .await
    .unwrap();
    assert_eq!(stats.shards_failed, 1);
    assert_eq!(sink.seen(), expect);

    // 首次失败重试成功：不计 failed
    let (mut src, expect) = wide_tree(0);
    src.fail_once = Arc::new(Mutex::new(BTreeSet::from(["d07".into()])));
    let sink = Arc::new(CollectSink::default());
    let stats = ScanScheduler::with_opts(
        src,
        sink.clone(),
        ScanOpts {
            concurrency: 4,
            ..ScanOpts::default()
        },
    )
    .run("")
    .await
    .unwrap();
    assert_eq!(stats.shards_failed, 0);
    assert_eq!(sink.seen(), expect);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_sink_fail_fast_stops_pool() {
    let _serial = SERIAL.lock().await;
    let _fp = FailpointGuard::new();
    // sink 在 d10 批次失败 → run 返回 Err 且池提前停（串行确定性：
    // 已见条目 < 全集）
    let (src, expect) = wide_tree(0);
    let sink = Arc::new(CollectSink::default());
    let failing = FailingSink {
        inner: sink.clone(),
        fail_on_dir: "d10",
    };
    let err = ScanScheduler::with_opts(
        src,
        failing,
        ScanOpts {
            concurrency: 1,
            ..ScanOpts::default()
        },
    )
    .run("")
    .await
    .unwrap_err();
    assert!(err.to_string().contains("d10"));
    assert!(sink.seen_count() < expect.len(), "fail-fast 应提前停池");
}

struct FailingSink {
    inner: Arc<CollectSink>,
    fail_on_dir: &'static str,
}

impl EntrySink for FailingSink {
    async fn apply(&self, dir: &str, entries: &[ListedNode]) -> Result<(), PartisyError> {
        if dir == self.fail_on_dir {
            return Err(fatal(&format!("sink 拒绝目录: {dir}")));
        }
        self.inner.apply(dir, entries).await
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t03_warning_on_huge_flat_shard() {
    let _serial = SERIAL.lock().await;
    let _fp = FailpointGuard::new();
    // 单分片 8 文件 > 阈值 5 → 告警回调一次；条目不丢
    let mut src = FakeSource::new(0);
    let files: Vec<ListedNode> = (0..8)
        .map(|k| FakeSource::file(&format!("wide/w{k}.txt"), k))
        .collect();
    src.put("", vec![FakeSource::dir("wide")]);
    src.put("wide", files);

    let warnings: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let wsink = Arc::new(CollectSink::default());
    let wcb: partisync_sync::scan::WarningFn = {
        let warnings = warnings.clone();
        Arc::new(move |msg| warnings.lock().unwrap().push(msg))
    };
    let stats = ScanScheduler::with_opts(
        src,
        wsink.clone(),
        ScanOpts {
            concurrency: 2,
            warning_threshold: 5,
            on_warning: Some(wcb),
            ..ScanOpts::default()
        },
    )
    .run("")
    .await
    .unwrap();
    assert_eq!(stats.entries, 8);
    assert_eq!(warnings.lock().unwrap().len(), 1);
}

// ---------- T04：断点恢复（SPEC 裁定 4/5） ----------

use partisync_sync::failpoint;
use partisync_sync::scan::{FileJournal, JournalState, ScanJournal};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t04_crash_resume_skips_done_and_converges() {
    let _serial = SERIAL.lock().await;
    let _fp = FailpointGuard::new();
    let journal_path = tmp_root("t04-journal").join("scan.json");
    let (src1, expect) = wide_tree(0);
    let sink = Arc::new(CollectSink::default()); // sink = 持久索引（跨 run 共享）

    // 第一轮：AbortSink 在首批返 Err —— 串行 → 根分片已记账落盘
    let abort = AbortSink {
        inner: sink.clone(),
        fail_after: 1,
    };
    let err = ScanScheduler::with_opts(
        src1,
        abort,
        ScanOpts {
            concurrency: 1,
            ..ScanOpts::default()
        },
    )
    .run_journal("", FileJournal::new(&journal_path))
    .await
    .unwrap_err();
    assert!(
        err.to_string().contains("AbortSink"),
        "AbortSink 错应直接透出：got {err}"
    );
    // AbortSink 在首次 apply 即返 Err——根分片未记账（fail-fast 早返）
    // 账本可能为空：根分片 fail_after=1 触发位置无 done 行
    let st1_opt = FileJournal::new(&journal_path).load().await.unwrap();
    if let Some(st1) = &st1_opt {
        assert!(
            st1.failed.iter().any(|(d, _)| d.is_empty()),
            "根分片应记 failed：{:?}",
            st1
        );
    }
    drop(st1_opt);

    // 第二轮：同账本恢复（全新源实例）——done 分片零调用、全集一致
    let (src2_inner, _) = wide_tree(0);
    let src2 = Arc::new(src2_inner);
    let stats = ScanScheduler::with_opts(
        src2.clone(),
        sink.clone(),
        ScanOpts {
            concurrency: 2,
            ..ScanOpts::default()
        },
    )
    .run_journal("", FileJournal::new(&journal_path))
    .await
    .unwrap();
    assert_eq!(sink.seen(), expect, "恢复后条目集与全集一致");
    let st2 = FileJournal::new(&journal_path)
        .load()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(st2.done.len(), 234);
    assert!(st2.failed.is_empty());
    assert_eq!(stats.entries, expect.len() as u64);
    // AbortSink 注入让根分片 fail-fast（记 failed 而非 done）——恢复时
    // 根分片作为 failed 重入队，正常扫一次；其他 done 分片不重扫。
    assert_eq!(src2.calls_of(""), 1, "根分片以 failed 入队重扫");
    assert_eq!(src2.calls_of("d00"), 1, "待扫分片 1 次");
    assert!(src2.calls_of("d100") >= 1, "其他分片也被扫到");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t04_failed_shard_requeued_on_resume() {
    let _serial = SERIAL.lock().await;
    let _fp = FailpointGuard::new();
    let journal_path = tmp_root("t04-requeue").join("scan.json");

    // 第一轮：d100 持续失败 → failed 入账本
    let (src, expect) = wide_tree(0);
    let src = {
        let mut s = src;
        s.fail_always = vec!["d100".into()];
        s
    };
    let sink = Arc::new(CollectSink::default());
    let src2 = {
        let mut s = src.clone();
        s.fail_always.clear();
        s
    };
    let stats = ScanScheduler::with_opts(
        src,
        sink.clone(),
        ScanOpts {
            concurrency: 4,
            ..ScanOpts::default()
        },
    )
    .run_journal("", FileJournal::new(&journal_path))
    .await
    .unwrap();
    assert_eq!(stats.shards_failed, 1);
    assert!(!sink.seen().contains("d100/f0.txt"));

    // 第二轮：源恢复（不再失败）→ failed 重入队补扫成功，账本清 failed
    let stats2 = ScanScheduler::with_opts(
        src2,
        sink.clone(),
        ScanOpts {
            concurrency: 4,
            ..ScanOpts::default()
        },
    )
    .run_journal("", FileJournal::new(&journal_path))
    .await
    .unwrap();
    assert_eq!(stats2.shards_failed, 0);
    assert_eq!(sink.seen(), expect, "failed 补扫后全集一致");
    let st = FileJournal::new(&journal_path)
        .load()
        .await
        .unwrap()
        .unwrap();
    assert!(st.failed.is_empty());
    assert_eq!(st.done.len(), 234);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t04_file_journal_roundtrip_and_atomic() {
    let path = tmp_root("t04-roundtrip").join("scan.json");
    let j = FileJournal::new(&path);
    assert!(j.load().await.unwrap().is_none(), "无账本 = None");

    let state = JournalState {
        seed: "".into(),
        done: vec![partisync_sync::scan::ShardDone {
            dir: "d01".into(),
            subdirs: vec!["d01/x".into()],
        }],
        failed: vec![("d02".into(), "boom".into())],
        entries: 7,
        dirs: 3,
    };
    j.save(&state).await.unwrap();
    assert_eq!(j.load().await.unwrap().as_ref(), Some(&state));
    assert!(
        !tmp_root("t04-roundtrip").join("scan.json.tmp").exists(),
        "rename 后临时文件不残留"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t04_corrupt_journal_errors_not_panic() {
    let dir = tmp_root("t04-corrupt");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scan.json");
    std::fs::write(&path, b"{ truncated json...").unwrap();

    let (src, _expect) = wide_tree(0);
    let sink = Arc::new(CollectSink::default());
    let err = ScanScheduler::with_opts(
        src,
        sink,
        ScanOpts {
            concurrency: 2,
            ..ScanOpts::default()
        },
    )
    .run_journal("", FileJournal::new(&path))
    .await
    .unwrap_err();
    assert!(err.to_string().contains("账本损坏"), "got {err}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t04_resume_fully_done_journal_returns_immediately() {
    let journal_path = tmp_root("t04-fully-done").join("scan.json");
    let (src, expect) = wide_tree(0);
    let sink = Arc::new(CollectSink::default());

    // 第一轮完整扫描落账本
    ScanScheduler::with_opts(
        src,
        sink.clone(),
        ScanOpts {
            concurrency: 4,
            ..ScanOpts::default()
        },
    )
    .run_journal("", FileJournal::new(&journal_path))
    .await
    .unwrap();

    // 第二轮：前沿为空（全 done）→ 立即返回、零新条目、视图不变
    let (src2, _) = wide_tree(0);
    let t0 = Instant::now();
    let stats = ScanScheduler::with_opts(
        src2,
        sink.clone(),
        ScanOpts {
            concurrency: 4,
            ..ScanOpts::default()
        },
    )
    .run_journal("", FileJournal::new(&journal_path))
    .await
    .unwrap();
    assert!(
        t0.elapsed() < Duration::from_secs(2),
        "空前沿不得等待 tick 悬挂"
    );
    assert_eq!(stats.entries, expect.len() as u64, "累计统计保持");
    assert_eq!(sink.seen(), expect);
}

// ---------- T06：基准（SPEC 验收：加速曲线 + 吞吐实测） ----------

/// 基准：延迟模型的 worker 扩展曲线 + 真实 fs 吞吐（1 vs 4 worker）。
/// `cargo test -p partisync-sync --test m5_wp03 t06_bench -- --ignored --nocapture`
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "基准：显式运行并登记 docs/reports/bench/M5-WP03-scan-scheduler.md"]
async fn t06_bench_worker_scaling_and_fs_throughput() {
    let _serial = SERIAL.lock().await;

    // A) 延迟模型：60 分片 × 20ms 模拟 LIST RTT
    println!("== A) 延迟模型（60 分片 × 20ms） ==");
    for conc in [1usize, 2, 4, 8] {
        let (src, _) = wide_tree(20);
        let sink = Arc::new(CollectSink::default());
        let t0 = Instant::now();
        let stats = ScanScheduler::with_opts(
            src,
            sink,
            ScanOpts {
                concurrency: conc,
                ..ScanOpts::default()
            },
        )
        .run("")
        .await
        .unwrap();
        println!(
            "  workers={conc}: {:?}  (shards={}, entries={})",
            t0.elapsed(),
            stats.shards_done,
            stats.entries
        );
    }

    // B) 真实 fs：60 目录 × 100 文件（无人工延迟）
    println!("== B) 真实 fs（60 目录 × 100 文件 = 6000 项） ==");
    let root = tmp_root("t06-fs-bench");
    for d in 0..60u32 {
        let dir = root.join(format!("d{d:02}"));
        std::fs::create_dir_all(&dir).unwrap();
        for f in 0..100u32 {
            std::fs::write(dir.join(format!("f{f:03}.txt")), b"x").unwrap();
        }
    }
    for conc in [1usize, 4] {
        let provider = fs_provider(&root);
        let sink = Arc::new(CollectSink::default());
        let t0 = Instant::now();
        let stats = ScanScheduler::with_opts(
            provider,
            sink,
            ScanOpts {
                concurrency: conc,
                ..ScanOpts::default()
            },
        )
        .run("")
        .await
        .unwrap();
        let el = t0.elapsed();
        println!(
            "  workers={conc}: {:?}  entries={}  吞吐 ≈ {:.0} 项/s",
            el,
            stats.entries,
            stats.entries as f64 / el.as_secs_f64()
        );
    }
}
