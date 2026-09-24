//! 分布式扫描调度器（SPEC M5-WP03）：目录前缀即分片、worker 池自取队列、
//! 分片账本断点恢复。
//!
//! 设计（裁定 1-8）：`ListSource`（清单源，列单层）+ [`ScanScheduler`]
//! （并行调度）+ `EntrySink`（批次消费）三件套；`impl ListSource for
//! Provider` 落本 crate（trait 本地，孤儿规则合规；sync→provider 为能力
//! 层→领域层向下依赖）。分片 = 目录前缀，worker 列单层后子目录作为新
//! 分片入队——自适应分裂即 BFS 展开本身（裁定 2），负载均衡 = 队列自取
//! （裁定 3）。断点恢复 = 分片粒度账本（裁定 4）：单层 list 为原子操作，
//! 分片即最小恢复单元。
//!
//! 与 `graph::remote_index` 的差异（裁定 6）：并行分片无全局字典序——
//! sink 须接受乱序批次（graph 写入面为幂等 upsert，乱序安全）；断点语义
//! 由分片账本承接，取代路径划界。

use std::future::Future;

use partisync_core::error::PartisyError;
use partisync_provider::Provider;

/// 单层清单项（`ListSource::list_dir` 返回；路径为 provider 内部全路径，
/// 无前导 `/`——`ProviderEntry` 同口径）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedNode {
    /// provider 内部全路径（相对 bucket/root）。
    pub path: String,
    /// 目录否。
    pub is_dir: bool,
    /// 字节大小（目录为 0）。
    pub size: u64,
    /// mtime ns（后端不提供为 0）。
    pub mtime_ns: u64,
}

impl ListedNode {
    /// 由 [`partisync_provider::ProviderEntry`] 升格（字段一一对应）。
    #[must_use]
    pub fn from_provider(e: partisync_provider::ProviderEntry) -> Self {
        Self {
            path: e.path,
            is_dir: e.is_dir,
            size: e.size,
            mtime_ns: e.mtime_ns,
        }
    }

    /// 直接父目录前缀（`""` = 根）；顶层项返回 `""`。
    #[must_use]
    pub fn parent_dir(&self) -> &str {
        match self.path.rfind('/') {
            Some(i) => &self.path[..i],
            None => "",
        }
    }
}

/// 清单源：列目录单层（裁定 1/8——层级语义由实现方吸收，调度器不直触
/// OpenDAL）。`dir = ""` 表示根。
pub trait ListSource: Send + Sync {
    /// 列 `dir` 单层子项。
    ///
    /// # Errors
    /// 源实现透传（Provider = OpenDAL 错误分类）。
    fn list_dir(
        &self,
        dir: &str,
    ) -> impl Future<Output = Result<Vec<ListedNode>, PartisyError>> + Send;
}

impl ListSource for Provider {
    async fn list_dir(&self, dir: &str) -> Result<Vec<ListedNode>, PartisyError> {
        Ok(self
            .list_children(dir)
            .await?
            .into_iter()
            .map(ListedNode::from_provider)
            .collect())
    }
}

impl<T: ListSource + ?Sized> ListSource for &T {
    async fn list_dir(&self, dir: &str) -> Result<Vec<ListedNode>, PartisyError> {
        (**self).list_dir(dir).await
    }
}

impl<T: ListSource + ?Sized> ListSource for Arc<T> {
    async fn list_dir(&self, dir: &str) -> Result<Vec<ListedNode>, PartisyError> {
        (**self).list_dir(dir).await
    }
}

/// 批次消费面（裁定 6）：按分片目录接收该层文件项；实现方须幂等
/// （并行 worker 乱序交付，且崩溃恢复会重放未落账本的分片）。
pub trait EntrySink: Send + Sync {
    /// 应用一批同目录文件项。
    ///
    /// # Errors
    /// 失败即 fail-fast（裁定 5：池立即停机）。
    fn apply(
        &self,
        dir: &str,
        entries: &[ListedNode],
    ) -> impl Future<Output = Result<(), PartisyError>> + Send;
}

impl<T: EntrySink + ?Sized> EntrySink for &T {
    async fn apply(&self, dir: &str, entries: &[ListedNode]) -> Result<(), PartisyError> {
        (**self).apply(dir, entries).await
    }
}

impl<T: EntrySink + ?Sized> EntrySink for Arc<T> {
    async fn apply(&self, dir: &str, entries: &[ListedNode]) -> Result<(), PartisyError> {
        (**self).apply(dir, entries).await
    }
}

/// 分片状态（账本持久态；在途分片不入账本——crash 后按 pending 重扫，
/// 裁定 4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShardStatus {
    /// 待处理。
    Pending,
    /// 完成（账本记录）。
    Done,
    /// 失败（重试 1 次后；错误文本随行，恢复时重入队）。
    Failed {
        /// 首个错误摘要。
        error: String,
    },
}

