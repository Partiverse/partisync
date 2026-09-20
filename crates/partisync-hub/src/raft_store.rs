//! Raft 存储适配器（SPEC M3-WP02 T03，裁定 2/3/7）：openraft 0.9
//! storage-v2（[`openraft::RaftLogStorage`]/[`openraft::RaftStateMachine`]）
//! → fjall keyspace。
//!
//! 物理布局（裁定 3：组 keyspace 按 `{pid}` 前缀收敛为共享实例——组数 ×
//! log/sm 的 keyspace 叠加会重演 WP01 open=12.8s）：
//!
//! | keyspace | 键 | 内容 |
//! |---|---|---|
//! | `r-log`  | `[pid 8B][index 8B]` | 组 {pid} 日志条目 `[term 8B][tag 1B][payload]` |
//! | `r-sm`   | `[pid][0x00]` / `[pid][0x01][index 8B]` | 应用指针+membership / 应用数据节 |
//! | `r-meta` | `[pid][0x00..=0x04]` | vote / committed / purged / 快照 meta / 快照 blob |
//!
//! 持久化语义（关门 KPI「已 ACK 写入不丢」）：raft 协议 ACK = 落盘。
//! fjall journal 默认停留在用户态 BufWriter（不设 durability 的提交连
//! `kill -9` 都扛不住），全部协议写路径显式
//! `OwnedWriteBatch::durability(Some(PersistMode::SyncData))`（每批一次
//! fdatasync）。
//!
//! 状态机（裁定 7）：entry 行与 children 行是同组状态机的两个 section；
//! 本模块交付通用存储面——[`RaftStateMachineStore::apply`] 把 AppData
//! 载荷按 log index 落进数据节，业务应用（写 m-entry/m-child 平面）归
//! replica 层（T05 接线）。

// openraft storage-v2 的错误类型固定为 `StorageError<u64>`（~224B，内含
// AnyError/backtrace 字段）——体积由 trait API 决定，非本模块可收缩。
#![allow(clippy::result_large_err)]

use std::fmt::Debug;
use std::io::Cursor;
use std::ops::{Bound, RangeBounds};
use std::sync::Arc;

use fjall::{Database, Keyspace, KeyspaceCreateOptions, OwnedWriteBatch, PersistMode};
use openraft::storage::{LogFlushed, RaftLogStorage, RaftStateMachine};
use openraft::{
    declare_raft_types, AnyError, BasicNode, CommittedLeaderId, Entry, EntryPayload, ErrorSubject,
    ErrorVerb, LogId, LogState, Membership, OptionalSend, RaftLogReader, RaftSnapshotBuilder,
    Snapshot, SnapshotMeta, StorageError, StorageIOError, StoredMembership, Vote,
};
use serde::{Deserialize, Serialize};

/// Raft 客户端载荷（裁定 4：newtype-over-bytes，紧凑编码不过 JSON；serde
/// 仅为满足 openraft `OptionalSerde` bound，存储路径不走 JSON）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HubData(pub Vec<u8>);

/// 状态机应用回执（回显应用载荷）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HubResponse(pub Vec<u8>);

declare_raft_types! {
    /// Hub raft 类型配置：NodeId=u64 / Node=BasicNode /
    /// SnapshotData=Cursor<Vec<u8>> 等沿用 openraft 默认。
    pub HubTypeConfig:
        D = HubData,
        R = HubResponse,
}

/// raft 日志共享 keyspace（全部组按 pid 前缀共用）。
pub const KS_RAFT_LOG: &str = "r-log";
/// raft 状态机共享 keyspace。
pub const KS_RAFT_SM: &str = "r-sm";
/// raft 硬状态共享 keyspace（vote/committed/purged/快照）。
pub const KS_RAFT_META: &str = "r-meta";

/// 状态机 meta 行内容（meta 行与快照 blob 头共用此布局）。
///
/// `[applied_present 1B][applied 24B][mem_log_present 1B][mem_log 24B]
/// [mlen 4B][membership json]`——presence 字节显式区分「无指针」与
/// `(term=0,index=0)` 合法指针（openraft 套件在 index 0 建 log）；
/// `mem_log` 是 membership 条目自身的 log id（`StoredMembership::log_id`
/// 语义，≠ last applied）；LogId 定长 24B（term+leader_node+index）。
#[derive(Debug, Clone)]
struct SmMetaRow {
    applied: Option<LogId<u64>>,
    mem_log_id: Option<LogId<u64>>,
    membership: Membership<u64, BasicNode>,
}

