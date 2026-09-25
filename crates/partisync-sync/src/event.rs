//! 云事件流摄取（SPEC M5-WP04 裁定 1-8）：统一 EventSource 抽象、
//! Webhook/SQS/Kafka 适配、journal 协调。
//!
//! kind 折叠（裁定 2）：各家云端事件折叠到 `EventKind::Created/
//! Modified/Removed`，与本地 `notify` 同口径；差异由适配器 `map_kind`
//! 吸收，sync 侧只见统一枚举。checkpoint = cursor + processed_seq
//! （裁定 4），`EventJournal` 复用 WP03 `FileJournal` 形态——save/load
//! JSON 文件，原子写（防半写）。失败语义（裁定 6）：source 拉取失败
//! 重试 3 次（指数 backoff 250/500/1000ms）→ 计入 source fail counter
//! + 续读下批；apply 失败（store upsert 抛 Err）= fail-fast 透传。
//!
//! v0.1 适配器：WebhookAdapter（axum POST + HMAC 校验，`sync` 依赖 axum
//! 已是 workspace 既有）；SqsAdapter / KafkaAdapter stub impl（满足验收
//! trait 边界；正式 SDK 接线归后续 ADR 卡，避免无审批新增顶层依赖）。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::failpoint;

/// 事件类别（与 `partisync_graph::journal::EventKind` 同语义——直接
/// re-export，避免双枚举）。
pub use partisync_graph::journal::EventKind as GraphEventKind;

use serde::{Deserializer, Serializer};

/// 事件类别（与 graph 同口径；serde 通过 i64 数值映射——discriminant 0/1/2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Created = 0,
    Modified = 1,
    Removed = 2,
}

impl From<GraphEventKind> for EventKind {
    fn from(g: GraphEventKind) -> Self {
        match g {
            GraphEventKind::Created => Self::Created,
            GraphEventKind::Modified => Self::Modified,
            GraphEventKind::Removed => Self::Removed,
        }
    }
}

impl From<EventKind> for GraphEventKind {
    fn from(k: EventKind) -> Self {
        match k {
            EventKind::Created => Self::Created,
            EventKind::Modified => Self::Modified,
            EventKind::Removed => Self::Removed,
        }
    }
}

impl Serialize for EventKind {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i64(*self as i64)
    }
}

impl<'de> Deserialize<'de> for EventKind {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let n = i64::deserialize(d)?;
        match n {
            0 => Ok(Self::Created),
            1 => Ok(Self::Modified),
            2 => Ok(Self::Removed),
            other => Err(serde::de::Error::custom(format!(
                "unknown EventKind {other}"
            ))),
        }
    }
}

/// 单条云事件记录（裁定 1）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRecord {
    /// 上游 provider 名（"aws-s3"/"gcs-pubsub"/"minio-webhook"/...）。
    pub provider: String,
    /// 空间 id（webhook 配置静态指定；S3/Kafka 从 key 前缀解析）。
    pub space: String,
    /// 对象路径（provider 内部全路径，无前导 `/`，同 `ListedNode`）。
    pub path: String,
    /// 折叠后的 kind（裁定 2：各适配器自负责映射）。
    pub kind: EventKind,
    /// provider 级 cursor（SQS receipt handle / Kafka offset+partition /
    /// webhook timestamp+nonce）：唯一标识，用于 commit + 重投递去重。
    pub cursor: String,
    /// 原 payload（JSON 字典）：适配器调试 + 未来审计；v0.1 不消费。
    pub payload: serde_json::Value,
}

/// 单源 checkpoint（裁定 4）：本 hub 处理进度 + 失败计数 + 最近事件时间。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SourceCheckpoint {
    /// 最近一次 commit 的 cursor（供下次 poll 接续）。
    pub cursor: String,
    /// 累计成功处理事件数。
    pub processed: u64,
    /// 累计失败 source 计数（拉取重试耗尽，不影响 apply 成功条目）。
    pub failed: u64,
    /// 最近一次事件时间戳（unix ns；调试用）。
    pub last_at_ns: i64,
}