/// 扫描累计统计（原子累积；`snapshot()` 供回调与 CLI 输出）。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ScanStats {
    /// 已完成分片数。
    pub shards_done: u64,
    /// 失败分片数（重试耗尽）。
    pub shards_failed: u64,
    /// 已交付 sink 的文件条目数。
    pub entries: u64,
    /// 发现的目录（子分片）数。
    pub dirs: u64,
}
// ---------- 调度器核心（T03，SPEC 裁定 2/3/5） ----------

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::{Mutex, Notify};

/// 进度回调（每 done 分片一次，携带统计快照）。
pub type ProgressFn = Arc<dyn Fn(ScanStats) + Send + Sync>;
/// 告警回调（裁定 8：单分片条目超阈值——巨型扁平目录退化为单 worker）。
pub type WarningFn = Arc<dyn Fn(String) + Send + Sync>;

/// 调度选项（`Default` = concurrency min(8,cpu)、batch 512、告警阈值 10⁵）。
#[derive(Clone)]
pub struct ScanOpts {
    /// 并行 worker 数。
    pub concurrency: usize,
    /// sink 批大小（文件项/批）。
    pub batch_size: usize,
    /// 单分片文件项告警阈值。
    pub warning_threshold: usize,
    /// 进度回调（可选）。
    pub progress: Option<ProgressFn>,
    /// 告警回调（可选）。
    pub on_warning: Option<WarningFn>,
}

impl Default for ScanOpts {
    fn default() -> Self {
        Self {
            concurrency: std::thread::available_parallelism()
                .map_or(4, |n| n.get())
                .min(8),
            batch_size: 512,
            warning_threshold: 100_000,
            progress: None,
            on_warning: None,
        }
    }
}

impl std::fmt::Debug for ScanOpts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScanOpts")
            .field("concurrency", &self.concurrency)
            .field("batch_size", &self.batch_size)
            .field("warning_threshold", &self.warning_threshold)
            .field("progress", &self.progress.is_some())
            .field("on_warning", &self.on_warning.is_some())
            .finish()
    }
}

/// 内部原子计数器（[`ScanStats`] 的可变底座）。
#[derive(Debug, Default)]
struct Counters {
    shards_done: AtomicU64,
    shards_failed: AtomicU64,
    entries: AtomicU64,
    dirs: AtomicU64,
}

impl Counters {
    fn snapshot(&self) -> ScanStats {
        ScanStats {
            shards_done: self.shards_done.load(Ordering::Relaxed),
            shards_failed: self.shards_failed.load(Ordering::Relaxed),
            entries: self.entries.load(Ordering::Relaxed),
            dirs: self.dirs.load(Ordering::Relaxed),
        }
    }
}

/// worker 共享协调态。终止协议（裁定 3）：**队列空 且 无在途分片**——
/// 取分片与 active 计数同锁推进，防「弹走末分片未入队子分片」的提前停机
/// 竞态；分裂入队/在途减一后广播 `changed`。
struct Coordinator {
    queue: Mutex<VecDeque<String>>,
    active: AtomicUsize,
    changed: Notify,
    abort: AtomicBool,
    /// 失败分片账（分片 → 首个错误摘要；T04 账本承接）。
    failed: Mutex<Vec<(String, String)>>,
}

impl Coordinator {
    /// 取下一个分片；`None` = 全局完成（队列空且无在途）或已 abort。
    async fn next_shard(&self) -> Option<String> {
        loop {
            let (got, wait) = {
                let mut q = self.queue.lock().await;
                match q.pop_front() {
                    Some(shard) => {
                        // 同锁推进 active：持有分片即在途（终止判定无竞态窗）
                        self.active.fetch_add(1, Ordering::SeqCst);
                        (Some(shard), false)
                    }
                    None if self.abort.load(Ordering::SeqCst) => (None, false),
                    None if self.active.load(Ordering::SeqCst) == 0 => (None, false),
                    None => (None, true),
                }
            };
            if !wait {
                return got;
            }
            self.changed.notified().await;
        }
    }

    /// 分片处理收尾：在途减一并广播（分裂子分片此时已入队）。
    fn finish_shard(&self) {
        self.active.fetch_sub(1, Ordering::SeqCst);
        self.changed.notify_waiters();
    }
}

/// 单分片处理结果：`Err` 仅 sink 失败（fail-fast 传播，裁定 5）。
enum ShardOutcome {
    Done,
    Failed { error: String },
}

/// 扫描调度器（裁定 1/2/3/5）：worker 池 + BFS 分片队列自取。
pub struct ScanScheduler<S: ListSource, K: EntrySink> {
    source: Arc<S>,
    sink: Arc<K>,
    opts: ScanOpts,
}

impl<S: ListSource + 'static, K: EntrySink + 'static> ScanScheduler<S, K> {
    /// 默认选项构造。
    pub fn new(source: S, sink: K) -> Self {
        Self::with_opts(source, sink, ScanOpts::default())
    }

    /// 指定选项构造。
    pub fn with_opts(source: S, sink: K, opts: ScanOpts) -> Self {
        Self {
            source: Arc::new(source),
            sink: Arc::new(sink),
            opts,
        }
    }

