//! 装配层事件面（M9-WP01-T01；SPEC M9-WP01 §2.1）。
//!
//! 挂载写「apply 成功」时刻的事件（R5 落锤：WAL execute 后即压实——
//! WAL 不留痕，事件源只能是 apply 成功点而非日志回读）。路径口径 =
//! backing 相对路径、'/' 分隔、无前导 '/'（`WriteBackOp` 同款）；折叠
//! 成 `EventRecord`（graph 口径前导 '/'）归 gateway 装配层，fuser 依赖
//! 不出本 crate 边界（ADR-0026）。

/// 写事件（挂载写成功产物；emit 点唯一 = execute/create 成功分支）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FuseWriteEvent {
    /// 新文件落 backing（create）或既有文件被整文件替换（flush /
    /// setattr truncate）。
    Upsert { path: String },
    /// 文件/空目录删除（unlink/rmdir）。
    Remove { path: String },
    /// 同挂载点改名（rename；折叠为 Removed(from) + Created(to) 归
    /// 装配层——Created 须带源条目 size，T05 判例）。
    Rename { from: String, to: String },
}

/// 事件汇（gateway 装配层持有 Rx 端；fuser handler 同步线程 `send`
/// 无阻塞——unbounded channel，收端存活期发送必成功）。
pub type EventSink = tokio::sync::mpsc::UnboundedSender<FuseWriteEvent>;
