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
use std::time::Duration;

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
} // ---------- 断点恢复账本（T04，SPEC 裁定 4） ----------

/// 分片完成记录：目录 + 发现的子分片（恢复前沿构建依据——跳过 done 分片
/// 的重扫而不丢失其未完成子分片的发现链）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShardDone {
    /// 分片目录前缀。
    pub dir: String,
    /// 该分片列出的子目录（下一层分片）。
    pub subdirs: Vec<String>,
}

/// 账本持久态（分片粒度，裁定 4）：单层 list 为原子操作，分片即最小恢复
/// 单元——在途分片不入账本，crash 至多重扫当前在途分片。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JournalState {
    /// 扫描种子（前缀；`""` = 根）。
    pub seed: String,
    /// 已完成分片（恢复时跳过重扫）。
    pub done: Vec<ShardDone>,
    /// 失败分片（分片 → 错误摘要；恢复时重入队）。
    pub failed: Vec<(String, String)>,
    /// 已交付 sink 的文件条目累计。
    pub entries: u64,
    /// 已发现目录（子分片）累计。
    pub dirs: u64,
}

/// 分片账本（裁定 4：每分片终态落盘一次）。
pub trait ScanJournal: Send + Sync {
    /// 原子落盘账本（实现须防半写，如临时文件 + rename）。
    ///
    /// # Errors
    /// IO/序列化失败。
    fn save(&self, state: &JournalState) -> impl Future<Output = Result<(), PartisyError>> + Send;

    /// 读取账本；无账本 = `Ok(None)`。
    ///
    /// # Errors
    /// 账本存在但损坏（调用方自行决定降级/中止）或 IO 失败。
    fn load(&self) -> impl Future<Output = Result<Option<JournalState>, PartisyError>> + Send;
}

/// 无账本（[`ScanScheduler::run`] 默认；内存态，不持久化）。
pub struct NoJournal;

impl ScanJournal for NoJournal {
    async fn save(&self, _state: &JournalState) -> Result<(), PartisyError> {
        Ok(())
    }

    async fn load(&self) -> Result<Option<JournalState>, PartisyError> {
        Ok(None)
    }
}

impl<T: ScanJournal + ?Sized> ScanJournal for Arc<T> {
    async fn save(&self, state: &JournalState) -> Result<(), PartisyError> {
        (**self).save(state).await
    }

    async fn load(&self) -> Result<Option<JournalState>, PartisyError> {
        (**self).load().await
    }
}

impl<T: ScanJournal + ?Sized> ScanJournal for &T {
    async fn save(&self, state: &JournalState) -> Result<(), PartisyError> {
        (**self).save(state).await
    }

    async fn load(&self) -> Result<Option<JournalState>, PartisyError> {
        (**self).load().await
    }
}

/// JSON 文件账本（临时文件 + 原子 rename，防半写）。
#[derive(Debug, Clone)]
pub struct FileJournal {
    path: std::path::PathBuf,
}

impl FileJournal {
    /// 指定账本文件路径构造。
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl ScanJournal for FileJournal {
    async fn save(&self, state: &JournalState) -> Result<(), PartisyError> {
        let json = serde_json::to_vec(state).map_err(|e| fatal_io(format!("账本序列化: {e}")))?;
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| fatal_io(format!("账本目录创建: {e}")))?;
            }
        }
        // 临时名带进程内唯一序号：并发 save 各用各的 tmp（共享名会被
        // 对手的 rename 抽走 → ENOENT）
        static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = self.path.with_extension(format!("tmp.{seq}"));
        tokio::fs::write(&tmp, json)
            .await
            .map_err(|e| fatal_io(format!("账本临时写入: {e}")))?;
        tokio::fs::rename(&tmp, &self.path)
            .await
            .map_err(|e| fatal_io(format!("账本原子替换: {e}")))?;
        Ok(())
    }

    async fn load(&self) -> Result<Option<JournalState>, PartisyError> {
        match tokio::fs::read(&self.path).await {
            Ok(bytes) => {
                Ok(Some(serde_json::from_slice(&bytes).map_err(|e| {
                    fatal_io(format!("账本损坏（删除后可全新恢复）: {e}"))
                })?))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(fatal_io(format!("账本读取: {e}"))),
        }
    }
}

fn fatal_io(what: String) -> PartisyError {
    PartisyError {
        severity: partisync_core::error::Severity::Fatal,
        source: Some(what.into()),
    }
}

// ---------- 调度器核心（T03，SPEC 裁定 2/3/5） ----------

use std::collections::{BTreeSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::Mutex;

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

/// 内部原子计数器（[`ScanStats`] 的可变底座；恢复时自账本初值起算）。
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

    fn set_shards_done(&self, v: u64) {
        self.shards_done.store(v, Ordering::Relaxed);
    }

    fn set_entries(&self, v: u64) {
        self.entries.store(v, Ordering::Relaxed);
    }

    fn set_dirs(&self, v: u64) {
        self.dirs.store(v, Ordering::Relaxed);
    }
}