/// 快照 blob 解码产物：`(meta 行, (index, data) 行集)`。
type SmBlob = (SmMetaRow, Vec<(u64, Vec<u8>)>);

/// meta 行/blob 头的编码。
fn encode_meta_prefix(row: &SmMetaRow, out: &mut Vec<u8>) -> Result<(), StorageError<u64>> {
    match row.applied {
        Some(lid) => {
            out.push(1);
            out.extend_from_slice(&encode_log_id(&lid));
        }
        None => out.extend_from_slice(&[0u8; 25]),
    }
    match row.mem_log_id {
        Some(lid) => {
            out.push(1);
            out.extend_from_slice(&encode_log_id(&lid));
        }
        None => out.extend_from_slice(&[0u8; 25]),
    }
    let json = serde_json::to_vec(&row.membership)
        .map_err(|_| corrupt(ErrorSubject::StateMachine, "membership json encode"))?;
    out.extend_from_slice(&(json.len() as u32).to_be_bytes());
    out.extend_from_slice(&json);
    Ok(())
}

/// meta 行/blob 头的解码（[`encode_meta_prefix`] 逆变换）。
fn decode_meta_prefix(blob: &[u8]) -> Result<(SmMetaRow, usize), StorageError<u64>> {
    let short = |what| corrupt(ErrorSubject::StateMachine, what);
    if blob.len() < 54 {
        return Err(short("meta prefix too short"));
    }
    let present = |b: u8| b != 0;
    let applied = if present(blob[0]) {
        Some(decode_log_id(&blob[1..25]).ok_or_else(|| short("applied corrupted"))?)
    } else {
        None
    };
    let mem_log_id = if present(blob[25]) {
        Some(decode_log_id(&blob[26..50]).ok_or_else(|| short("mem log id corrupted"))?)
    } else {
        None
    };
    let mut mlen = [0u8; 4];
    mlen.copy_from_slice(&blob[50..54]);
    let mlen = u32::from_be_bytes(mlen) as usize;
    if blob.len() < 54 + mlen {
        return Err(short("membership truncated"));
    }
    let membership: Membership<u64, BasicNode> = serde_json::from_slice(&blob[54..54 + mlen])
        .map_err(|_| short("membership json corrupted"))?;
    Ok((
        SmMetaRow {
            applied,
            mem_log_id,
            membership,
        },
        54 + mlen,
    ))
}

/// `r-sm` 应用指针 + membership 行（组内唯一）。
const SM_META: u8 = 0x00;
/// `r-sm` 应用数据节（后接 log index）。
const SM_DATA: u8 = 0x01;
/// `r-meta` vote 行。
const RM_VOTE: u8 = 0x00;
/// `r-meta` committed 指针行。
const RM_COMMITTED: u8 = 0x01;
/// `r-meta` last-purged 行。
const RM_PURGED: u8 = 0x02;
/// `r-meta` 快照 meta 行（serde_json）。
const RM_SNAP_META: u8 = 0x03;
/// `r-meta` 快照 blob 行。
const RM_SNAP_BLOB: u8 = 0x04;

/// 日志条目载荷 tag：blank。
const TAG_BLANK: u8 = 0x00;
/// 日志条目载荷 tag：normal（HubData 原始字节）。
const TAG_NORMAL: u8 = 0x01;
/// 日志条目载荷 tag：membership（serde_json）。
const TAG_MEMBERSHIP: u8 = 0x02;

/// 组前缀键：`[pid 8B]`（`r-sm`/`r-meta` 组内行的公共前缀）。
#[must_use]
pub fn group_prefix(pid: u64) -> [u8; 8] {
    pid.to_be_bytes()
}

/// 日志键：`[pid 8B][index 8B]`（组内按键序 = index 序）。
fn log_key(pid: u64, index: u64) -> [u8; 16] {
    let mut k = [0u8; 16];
    k[..8].copy_from_slice(&pid.to_be_bytes());
    k[8..].copy_from_slice(&index.to_be_bytes());
    k
}

/// 组内单例键：`[pid 8B][tag 1B]`。
fn group_key(pid: u64, tag: u8) -> Vec<u8> {
    let mut k = Vec::with_capacity(9);
    k.extend_from_slice(&pid.to_be_bytes());
    k.push(tag);
    k
}

/// 应用数据节键：`[pid 8B][0x01][index 8B]`。
fn sm_data_key(pid: u64, index: u64) -> Vec<u8> {
    let mut k = Vec::with_capacity(17);
    k.extend_from_slice(&pid.to_be_bytes());
    k.push(SM_DATA);
    k.extend_from_slice(&index.to_be_bytes());
    k
}

