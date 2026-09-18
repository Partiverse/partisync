//! HLC——混合逻辑时钟（SPEC M0-WP01；P5：严格单调 + 跨设备全序）。
//!
//! **本文件当前为 S3 测试先行的桩实现**（SOP 执行方案 §3.1）。

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hlc {
    phys_ms: u64,
    logic: u32,
    device: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HlcError {
    CounterOverflow,
}

impl Hlc {
    #[must_use]
    pub const fn new(device: u64) -> Self {
        Hlc {
            phys_ms: 0,
            logic: 0,
            device,
        }
    }

    #[must_use]
    pub const fn from_wall(device: u64, wall_ms: u64) -> Self {
        Hlc {
            phys_ms: wall_ms,
            logic: 0,
            device,
        }
    }

    /// oplog 持久化重建入口（合法状态域：任意 u64/u64/u32 组合）。
    #[must_use]
    pub const fn from_raw(device: u64, phys_ms: u64, logic: u32) -> Self {
        Hlc {
            phys_ms,
            logic,
            device,
        }
    }

    pub fn tick(&mut self, _wall_ms: u64) -> Result<(), HlcError> {
        Ok(()) // S4
    }

    pub fn recv(&mut self, _wall_ms: u64, _remote: Hlc) -> Result<(), HlcError> {
        Ok(()) // S4
    }

    #[must_use]
    pub const fn phys_ms(&self) -> u64 {
        self.phys_ms
    }

    #[must_use]
    pub const fn logic(&self) -> u32 {
        self.logic
    }

    #[must_use]
    pub const fn device(&self) -> u64 {
        self.device
    }
}
