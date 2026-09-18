//! 传输引擎：设备/云/hub 三通道、MPU、泛化 delta、限速、分块流水线
//!
//! 骨架 crate（M-1 bootstrap，调研方案附录 A）。实现按里程碑推进，
//! 行为契约见 docs/specs/ 对应工作包规格。