/// fjall 错误 → openraft [`StorageError::IO`]。
fn sto<T>(
    r: fjall::Result<T>,
    subject: ErrorSubject<u64>,
    verb: ErrorVerb,
) -> Result<T, StorageError<u64>> {
    r.map_err(|e| {
        let io = std::io::Error::other(e.to_string());
        StorageError::IO {
            source: StorageIOError::new(subject, verb, AnyError::new(&io)),
        }
    })
}

/// 数据损坏（长度/编码不合法）→ [`StorageError::IO`]。
fn corrupt(subject: ErrorSubject<u64>, what: &'static str) -> StorageError<u64> {
    let io = std::io::Error::other(what);
    StorageError::IO {
        source: StorageIOError::new(subject, ErrorVerb::Read, AnyError::new(&io)),
    }
}

/// 空应用指针哨兵（`term=0, index=0`——真实日志 index 从 1 起，见
/// [`raft_store::apply` 内不变量]）。
fn zero_log_id() -> LogId<u64> {
    LogId::new(CommittedLeaderId::new(0, 0), 0)
}

/// 编码 `LogId<u64>`：`[term 8B][leader_node 8B][index 8B]`——LogId 身份
/// 含 leader node（缺 node 会破坏 openraft progress 匹配，T05 演练实测）。
fn encode_log_id(lid: &LogId<u64>) -> [u8; 24] {
    let mut b = [0u8; 24];
    b[..8].copy_from_slice(&lid.leader_id.term.to_be_bytes());
    b[8..16].copy_from_slice(&lid.leader_id.node_id.to_be_bytes());
    b[16..].copy_from_slice(&lid.index.to_be_bytes());
    b
}

/// 解码 [`encode_log_id`] 的逆变换。
fn decode_log_id(b: &[u8]) -> Option<LogId<u64>> {
    if b.len() < 24 {
        return None;
    }
    let mut term = [0u8; 8];
    let mut node = [0u8; 8];
    let mut index = [0u8; 8];
    term.copy_from_slice(&b[..8]);
    node.copy_from_slice(&b[8..16]);
    index.copy_from_slice(&b[16..24]);
    Some(LogId::new(
        CommittedLeaderId::new(u64::from_be_bytes(term), u64::from_be_bytes(node)),
        u64::from_be_bytes(index),
    ))
}

/// 日志条目值编码：`[term 8B][leader_node 8B][tag 1B][payload]`（index 在
/// 键中，不重复存；leader node 是 LogId 身份的一部分，见
/// [`encode_log_id`]）。
fn encode_entry(entry: &Entry<HubTypeConfig>) -> Result<Vec<u8>, StorageError<u64>> {
    let mut v = Vec::with_capacity(17 + 32);
    v.extend_from_slice(&entry.log_id.leader_id.term.to_be_bytes());
    v.extend_from_slice(&entry.log_id.leader_id.node_id.to_be_bytes());
    match &entry.payload {
        EntryPayload::Blank => {
            v.push(TAG_BLANK);
        }
        EntryPayload::Normal(d) => {
            v.push(TAG_NORMAL);
            v.extend_from_slice(&d.0);
        }
        EntryPayload::Membership(m) => {
            v.push(TAG_MEMBERSHIP);
            let json = serde_json::to_vec(m)
                .map_err(|_| corrupt(ErrorSubject::Logs, "membership json encode"))?;
            v.extend_from_slice(&json);
        }
    }
    Ok(v)
}

/// 日志条目值解码（[`encode_entry`] 逆变换；index 取自键）。
fn decode_entry(index: u64, v: &[u8]) -> Result<Entry<HubTypeConfig>, StorageError<u64>> {
    let subject = ErrorSubject::Log(zero_log_id());
    if v.len() < 17 {
        return Err(corrupt(subject, "log entry value too short"));
    }
    let mut term = [0u8; 8];
    term.copy_from_slice(&v[..8]);
    let mut node = [0u8; 8];
    node.copy_from_slice(&v[8..16]);
    let term = u64::from_be_bytes(term);
    let node = u64::from_be_bytes(node);
    let payload = match v[16] {
        TAG_BLANK => EntryPayload::Blank,
        TAG_NORMAL => EntryPayload::Normal(HubData(v[17..].to_vec())),
        TAG_MEMBERSHIP => {
            let m: Membership<u64, BasicNode> = serde_json::from_slice(&v[17..])
                .map_err(|_| corrupt(subject, "membership json corrupted"))?;
            EntryPayload::Membership(m)
        }
        _ => return Err(corrupt(subject, "unknown payload tag")),
    };
    Ok(Entry {
        log_id: LogId::new(CommittedLeaderId::new(term, node), index),
        payload,
    })
}

