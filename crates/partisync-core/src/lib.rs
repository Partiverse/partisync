//! 共享值类型与基础契约（crate 地图最底层，不依赖任何其他 partisync crate）。
//!
//! 当前内容：[`ulid`]——PartiGraph 实体主键（SPEC M-1-WP07）。

pub mod ulid;

pub use ulid::{Ulid, UlidError};
