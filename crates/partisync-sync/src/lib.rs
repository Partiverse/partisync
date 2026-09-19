//! 同步核（SPEC M2-WP01）：域分离 oplog 捕获、push/pull 会话、回环防护、ACK 裁剪。
//!
//! 域分离设计（调研方案 §5.7）：设备自有数据单写者（属主状态权威，无 CRDT/共识）；
//! 共享域（v1 预留）HLC 全序 + LWW。回环防护：oplog 行携带 origin_device，
//! 应用时跳过 origin == 本机 的行。ACK 裁剪：对端确认后删除已应用行。

pub mod capture;
pub mod session;