/// 组内全部 keyspace 的共享句柄（`Keyspace`/`Database` 为 Arc 内核，廉价克隆）。
#[derive(Clone)]
struct RaftStoreShared {
    db: Database,
    log: Keyspace,
    sm: Keyspace,
    meta: Keyspace,
    pid: u64,
}

impl std::fmt::Debug for RaftStoreShared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RaftStoreShared")
            .field("pid", &self.pid)
            .finish()
    }
}

impl RaftStoreShared {
    /// 协议写批（统一 fdatasync durability——见模块文档）。
    fn batch(&self, capacity: usize) -> OwnedWriteBatch {
        OwnedWriteBatch::with_capacity(self.db.clone(), capacity)
            .durability(Some(PersistMode::SyncData))
    }
}

/// 打开组 `{pid}` 的 raft 存储（日志面 + 状态机面），挂在既有 Database 上。
///
/// # Errors
/// keyspace 创建失败。
pub fn open_raft_stores(
    db: &Database,
    pid: u64,
) -> fjall::Result<(RaftLogStore, RaftStateMachineStore)> {
    let shared = RaftStoreShared {
        db: db.clone(),
        log: db.keyspace(KS_RAFT_LOG, KeyspaceCreateOptions::default)?,
        sm: db.keyspace(KS_RAFT_SM, KeyspaceCreateOptions::default)?,
        meta: db.keyspace(KS_RAFT_META, KeyspaceCreateOptions::default)?,
        pid,
    };
    let shared = Arc::new(shared);
    Ok((RaftLogStore(shared.clone()), RaftStateMachineStore(shared)))
}

/// [`RaftLogStorage`] 面（日志 + 硬状态）。
#[derive(Debug, Clone)]
pub struct RaftLogStore(Arc<RaftStoreShared>);

/// [`RaftLogReader`] 面（复制流读日志）。
#[derive(Debug, Clone)]
pub struct RaftLogReaderStore(Arc<RaftStoreShared>);

/// [`RaftStateMachine`] 面（应用指针 + 数据节 + 快照）。
#[derive(Debug, Clone)]
pub struct RaftStateMachineStore(Arc<RaftStoreShared>);

/// 快照构建器（读状态机全量 → blob）。
#[derive(Debug, Clone)]
pub struct RaftSnapshotBuilderStore(Arc<RaftStoreShared>);

impl RaftLogStore {
    /// 读 `[pid][RM_PURGED]` 行。
    fn read_purged(&self) -> Result<Option<LogId<u64>>, StorageError<u64>> {
        match sto(
            self.0.meta.get(group_key(self.0.pid, RM_PURGED)),
            ErrorSubject::Vote,
            ErrorVerb::Read,
        )? {
            None => Ok(None),
            Some(g) => decode_log_id(&g)
                .map(Some)
                .ok_or_else(|| corrupt(ErrorSubject::Vote, "purged marker corrupted")),
        }
    }

    /// 读 `[pid][RM_COMMITTED]` 行。
    fn read_committed_row(&self) -> Result<Option<LogId<u64>>, StorageError<u64>> {
        match sto(
            self.0.meta.get(group_key(self.0.pid, RM_COMMITTED)),
            ErrorSubject::Vote,
            ErrorVerb::Read,
        )? {
            None => Ok(None),
            Some(g) => decode_log_id(&g)
                .map(Some)
                .ok_or_else(|| corrupt(ErrorSubject::Vote, "committed marker corrupted")),
        }
    }
}

/// `[start, end)` log index 区间 → fjall 键区间（同组前缀内）。
fn log_key_range(
    pid: u64,
    start: Bound<&u64>,
    end: Bound<&u64>,
) -> (Bound<Vec<u8>>, Bound<Vec<u8>>) {
    let lo = match start {
        Bound::Included(i) => Bound::Included(log_key(pid, *i).to_vec()),
        Bound::Excluded(i) => Bound::Excluded(log_key(pid, *i).to_vec()),
        Bound::Unbounded => Bound::Included(log_key(pid, 0).to_vec()),
    };
    let hi = match end {
        Bound::Included(i) => Bound::Excluded(log_key(pid, i.wrapping_add(1)).to_vec()),
        Bound::Excluded(i) => Bound::Excluded(log_key(pid, *i).to_vec()),
        Bound::Unbounded => Bound::Included(log_key(pid, u64::MAX).to_vec()),
    };
    (lo, hi)
}