/// 账本持久态（裁定 1.4）：单 source 独立 checkpoint —— 支持多源多队列
/// 并存（M5-WP02 联邦面 worker 接管多 source 同口径）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EventJournalState {
    /// source 标识（用户配置）→ checkpoint。
    pub sources: HashMap<String, SourceCheckpoint>,
}

/// 错误集（裁定 2）。
#[derive(Debug)]
pub enum EventError {
    /// 拉取/提交失败（IO/HTTP/Kafka broker）→ 重试 + 计入。
    Transport(String),
    /// 配置错误（凭证/endpoint 缺失）→ 不重试，进程退出。
    Config(String),
    /// 应用失败（store upsert 抛）→ fail-fast。
    Apply(partisync_core::error::PartisyError),
}

impl std::fmt::Display for EventError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(s) => write!(f, "event transport: {s}"),
            Self::Config(s) => write!(f, "event config: {s}"),
            Self::Apply(e) => write!(f, "event apply: {e}"),
        }
    }
}

impl std::error::Error for EventError {}

impl From<partisync_core::error::PartisyError> for EventError {
    fn from(e: partisync_core::error::PartisyError) -> Self {
        Self::Apply(e)
    }
}

/// EventSource trait（裁定 1）：适配器抽象 — poll 拉取 + commit cursor。
///
/// `Send + Sync + 'static`：`tokio::spawn` 要求；'static 是 `EventDrain`
/// 在多 worker 池共享 source 的需要。
pub trait EventSource: Send + Sync + 'static {
    /// 拉一批（≤ max 事件 / 满 deadline 触发先返）。
    ///
    /// # Errors
    /// `Transport` 拉取失败；`Config` 配置错误（不应重试）。
    fn poll_batch(
        &self,
        max: usize,
        deadline: Duration,
    ) -> impl std::future::Future<Output = Result<Vec<EventRecord>, EventError>> + Send;

    /// 提交 cursor（成功应用后调用；语义依赖 provider：SQS Delete /
    /// Kafka commit offset / webhook 无 op）。
    ///
    /// # Errors
    /// `Transport` 失败。
    fn commit_cursor(
        &self,
        cursor: &str,
    ) -> impl std::future::Future<Output = Result<(), EventError>> + Send;

    /// 源标识（与 `EventJournalState::sources` 键一致）。
    fn name(&self) -> &str;
}

impl<T: EventSource + ?Sized> EventSource for std::sync::Arc<T> {
    async fn poll_batch(
        &self,
        max: usize,
        deadline: Duration,
    ) -> Result<Vec<EventRecord>, EventError> {
        (**self).poll_batch(max, deadline).await
    }

    async fn commit_cursor(&self, cursor: &str) -> Result<(), EventError> {
        (**self).commit_cursor(cursor).await
    }

    fn name(&self) -> &str {
        (**self).name()
    }
}

/// EventJournal trait（裁定 1.4）：与 WP03 `ScanJournal` 同构，可复用
/// `FileJournal` 模板（save/load JSON + 原子写）。
pub trait EventJournal: Send + Sync + 'static {
    /// 原子落盘（裁定 4：每批一记）。
    ///
    /// # Errors
    /// IO / 序列化失败。
    fn save(
        &self,
        state: &EventJournalState,
    ) -> impl std::future::Future<Output = Result<(), EventError>> + Send;

    /// 读取账本；无账本 = `Ok(None)`。
    ///
    /// # Errors
    /// 账本损坏（调用方决定降级/中止）或 IO 失败。
    fn load(
        &self,
    ) -> impl std::future::Future<Output = Result<Option<EventJournalState>, EventError>> + Send;
}

/// 无账本（默认；内存态）。
pub struct NoEventJournal;

