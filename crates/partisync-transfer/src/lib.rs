//! 传输引擎：设备/云/hub 三通道、MPU、泛化 delta、限速、分块流水线
//!
//! 骨架 crate（M-1 bootstrap，调研方案附录 A）。实现按里程碑推进，
//! 行为契约见 docs/specs/ 对应工作包规格。
//!
//! M2-WP05 起包含 `chunk_plan` 块差分模块——为 iroh-blobs 验证流与 WP03
//! 对账修复提供「需推送块清单」的纯计算面。

pub mod chunk_plan;

// M3-WP04-T06: iroh-blobs 设备通道（ADR-0015）
pub mod iroh_blobs;