/// 终止轮询间隔：pending 归零/abort 的兜底重查周期——替代条件变量广播，
/// 无丢失唤醒窗口（每 tick 重查，正确性不依赖时序）。
const POLL_TICK: Duration = Duration::from_millis(50);

/// worker 共享协调态（裁定 3）：`pending = 排队 + 在途` 分片数；worker 取
/// 分片自 mpsc，终止 = **pending 归零**（最后一片收尾后全池 tick 退出）或
/// abort。子分片入队先加 pending 再发送，本分片收尾再减——计数恒准确，
/// 「弹走末分片未入队子分片」不产生提前停机。
struct PoolNet {
    tx: tokio::sync::mpsc::UnboundedSender<String>,
    rx: tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<String>>,
    pending: AtomicUsize,
    abort: AtomicBool,
}

impl PoolNet {
    /// 取下一个分片；`None` = 全局完成（pending 归零）或已 abort。
    async fn next_shard(&self) -> Option<String> {
        loop {
            if self.pending.load(Ordering::SeqCst) == 0 || self.abort.load(Ordering::SeqCst) {
                return None;
            }
            // 锁内 recv（多 worker 串行取件）；tick 兜底重查终止条件
            let mut guard = self.rx.lock().await;
            match tokio::time::timeout(POLL_TICK, guard.recv()).await {
                Ok(Some(shard)) => return Some(shard),
                Ok(None) => return None, // 通道关闭（异常路径，防御性退出）
                Err(_elapsed) => {}      // tick：重查 pending/abort
            }
        }
    }

    /// 子分片入队（BFS 自适应分裂，裁定 2）：先加 pending 再发送。
    fn enqueue(&self, dirs: Vec<String>) {
        if dirs.is_empty() {
            return;
        }
        self.pending.fetch_add(dirs.len(), Ordering::SeqCst);
        for d in dirs {
            let _ = self.tx.send(d);
        }
    }

    /// 本分片收尾（done/failed 均计）。
    fn shard_done(&self) {
        self.pending.fetch_sub(1, Ordering::SeqCst);
    }
}

/// 单分片处理结果：`Err` 仅 sink 失败（fail-fast 传播，裁定 5）。
enum ShardOutcome {
    /// 完成（携带发现的子分片，供入队与账本记录）。
    Done { subdirs: Vec<String> },
    /// 失败（重试 1 次耗尽；错误摘要随行）。
    Failed {
        /// 首个错误摘要。
        error: String,
    },
}

/// 扫描调度器（裁定 1/2/3/5）：worker 池 + BFS 分片队列自取 + 分片账本。
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

    /// 执行扫描（`seed = ""` 为根；无账本）。返回累计统计；sink 失败 =
    /// fail-fast `Err`（裁定 5），分片源失败不传播（计入
    /// `ScanStats::shards_failed`）。
    ///
    /// # Errors
    /// sink 失败（fail-fast）、账本损坏或 worker 池 join 异常。
    pub async fn run(&self, seed: &str) -> Result<ScanStats, PartisyError> {
        self.run_journal(seed, NoJournal).await
    }

    /// 带账本执行（裁定 4）：载入账本 → 构建恢复前沿（done 分片不重扫、
    /// failed 重入队）→ 运行 → 每分片终态落盘。统计自账本初值起算
    /// （`shards_failed` 计本轮新增失败）。
    ///
    /// # Errors
    /// sink 失败（fail-fast）、账本损坏/IO 或 worker 池 join 异常。
    pub async fn run_journal<J: ScanJournal + 'static>(
        &self,
        seed: &str,
        journal: J,
    ) -> Result<ScanStats, PartisyError> {
        let prior = journal.load().await?;

        // 恢复前沿（裁定 4）：done 分片不重扫——其未完成子分片与 failed
        // 分片构成初始队列（seen 去重）；全新扫描 = 单 seed。
        let mut queue = VecDeque::new();
        let counters = Counters::default();
        let mut state = JournalState {
            seed: seed.to_owned(),
            done: Vec::new(),
            failed: Vec::new(),
            entries: 0,
            dirs: 0,
        };
        if let Some(st) = &prior {
            let done: BTreeSet<&str> = st.done.iter().map(|d| d.dir.as_str()).collect();
            counters.set_shards_done(st.done.len() as u64);
            counters.set_entries(st.entries);
            counters.set_dirs(st.dirs);
            state.done = st.done.clone();
            state.entries = st.entries;
            state.dirs = st.dirs;
            if !done.contains(seed) {
                queue.push_back(seed.to_owned());
            }
            let mut seen: BTreeSet<String> = queue.iter().cloned().collect();
            for rec in &st.done {
                for sub in &rec.subdirs {
                    if !done.contains(sub.as_str()) && seen.insert(sub.clone()) {
                        queue.push_back(sub.clone());
                    }
                }
            }
            for (dir, _err) in &st.failed {
                if !done.contains(dir.as_str()) && seen.insert(dir.clone()) {
                    queue.push_back(dir.clone());
                }
            }
        } else {
            queue.push_back(seed.to_owned());
        }

        let initial = queue.len().max(1); // 恢复重排 + seed（seed 不在重排队列时也计）
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        for shard in &queue {
            let _ = tx.send(shard.clone());
        }
        let net = Arc::new(PoolNet {
            tx,
            rx: tokio::sync::Mutex::new(rx),
            pending: AtomicUsize::new(initial),
            abort: AtomicBool::new(false),
        });
        let counters = Arc::new(counters);
        let state = Arc::new(Mutex::new(state));
        let journal = Arc::new(journal);
        let opts = Arc::new(self.opts.clone());
        let seed_owned = seed.to_owned();

        // 监控者：worker 一有终态错误（含 panic 的 JoinError）立即置 abort，
        // 让其余 worker 从 tick 退出——不等顺序 join，杜绝悬挂窗口
        let (err_tx, mut err_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut monitors = Vec::with_capacity(self.opts.concurrency.max(1));
        for _ in 0..self.opts.concurrency.max(1) {
            let handle = tokio::spawn(worker_loop(
                net.clone(),
                self.source.clone(),
                self.sink.clone(),
                opts.clone(),
                counters.clone(),
                journal.clone(),
                state.clone(),
                seed_owned.clone(),
            ));
            let net_m = net.clone();
            let err_tx_m = err_tx.clone();
            monitors.push(tokio::spawn(async move {
                match handle.await {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        net_m.abort.store(true, Ordering::SeqCst);
                        let _ = err_tx_m.send(e);
                    }
                    Err(je) => {
                        net_m.abort.store(true, Ordering::SeqCst);
                        let _ = err_tx_m.send(fatal_io(format!("scan worker join: {je}")));
                    }
                }
            }));
        }
        drop(err_tx);
        for m in monitors {
            m.await.ok();
        }
        match err_rx.recv().await {
            Some(e) => Err(e),
            None => Ok(counters.snapshot()),
        }
    }
}