/// 读 `[start, end)` 区间日志条目（[`RaftLogReader`] 的共享实现）。
fn try_get_log_entries_impl(
    shared: &RaftStoreShared,
    start: Bound<&u64>,
    end: Bound<&u64>,
) -> Result<Vec<Entry<HubTypeConfig>>, StorageError<u64>> {
    let (lo, hi) = log_key_range(shared.pid, start, end);
    let mut out = Vec::new();
    for guard in shared.log.range((lo, hi)) {
        let (k, v) = sto(guard.into_inner(), ErrorSubject::Logs, ErrorVerb::Read)?;
        if k.len() != 16 {
            return Err(corrupt(ErrorSubject::Logs, "log key length invalid"));
        }
        let mut idx = [0u8; 8];
        idx.copy_from_slice(&k[8..]);
        out.push(decode_entry(u64::from_be_bytes(idx), &v)?);
    }
    Ok(out)
}

impl RaftLogReader<HubTypeConfig> for RaftLogStore {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> Result<Vec<Entry<HubTypeConfig>>, StorageError<u64>> {
        try_get_log_entries_impl(&self.0, range.start_bound(), range.end_bound())
    }
}

impl RaftLogReader<HubTypeConfig> for RaftLogReaderStore {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> Result<Vec<Entry<HubTypeConfig>>, StorageError<u64>> {
        try_get_log_entries_impl(&self.0, range.start_bound(), range.end_bound())
    }
}

impl RaftLogStorage<HubTypeConfig> for RaftLogStore {
    type LogReader = RaftLogReaderStore;

    async fn get_log_state(&mut self) -> Result<LogState<HubTypeConfig>, StorageError<u64>> {
        let purged = self.read_purged()?;
        // 组前缀内末键：range `[pid|0] .. [pid+1|0)` 的末项（DoubleEnded 反向取 O(1)）
        let lo = Bound::Included(log_key(self.0.pid, 0).to_vec());
        let hi = Bound::Excluded(log_key(self.0.pid.wrapping_add(1), 0).to_vec());
        let last = self.0.log.range((lo, hi)).next_back();
        let last_log_id = match last {
            // term 由条目值携带（键只含 index）
            Some(guard) => {
                let (k, v) = sto(guard.into_inner(), ErrorSubject::Logs, ErrorVerb::Read)?;
                if k.len() != 16 {
                    return Err(corrupt(ErrorSubject::Logs, "log key length invalid"));
                }
                let mut idx = [0u8; 8];
                idx.copy_from_slice(&k[8..]);
                Some(decode_entry(u64::from_be_bytes(idx), &v)?.log_id)
            }
            None => purged,
        };
        Ok(LogState {
            last_purged_log_id: purged,
            last_log_id,
        })
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        RaftLogReaderStore(self.0.clone())
    }

    async fn save_vote(&mut self, vote: &Vote<u64>) -> Result<(), StorageError<u64>> {
        let json = serde_json::to_vec(vote)
            .map_err(|_| corrupt(ErrorSubject::Vote, "vote json encode"))?;
        let mut batch = self.0.batch(1);
        batch.insert(&self.0.meta, group_key(self.0.pid, RM_VOTE), json);
        sto(batch.commit(), ErrorSubject::Vote, ErrorVerb::Write)
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<u64>>, StorageError<u64>> {
        match sto(
            self.0.meta.get(group_key(self.0.pid, RM_VOTE)),
            ErrorSubject::Vote,
            ErrorVerb::Read,
        )? {
            None => Ok(None),
            Some(g) => {
                let vote: Vote<u64> = serde_json::from_slice(&g)
                    .map_err(|_| corrupt(ErrorSubject::Vote, "vote json corrupted"))?;
                Ok(Some(vote))
            }
        }
    }

    async fn save_committed(
        &mut self,
        committed: Option<LogId<u64>>,
    ) -> Result<(), StorageError<u64>> {
        let mut batch = self.0.batch(1);
        match committed {
            Some(lid) => batch.insert(
                &self.0.meta,
                group_key(self.0.pid, RM_COMMITTED),
                encode_log_id(&lid).to_vec(),
            ),
            None => batch.remove(&self.0.meta, group_key(self.0.pid, RM_COMMITTED)),
        }
        sto(batch.commit(), ErrorSubject::Vote, ErrorVerb::Write)
    }

    async fn read_committed(&mut self) -> Result<Option<LogId<u64>>, StorageError<u64>> {
        self.read_committed_row()
    }

    async fn append<I>(
        &mut self,
        entries: I,
        callback: LogFlushed<HubTypeConfig>,
    ) -> Result<(), StorageError<u64>>
    where
        I: IntoIterator<Item = Entry<HubTypeConfig>> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        let items: Vec<Entry<HubTypeConfig>> = entries.into_iter().collect();
        let mut batch = self.0.batch(items.len());
        for e in &items {
            let key = log_key(self.0.pid, e.log_id.index);
            batch.insert(&self.0.log, key.as_slice(), encode_entry(e)?);
        }
        sto(batch.commit(), ErrorSubject::Logs, ErrorVerb::Write)?;
        // 批提交已 fdatasync——ACK 语义（「已 ACK 写入不丢」）成立
        callback.log_io_completed(Ok(()));
        Ok(())
    }