    /// 执行扫描（`seed = ""` 为根）。返回累计统计；sink 失败 = fail-fast
    /// `Err`（裁定 5），分片源失败不传播（计入 `ScanStats::shards_failed`，
    /// T04 账本承接重扫）。
    ///
    /// # Errors
    /// sink 失败（fail-fast）或 worker 池 join 异常。
    pub async fn run(&self, seed: &str) -> Result<ScanStats, PartisyError> {
        let coord = Arc::new(Coordinator {
            queue: Mutex::new(VecDeque::from([seed.to_owned()])),
            active: AtomicUsize::new(0),
            changed: Notify::new(),
            abort: AtomicBool::new(false),
            failed: Mutex::new(Vec::new()),
        });
        let counters = Arc::new(Counters::default());
        let opts = Arc::new(self.opts.clone());
        let mut handles = Vec::with_capacity(self.opts.concurrency.max(1));
        for _ in 0..self.opts.concurrency.max(1) {
            handles.push(tokio::spawn(worker_loop(
                coord.clone(),
                self.source.clone(),
                self.sink.clone(),
                opts.clone(),
                counters.clone(),
            )));
        }
        let mut first_err = None;
        for h in handles {
            match h.await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    first_err.get_or_insert(e);
                }
                Err(e) => {
                    first_err.get_or_insert_with(|| PartisyError {
                        severity: partisync_core::error::Severity::Fatal,
                        source: Some(format!("scan worker join: {e}").into()),
                    });
                }
            }
        }
        match first_err {
            Some(e) => Err(e),
            None => Ok(counters.snapshot()),
        }
    }
}

/// worker 主循环（裁定 2/3/5）：取分片 → 处理 → 收尾广播。
async fn worker_loop<S: ListSource, K: EntrySink>(
    coord: Arc<Coordinator>,
    source: Arc<S>,
    sink: Arc<K>,
    opts: Arc<ScanOpts>,
    counters: Arc<Counters>,
) -> Result<(), PartisyError> {
    while let Some(shard) = coord.next_shard().await {
        let outcome = process_shard(&coord, &source, &sink, &opts, counters.as_ref(), &shard).await;
        coord.finish_shard(); // 分裂子分片已在 process_shard 内入队
        match outcome {
            Ok(ShardOutcome::Done) => {
                counters.shards_done.fetch_add(1, Ordering::Relaxed);
                if let Some(cb) = &opts.progress {
                    cb(counters.snapshot());
                }
            }
            Ok(ShardOutcome::Failed { error }) => {
                counters.shards_failed.fetch_add(1, Ordering::Relaxed);
                coord.failed.lock().await.push((shard, error));
            }
            Err(e) => {
                // fail-fast（裁定 5）：置 abort，全池在下一取片点退出
                coord.abort.store(true, Ordering::SeqCst);
                coord.changed.notify_waiters();
                return Err(e);
            }
        }
    }
    Ok(())
}

/// 处理单分片（裁定 2/5/8）：列单层（失败重试 1 次）→ 子目录入队（BFS
/// 分裂）→ 文件分批交 sink。失败重试耗尽 = [`ShardOutcome::Failed`]，
/// 不传播。
async fn process_shard<S: ListSource, K: EntrySink>(
    coord: &Coordinator,
    source: &S,
    sink: &K,
    opts: &ScanOpts,
    counters: &Counters,
    shard: &str,
) -> Result<ShardOutcome, PartisyError> {
    let nodes = match source.list_dir(shard).await {
        Ok(n) => n,
        Err(_first) => {
            // 重试 1 次（裁定 5）；耗尽 = Failed 记账，不传播
            match source.list_dir(shard).await {
                Ok(n) => n,
                Err(retry) => {
                    return Ok(ShardOutcome::Failed {
                        error: retry.to_string(),
                    });
                }
            }
        }
    };

    let mut subdirs = Vec::new();
    let mut files = Vec::new();
    for n in nodes {
        if n.is_dir {
            subdirs.push(n.path);
        } else {
            files.push(n);
        }
    }
    counters
        .dirs
        .fetch_add(subdirs.len() as u64, Ordering::Relaxed);

    if files.len() > opts.warning_threshold {
        if let Some(cb) = &opts.on_warning {
            cb(format!(
                "shard `{shard}` 单分片 {n} 项超阈值（巨型扁平目录退化为单 worker）",
                n = files.len()
            ));
        }
    }

    // 文件分批交 sink（sink 失败 = fail-fast Err 传播）
    for batch in files.chunks(opts.batch_size.max(1)) {
        sink.apply(shard, batch).await?;
    }
    counters
        .entries
        .fetch_add(files.len() as u64, Ordering::Relaxed);

    // 子目录作为新分片入队（BFS 自适应分裂，裁定 2）
    if !subdirs.is_empty() {
        let mut q = coord.queue.lock().await;
        for d in subdirs {
            q.push_back(d);
        }
        drop(q);
        coord.changed.notify_waiters();
    }
    Ok(ShardOutcome::Done)
}