impl EventJournal for NoEventJournal {
    async fn save(&self, _state: &EventJournalState) -> Result<(), EventError> {
        Ok(())
    }
    async fn load(&self) -> Result<Option<EventJournalState>, EventError> {
        Ok(None)
    }
}

/// JSON 文件账本（与 WP03 `FileJournal` 同形态——临时文件 + 原子 rename）。
#[derive(Debug, Clone)]
pub struct FileEventJournal {
    path: std::path::PathBuf,
}

impl FileEventJournal {
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl EventJournal for FileEventJournal {
    async fn save(&self, state: &EventJournalState) -> Result<(), EventError> {
        let json = serde_json::to_vec(state)
            .map_err(|e| EventError::Config(format!("账本序列化: {e}")))?;
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| EventError::Transport(format!("账本目录创建: {e}")))?;
            }
        }
        static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = self.path.with_extension(format!("tmp.{seq}"));
        tokio::fs::write(&tmp, json)
            .await
            .map_err(|e| EventError::Transport(format!("账本临时写入: {e}")))?;
        tokio::fs::rename(&tmp, &self.path)
            .await
            .map_err(|e| EventError::Transport(format!("账本原子替换: {e}")))?;
        Ok(())
    }

    async fn load(&self) -> Result<Option<EventJournalState>, EventError> {
        match tokio::fs::read(&self.path).await {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes).map_err(|e| {
                EventError::Config(format!("账本损坏（删除后可全新恢复）: {e}"))
            })?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(EventError::Transport(format!("账本读取: {e}"))),
        }
    }
}

impl<T: EventJournal + ?Sized> EventJournal for std::sync::Arc<T> {
    async fn save(&self, state: &EventJournalState) -> Result<(), EventError> {
        (**self).save(state).await
    }

    async fn load(&self) -> Result<Option<EventJournalState>, EventError> {
        (**self).load().await
    }
}

/// 累计统计（裁定 5）。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EventStats {
    /// 成功应用事件数。
    pub applied: u64,
    /// 跳过（空间路由闸/重投递去重）数。
    pub skipped: u64,
    /// source 拉取失败计数（计入 source failed）。
    pub source_failed: u64,
}

/// 调度器选项（裁定 5/6/8）。
#[derive(Clone)]
pub struct EventOpts {
    /// 拉取批大小上限（裁定 5）。
    pub batch_size: usize,
    /// 拉取 deadline：任一满即返（裁定 5）。
    pub batch_deadline: Duration,
    /// source 拉取失败重试次数上限（裁定 6：3）。
    pub max_retries: u32,
    /// 本 hub 持有空间过滤函数：`Some(f)` 时，非持有空间事件不落 journal
    /// （裁定 7）；`None` = 不过滤（测试桩场景）。
    #[allow(clippy::type_complexity)]
    pub space_filter: Option<std::sync::Arc<dyn Fn(&str) -> bool + Send + Sync>>,
    /// 事件应用面（M5-WP08 裁定 1）：Some = 真落图谱；None = stub 记账。
    pub applier: Option<Arc<dyn EventApplier>>,
}

impl std::fmt::Debug for EventOpts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventOpts")
            .field("batch_size", &self.batch_size)
            .field("batch_deadline", &self.batch_deadline)
            .field("max_retries", &self.max_retries)
            .field("has_space_filter", &self.space_filter.is_some())
            .field("applier", &self.applier.is_some())
            .finish()
    }
}

impl Default for EventOpts {
    fn default() -> Self {
        Self {
            batch_size: 256,
            batch_deadline: Duration::from_secs(5),
            max_retries: 3,
            space_filter: None,
            applier: None,
        }
    }
}