    async fn truncate(&mut self, log_id: LogId<u64>) -> Result<(), StorageError<u64>> {
        // 删 `[log_id.index, ∞)`（含）：先收集后批删（同批原子）
        let (lo, hi) = log_key_range(self.0.pid, Bound::Included(&log_id.index), Bound::Unbounded);
        let mut keys = Vec::new();
        for guard in self.0.log.range((lo, hi)) {
            keys.push(sto(guard.into_inner(), ErrorSubject::Logs, ErrorVerb::Read)?.0);
        }
        let mut batch = self.0.batch(keys.len());
        for k in &keys {
            batch.remove(&self.0.log, k.as_slice());
        }
        sto(batch.commit(), ErrorSubject::Logs, ErrorVerb::Delete)
    }

    async fn purge(&mut self, log_id: LogId<u64>) -> Result<(), StorageError<u64>> {
        // 删 `[?, log_id.index]`（含）+ 记 last-purged 行（同批原子）
        let lo = Bound::Included(log_key(self.0.pid, 0).to_vec());
        let hi = Bound::Included(log_key(self.0.pid, log_id.index).to_vec());
        let mut keys = Vec::new();
        for guard in self.0.log.range((lo, hi)) {
            keys.push(sto(guard.into_inner(), ErrorSubject::Logs, ErrorVerb::Read)?.0);
        }
        let mut batch = self.0.batch(keys.len() + 1);
        for k in &keys {
            batch.remove(&self.0.log, k.as_slice());
        }
        batch.insert(
            &self.0.meta,
            group_key(self.0.pid, RM_PURGED),
            encode_log_id(&log_id).to_vec(),
        );
        sto(batch.commit(), ErrorSubject::Logs, ErrorVerb::Delete)
    }
}

/// 读状态机 meta 行（缺行 = 全新状态机）。
fn read_sm_meta(shared: &RaftStoreShared) -> Result<SmMetaRow, StorageError<u64>> {
    let Some(g) = sto(
        shared.sm.get(group_key(shared.pid, SM_META)),
        ErrorSubject::StateMachine,
        ErrorVerb::Read,
    )?
    else {
        return Ok(SmMetaRow {
            applied: None,
            mem_log_id: None,
            membership: Membership::default(),
        });
    };
    let (row, _) = decode_meta_prefix(&g)?;
    Ok(row)
}

/// 快照 blob 编码：meta 头 + 数据节全量（[`decode_blob`] 逆变换）。
fn encode_blob(meta: &SmMetaRow, rows: &[(u64, Vec<u8>)]) -> Result<Vec<u8>, StorageError<u64>> {
    let mut blob = Vec::new();
    encode_meta_prefix(meta, &mut blob)?;
    blob.extend_from_slice(&(rows.len() as u64).to_be_bytes());
    for (index, data) in rows {
        blob.extend_from_slice(&index.to_be_bytes());
        blob.extend_from_slice(&(data.len() as u32).to_be_bytes());
        blob.extend_from_slice(data);
    }
    Ok(blob)
}

