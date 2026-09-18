//! PartiGraph 资产图谱——Entry/ContentIdentity（SPEC M0-WP02）。
//!
//! 当前内容：SQLite schema v1 + [`store::Store`] 仓储 + [`indexer`]（最小索引器）。
//! Tag DAG/Sidecar 归 M4，oplog 归 M2。

pub mod indexer;
pub mod journal;
pub mod store;
pub mod watch;

pub use journal::{apply_pending, record, Applied, EventKind, JournalEvent};
pub use store::{DupGroup, EntryKind, EntryRow, Stats, Store};
