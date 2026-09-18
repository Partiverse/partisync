//! 共享值类型与基础契约（crate 地图最底层，不依赖任何其他 partisync crate）。
//!
//! 当前内容：[`ulid`]（实体主键）、[`hlc`]（oplog 时钟）、[`error`]（错误分类学）、
//! [`caps`]（Provider 能力协商载体）——SPEC M0-WP01。

pub mod caps;
pub mod error;
pub mod hlc;
pub mod ulid;

pub use caps::{change_detection_needs, ChangeDetection, HashCaps, MtimePrecision, ProviderCaps};
pub use error::{classify_io, PartisyError, Severity};
pub use hlc::{Hlc, HlcError};
pub use ulid::{Ulid, UlidError};
