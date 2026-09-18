//! 同步引擎：watcher、scan_journal、域分离 oplog、bisync、Merkle 对账
//!
//! 骨架 crate（M-1 bootstrap，调研方案附录 A）。实现按里程碑推进，
//! 行为契约见 docs/specs/ 对应工作包规格。
