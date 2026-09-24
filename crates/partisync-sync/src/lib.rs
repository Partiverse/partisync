//! 同步核（SPEC M2-WP01/M2-WP02/M2-WP03）：域分离 oplog 捕获、push/pull/bisync、
//! 回环防护、ACK 裁剪、冲突血缘（P11）、Tag 共享域 LWW、Merkle 对账（P7）。
//!
//! 域分离设计（调研方案 §5.7）：设备自有数据单写者（属主状态权威，无 CRDT/共识）；
//! 共享域 HLC 全序 + LWW（墓碑防复活）。回环防护：oplog 行携带 origin_device，
//! 应用时跳过 origin == 本机 的行；中继保持原 HLC 键（一个写入全网同一个键）。
//! ACK 裁剪：对端确认后删除已应用行。Merkle 对账：水位快路径 + 分级桶下钻修复。

pub mod capture;
pub mod chaos;
pub mod crypto;
pub mod event;
pub mod failpoint;
pub mod pairing;
pub mod reconcile;
pub mod scan;
pub mod session;