/// worker 主循环（裁定 2/3/4/5）：取分片 → 处理 → 记账落盘 → 计数收尾。
#[allow(clippy::too_many_arguments)]
async fn worker_loop<S: ListSource, K: EntrySink, J: ScanJournal>(
    net: Arc<PoolNet>,
    source: Arc<S>,
    sink: Arc<K>,
    opts: Arc<ScanOpts>,
    counters: Arc<Counters>,
    journal: Arc<J>,
    state: Arc<Mutex<JournalState>>,
    seed: String,
) -> Result<(), PartisyError> {
    while let Some(shard) = net.next_shard().await {
        let outcome = process_shard(&source, &sink, &opts, counters.as_ref(), &shard).await;
        match outcome {
            Ok(ShardOutcome::Done { subdirs }) => {
                counters.shards_done.fetch_add(1, Ordering::Relaxed);
                // 记账 + 落盘（裁定 4：每分片终态一次，先于子分片发现传播；
                // 锁不跨 await）
                {
                    let mut st = state.lock().await;
                    st.done.push(ShardDone {
                        dir: shard.clone(),
                        subdirs: subdirs.clone(),
                    });
                    st.entries = counters.entries.load(Ordering::Relaxed);
                    st.dirs = counters.dirs.load(Ordering::Relaxed);
                    let snapshot = st.clone();
                    drop(st);
                    journal.save(&snapshot).await?;
                }
                if let Some(cb) = &opts.progress {
                    cb(counters.snapshot());
                }
                net.enqueue(subdirs); // BFS 自适应分裂入队（先加 pending）
                net.shard_done();
            }
            Ok(ShardOutcome::Failed { error }) => {
                net.shard_done();
                counters.shards_failed.fetch_add(1, Ordering::Relaxed);
                {
                    let mut st = state.lock().await;
                    st.failed.push((shard.clone(), error));
                    let snapshot = st.clone();
                    drop(st);
                    journal.save(&snapshot).await?;
                }
            }
            Err(e) => {
                // fail-fast（裁定 5）：置 abort，全池在下一 tick 退出
                net.shard_done();
                net.abort.store(true, Ordering::SeqCst);
                return Err(e);
            }
        }
        // failpoint（SPEC 契约 §3）：分片完成注入点（测试崩溃矩阵用）
        if crate::failpoint::check("scan.shard_done") {
            panic!("failpoint: scan.shard_done @ {seed}");
        }
    }
    Ok(())
}

/// 处理单分片（裁定 2/5/8）：列单层（失败重试 1 次）→ 文件分批交 sink。
/// 失败重试耗尽 = [`ShardOutcome::Failed`]，不传播；子分片交 worker_loop
/// 入队并记账。
async fn process_shard<S: ListSource, K: EntrySink>(
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
        // failpoint（SPEC 契约 §3）：sink 前注入点——账本尚未记录本分片，
        // crash 后按 pending 重扫（不丢数据，sink 须幂等）
        if crate::failpoint::check("scan.before_sink") {
            panic!("failpoint: scan.before_sink @ {shard}");
        }
        sink.apply(shard, batch).await?;
    }
    counters
        .entries
        .fetch_add(files.len() as u64, Ordering::Relaxed);

    Ok(ShardOutcome::Done { subdirs })
}
