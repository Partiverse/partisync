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

fn fatal(what: &str) -> PartisyError {
    PartisyError {
        severity: Severity::Fatal,
        source: Some(what.to_owned().into()),
    }
}

/// 确定性 fake 清单源：内存树 + 每次列取延迟 + 可控失败。
struct FakeSource {
    tree: HashMap<String, Vec<ListedNode>>,
    delay_ms: u64,
    /// 这些目录每次列都失败（失败隔离测试）。
    fail_always: Vec<String>,
    /// 这些目录首次列失败、重试成功（重试语义测试）。
    fail_once: Mutex<BTreeSet<String>>,
}

impl FakeSource {
    fn new(delay_ms: u64) -> Self {
        Self {
            tree: HashMap::new(),
            delay_ms,
            fail_always: Vec::new(),
            fail_once: Mutex::new(BTreeSet::new()),
        }
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

    for i in 0..24u32 {
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
    for j in 0..5u32 {
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
    assert_eq!(stats_serial.shards_done, 1 + 24 + 1 + 5); // 根+24 叶+top+5 子
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
    // 持续失败：failed 计 1、不阻塞其余（条目数差该目录 3 文件）
    let (mut src, mut expect) = wide_tree(0);
    src.fail_always = vec!["d05".into()];
    for k in 0..3 {
        expect.remove(&format!("d05/f{k}.txt"));
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
    src.fail_once = Mutex::new(BTreeSet::from(["d07".into()]));
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