/// 快照 blob 解码（[`encode_blob`] 逆变换）。
fn decode_blob(blob: &[u8]) -> Result<SmBlob, StorageError<u64>> {
    if blob.len() < 62 {
        return Err(corrupt(
            ErrorSubject::Snapshot(None),
            "snapshot blob too short",
        ));
    }
    let (meta, mut pos) = decode_meta_prefix(blob)?;
    let mut count = [0u8; 8];
    count.copy_from_slice(&blob[pos..pos + 8]);
    let count = u64::from_be_bytes(count) as usize;
    pos += 8;
    let mut rows = Vec::with_capacity(count);
    for _ in 0..count {
        if blob.len() < pos + 12 {
            return Err(corrupt(
                ErrorSubject::Snapshot(None),
                "snapshot row header truncated",
            ));
        }
        let mut idx = [0u8; 8];
        idx.copy_from_slice(&blob[pos..pos + 8]);
        let index = u64::from_be_bytes(idx);
        let mut dlen = [0u8; 4];
        dlen.copy_from_slice(&blob[pos + 8..pos + 12]);
        let dlen = u32::from_be_bytes(dlen) as usize;
        if blob.len() < pos + 12 + dlen {
            return Err(corrupt(
                ErrorSubject::Snapshot(None),
                "snapshot row truncated",
            ));
        }
        rows.push((index, blob[pos + 12..pos + 12 + dlen].to_vec()));
        pos += 12 + dlen;
    }
    Ok((meta, rows))
}

impl RaftStateMachineStore {
    /// 读数据节单行（replica 线性一致读通道；fjall 内部同步，与 raft apply
    /// 并发安全）。
    ///
    /// # Errors
    /// 引擎读取失败。
    pub async fn read_data_row(&self, index: u64) -> Result<Option<Vec<u8>>, StorageError<u64>> {
        match sto(
            self.0.sm.get(sm_data_key(self.0.pid, index)),
            ErrorSubject::StateMachine,
            ErrorVerb::Read,
        )? {
            None => Ok(None),
            Some(g) => Ok(Some(g.to_vec())),
        }
    }
}

impl RaftSnapshotBuilder<HubTypeConfig> for RaftSnapshotBuilderStore {
    async fn build_snapshot(&mut self) -> Result<Snapshot<HubTypeConfig>, StorageError<u64>> {
        let meta_row = read_sm_meta(&self.0)?;
        // 数据节全量（按键序 = index 序）
        let mut rows = Vec::new();
        for guard in self.0.sm.prefix(sm_data_prefix(self.0.pid)) {
            let (k, v) = sto(
                guard.into_inner(),
                ErrorSubject::StateMachine,
                ErrorVerb::Read,
            )?;
            if k.len() != 17 {
                return Err(corrupt(
                    ErrorSubject::StateMachine,
                    "sm data key length invalid",
                ));
            }
            let mut idx = [0u8; 8];
            idx.copy_from_slice(&k[9..]);
            rows.push((u64::from_be_bytes(idx), v.to_vec()));
        }
        let blob = encode_blob(&meta_row, &rows)?;
        let last_log_id = meta_row.applied;
        let meta = SnapshotMeta {
            last_log_id,
            last_membership: StoredMembership::new(
                meta_row.mem_log_id,
                meta_row.membership.clone(),
            ),
            // SnapshotId 仅要求可比较唯一；term-index 命名与 memstore 同口径
            snapshot_id: format!(
                "{}-{}",
                last_log_id.as_ref().map_or(0, |l| l.leader_id.term),
                last_log_id.as_ref().map_or(0, |l| l.index)
            ),
        };
        // 持久化当前快照（get_current_snapshot 依赖）
        let mut batch = self.0.batch(2);
        batch.insert(
            &self.0.meta,
            group_key(self.0.pid, RM_SNAP_META),
            serde_json::to_vec(&meta)
                .map_err(|_| corrupt(ErrorSubject::Snapshot(None), "snapshot meta json encode"))?,
        );
        batch.insert(
            &self.0.meta,
            group_key(self.0.pid, RM_SNAP_BLOB),
            blob.clone(),
        );
        sto(
            batch.commit(),
            ErrorSubject::Snapshot(None),
            ErrorVerb::Write,
        )?;
        Ok(Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(blob)),
        })
    }
}

/// 数据节组前缀：`[pid 8B][0x01]`。
fn sm_data_prefix(pid: u64) -> Vec<u8> {
    let mut k = Vec::with_capacity(9);
    k.extend_from_slice(&pid.to_be_bytes());
    k.push(SM_DATA);
    k
}

impl RaftStateMachine<HubTypeConfig> for RaftStateMachineStore {
    type SnapshotBuilder = RaftSnapshotBuilderStore;

    async fn applied_state(
        &mut self,
    ) -> Result<(Option<LogId<u64>>, StoredMembership<u64, BasicNode>), StorageError<u64>> {
        let row = read_sm_meta(&self.0)?;
        Ok((
            row.applied,
            StoredMembership::new(row.mem_log_id, row.membership),
        ))
    }