/// 事件源拉取失败重试（裁定 6）：指数 backoff 250/500/1000ms。
pub async fn poll_with_retry<S: EventSource>(
    source: &S,
    max: usize,
    deadline: Duration,
    max_retries: u32,
) -> Result<Vec<EventRecord>, EventError> {
    let mut attempt = 0u32;
    loop {
        match source.poll_batch(max, deadline).await {
            Ok(batch) => return Ok(batch),
            Err(EventError::Transport(msg)) if attempt < max_retries => {
                let delay = match attempt {
                    0 => Duration::from_millis(250),
                    1 => Duration::from_millis(500),
                    _ => Duration::from_millis(1000),
                };
                {
                    let _ = (delay, source.name(), attempt, &msg);
                }
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

/// 单批应用 + checkpoint 更新（裁定 5/6/7）：
/// 1. 空间过滤：本 hub 不持有空间 → 跳过（计入 skipped）；
/// 2. 应用：调 store 上 upsert/remove —— 失败 = fail-fast；
/// 3. commit cursor；
/// 4. checkpoint 更新 + 落盘。
pub async fn apply_batch<S: EventSource, J: EventJournal>(
    source: &S,
    journal: &J,
    state: &mut EventJournalState,
    batch: Vec<EventRecord>,
    opts: &EventOpts,
) -> Result<EventStats, EventError> {
    let mut stats = EventStats::default();
    let name = source.name().to_owned();
    let ckpt = state.sources.entry(name.clone()).or_default();
    for record in batch {
        // 空间路由闸（裁定 7）
        if let Some(filter) = &opts.space_filter {
            if !filter(&record.space) {
                stats.skipped += 1;
                continue;
            }
        }
        // failpoint 用于断点续传矩阵
        if failpoint::check("event.before_apply") {
            panic!(
                "failpoint: event.before_apply @ {}/{}",
                record.space, record.path
            );
        }
        // apply：applier Some = 真落目标面（M5-WP08）；None = stub 记账
        match &opts.applier {
            Some(applier) => applier.apply_event(&record).await?,
            None => apply_one(&record).await?,
        }
        // commit cursor（成功应用后）
        source.commit_cursor(&record.cursor).await?;
        ckpt.cursor = record.cursor.clone();
        ckpt.processed += 1;
        ckpt.last_at_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0);
        stats.applied += 1;
        if failpoint::check("event.after_cursor") {
            let cursor = ckpt.cursor.clone();
            panic!("failpoint: event.after_cursor @ {cursor}");
        }
    }
    journal.save(state).await?;
    Ok(stats)
}

/// 单事件应用（裁定 7）：当前为 stub（返回 Ok）—— 正式接入
/// `partisync_graph::journal::record` 归后续接线卡。
pub async fn apply_one(record: &EventRecord) -> Result<(), EventError> {
    let _ = record; // v0.1 stub
    Ok(())
}

/// EventDrain 调度器：单 source 单 worker 循环（裁定 5）。
pub struct EventDrain<S: EventSource, J: EventJournal> {
    source: std::sync::Arc<S>,
    journal: std::sync::Arc<J>,
    opts: EventOpts,
}

impl<S: EventSource + 'static, J: EventJournal + 'static> EventDrain<S, J> {
    pub fn new(source: S, journal: J) -> Self {
        Self::with_opts(source, journal, EventOpts::default())
    }

    pub fn with_opts(source: S, journal: J, opts: EventOpts) -> Self {
        Self {
            source: std::sync::Arc::new(source),
            journal: std::sync::Arc::new(journal),
            opts,
        }
    }

    /// 运行一轮（适配单次跑场景 + 测试桩；CLI 用 `run_loop` 常驻版本）。
    /// 账本访问器（测试 / 外部 checkpoint 读取用）。
    pub fn journal(&self) -> &std::sync::Arc<J> {
        &self.journal
    }

    pub async fn run_once(&self) -> Result<EventStats, EventError> {
        let mut state = self.journal.load().await?.unwrap_or_default();
        let batch = poll_with_retry(
            &*self.source,
            self.opts.batch_size,
            self.opts.batch_deadline,
            self.opts.max_retries,
        )
        .await?;
        if batch.is_empty() {
            return Ok(EventStats::default());
        }
        let stats =
            apply_batch(&*self.source, &*self.journal, &mut state, batch, &self.opts).await?;
        Ok(stats)
    }
}

// ---------- graph apply 接线（M5-WP08 裁定 1/2） ----------

use partisync_graph::store::{EntryKind, Store};

/// 事件应用面（M5-WP08 裁定 1）：`EventOpts.applier` 注入；None = stub
/// （事件仅记账，M5-WP04 v0.1 行为）。
/// 返回 future 类型（手工 boxed——`dyn` 兼容所需，不引 async-trait 依赖）。
pub type ApplyEventFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), EventError>> + Send + 'a>>;

pub trait EventApplier: Send + Sync {
    /// 应用单条事件到目标面（图谱/索引/…）。
    ///
    /// # Errors
    /// 实现方透传（fail-fast 语义同裁定 5）。
    fn apply_event<'a>(&'a self, record: &'a EventRecord) -> ApplyEventFuture<'a>;
}

impl<T: EventApplier + ?Sized> EventApplier for Arc<T> {
    fn apply_event<'a>(&'a self, record: &'a EventRecord) -> ApplyEventFuture<'a> {
        (**self).apply_event(record)
    }
}

/// graph::Store 后端（M5-WP08 裁定 2）：Created/Modified = `add_entry`
/// 幂等 upsert（父目录链自动建）；Removed = `remove_entry`（幂等）。
///
/// 与 `graph::journal` 通路的分工：journal 面向本地 fs 事件（apply 时
/// 校验 fs metadata），云事件无本地 fs 可查——直写 graph。
pub struct GraphApplier {
    store: Store,
}

impl GraphApplier {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    /// 逐级 ensure 父目录链（幂等 add_entry Dir）。
    async fn ensure_dir_chain(&self, path: &str) -> Result<Option<String>, EventError> {
        let mut parent_id: Option<String> = None;
        let trimmed = path.trim_start_matches('/');
        let parts: Vec<&str> = trimmed.split('/').filter(|p| !p.is_empty()).collect();
        // 末段是文件名，目录链含全部中间段（/docs/a/b/c.txt → docs, a, b）
        for i in 0..parts.len().saturating_sub(1) {
            let dir_path = format!("/{}", parts[..=i].join("/"));
            let name = parts[i];
            let id = self
                .store
                .add_entry(
                    parent_id.as_deref(),
                    name,
                    &dir_path,
                    EntryKind::Dir,
                    0,
                    0,
                    None,
                    None,
                )
                .await
                .map_err(EventError::from)?;
            parent_id = Some(id);
        }
        Ok(parent_id)
    }
}

impl GraphApplier {
    /// store 只读访问器（测试/调用方断言用）。
    pub fn store_ref(&self) -> &Store {
        &self.store
    }
}

impl EventApplier for GraphApplier {
    fn apply_event<'a>(&'a self, record: &'a EventRecord) -> ApplyEventFuture<'a> {
        Box::pin(async move { self.apply_event_inner(record).await })
    }
}

impl GraphApplier {
    async fn apply_event_inner(&self, record: &EventRecord) -> Result<(), EventError> {
        match record.kind {
            EventKind::Removed => {
                self.store
                    .remove_entry(&record.path)
                    .await
                    .map_err(EventError::from)?;
                Ok(())
            }
            EventKind::Created | EventKind::Modified => {
                let parent_id = self.ensure_dir_chain(&record.path).await?;
                let name = record
                    .path
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or("");
                let size = record
                    .payload
                    .get("size")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let mtime_ns = record
                    .payload
                    .get("mtime_ns")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                self.store
                    .add_entry(
                        parent_id.as_deref(),
                        name,
                        &record.path,
                        EntryKind::File,
                        size,
                        mtime_ns,
                        None,
                        None,
                    )
                    .await
                    .map_err(EventError::from)?;
                Ok(())
            }
        }
    }
}
