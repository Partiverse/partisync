//! 共享值类型与基础契约（crate 地图最底层，不依赖任何其他 partisync crate）。
//!
//! 当前内容：[`ulid`]（PartiGraph 实体主键）、[`hlc`]（oplog 时钟，SPEC M0-WP01）。

pub mod hlc;
pub mod ulid;

pub use hlc::{Hlc, HlcError};
pub use ulid::{Ulid, UlidError};