    async fn apply<I>(&mut self, entries: I) -> Result<Vec<HubResponse>, StorageError<u64>>
    where
        I: IntoIterator<Item = Entry<HubTypeConfig>> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        // 先读现有 meta（非 membership 条目保持 membership 与其 log id 不变）
        let mut meta_row = read_sm_meta(&self.0)?;
        let items: Vec<Entry<HubTypeConfig>> = entries.into_iter().collect();
        let mut last: Option<LogId<u64>> = None;
        let mut batch = self.0.batch(items.len() + 1);
        let mut responses = Vec::with_capacity(items.len());
        for e in &items {
            match &e.payload {
                EntryPayload::Blank => {}
                EntryPayload::Normal(d) => {
                    batch.insert(
                        &self.0.sm,
                        sm_data_key(self.0.pid, e.log_id.index),
                        d.0.as_slice(),
                    );
                }
                EntryPayload::Membership(m) => {
                    meta_row.membership = m.clone();
                    meta_row.mem_log_id = Some(e.log_id);
                }
            }
            responses.push(match &e.payload {
                EntryPayload::Normal(d) => HubResponse(d.0.clone()),
                _ => HubResponse(Vec::new()),
            });
            last = Some(e.log_id);
        }
        // 应用指针 + membership 同批落盘（持久化状态机：apply 返回前已 fdatasync）
        meta_row.applied =
            Some(last.ok_or_else(|| {
                corrupt(ErrorSubject::StateMachine, "apply with empty entry batch")
            })?);
        let mut row = Vec::new();
        encode_meta_prefix(&meta_row, &mut row)?;
        batch.insert(&self.0.sm, group_key(self.0.pid, SM_META), row);
        sto(batch.commit(), ErrorSubject::StateMachine, ErrorVerb::Write)?;
        Ok(responses)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        RaftSnapshotBuilderStore(self.0.clone())
    }

    async fn begin_receiving_snapshot(
        &mut self,
    ) -> Result<Box<Cursor<Vec<u8>>>, StorageError<u64>> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<u64, BasicNode>,
        snapshot: Box<Cursor<Vec<u8>>>,
    ) -> Result<(), StorageError<u64>> {
        let blob = snapshot.into_inner();
        let (meta_row, rows) = decode_blob(&blob)?;
        // 全量替换：删旧数据节 + 写新数据节 + meta + 快照行（同批原子）
        let mut old_keys = Vec::new();
        for guard in self.0.sm.prefix(sm_data_prefix(self.0.pid)) {
            old_keys.push(
                sto(
                    guard.into_inner(),
                    ErrorSubject::StateMachine,
                    ErrorVerb::Read,
                )?
                .0,
            );
        }
        let mut batch = self.0.batch(old_keys.len() + rows.len() + 3);
        for k in &old_keys {
            batch.remove(&self.0.sm, k.as_slice());
        }
        for (index, data) in &rows {
            batch.insert(&self.0.sm, sm_data_key(self.0.pid, *index), data.as_slice());
        }
        let mut row = Vec::new();
        encode_meta_prefix(&meta_row, &mut row)?;
        batch.insert(&self.0.sm, group_key(self.0.pid, SM_META), row);
        batch.insert(
            &self.0.meta,
            group_key(self.0.pid, RM_SNAP_META),
            serde_json::to_vec(meta)
                .map_err(|_| corrupt(ErrorSubject::Snapshot(None), "snapshot meta json encode"))?,
        );
        batch.insert(&self.0.meta, group_key(self.0.pid, RM_SNAP_BLOB), blob);
        sto(
            batch.commit(),
            ErrorSubject::Snapshot(None),
            ErrorVerb::Write,
        )
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<HubTypeConfig>>, StorageError<u64>> {
        let meta_row = sto(
            self.0.meta.get(group_key(self.0.pid, RM_SNAP_META)),
            ErrorSubject::Snapshot(None),
            ErrorVerb::Read,
        )?;
        let blob_row = sto(
            self.0.meta.get(group_key(self.0.pid, RM_SNAP_BLOB)),
            ErrorSubject::Snapshot(None),
            ErrorVerb::Read,
        )?;
        let (meta_row, blob_row) = match (meta_row, blob_row) {
            (Some(m), Some(b)) => (m, b),
            _ => return Ok(None),
        };
        let meta: SnapshotMeta<u64, BasicNode> = serde_json::from_slice(&meta_row)
            .map_err(|_| corrupt(ErrorSubject::Snapshot(None), "snapshot meta json corrupted"))?;
        Ok(Some(Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(blob_row.to_vec())),
        }))
    }
}
