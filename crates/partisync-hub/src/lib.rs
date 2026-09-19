//! Hub：分片元数据(fjall)、openraft 复制、联邦路由、分层与 EC
//!
//! 行为契约见 docs/specs/ 对应工作包规格。当前实现：M3-WP01 T03
//! （entry 哈希平面——shard 路由 + 紧凑编码 + put/get/remove 墓碑）。
//! children range 平面 / 动态分裂 / 读时修复归 T04/T05。

pub mod encode;
pub mod entry_plane;
pub mod shard;

pub use encode::{
    decode_entry_row, encode_entry_row, EncodeError, EntryRow, FLAG_DELETED, KIND_DIR, KIND_FILE,
};
pub use entry_plane::{HashPlane, HubError};
pub use shard::{shard_of, SHARD_COUNT};
