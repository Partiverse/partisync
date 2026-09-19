//! HLC——混合逻辑时钟（SPEC M0-WP01；P5：严格单调 + 跨设备全序）。
//!
//! 语义（Lamport/Mattern 式定义，Spacedrive HLC 先例）：
//! - `tick(w)`：墙钟前进 ⇒ `phys=w, logic=0`；否则 `logic+=1`（回拨靠逻辑位保序）；
//! - `recv(w, r)`：`c = max(w, phys, r.phys)`；logic 按 c 的来源分支合并；
//! - 每次操作后 self 严格大于操作前，且 recv 后严格大于 remote（oplog 去重需要）；
//! - `device` 参与排序键 ⇒ 跨设备全序（对端须持不同 device id，由 oplog 注册表保证）。

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hlc {
    phys_ms: u64,
    logic: u32,
    device: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HlcError {
    /// 同一 phys 毫秒内逻辑位打满 2³²（调用方按 Fatal 分类处置）。
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

    /// 本地事件。操作原子：溢出时返回 Err 且状态不变。
    pub fn tick(&mut self, wall_ms: u64) -> Result<(), HlcError> {
        if wall_ms > self.phys_ms {
            self.phys_ms = wall_ms;
            self.logic = 0;
            return Ok(());
        }
        self.logic = self.logic.checked_add(1).ok_or(HlcError::CounterOverflow)?;
        Ok(())
    }

    /// 远端事件合并。操作原子：溢出时返回 Err 且状态不变。
    pub fn recv(&mut self, wall_ms: u64, remote: Hlc) -> Result<(), HlcError> {
        let c_phys = wall_ms.max(self.phys_ms).max(remote.phys_ms);
        // SPEC v1.1 修正：c 的来源有四种——self、remote、双方相等、**仅 wall 领先**
        // （v1.0 契约漏数了 wall 这个操作数，由 P5 属性测试发现）
        let next_logic = if c_phys > self.phys_ms && c_phys > remote.phys_ms {
            0 // 仅墙钟领先 ⇒ 新纪元，logic 归零
        } else if c_phys == self.phys_ms && c_phys == remote.phys_ms {
            self.logic.max(remote.logic)
        } else if c_phys == self.phys_ms {
            self.logic
        } else {
            remote.logic
        };
        // 先验证再提交，保证原子性（溢出时 self 不变）
        let committed = next_logic.checked_add(1).ok_or(HlcError::CounterOverflow)?;
        self.phys_ms = c_phys;
        self.logic = committed;
        Ok(())
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

    /// oplog 主键序列化（phys:logic:device，各段定宽 hex——字符串序 == 时间序）。
    #[must_use]
    pub fn to_key(&self) -> String {
        format!(
            "{:016x}-{:08x}-{:016x}",
            self.phys_ms, self.logic, self.device
        )
    }

    /// [`Hlc::to_key`] 的逆运算。
    ///
    /// # Errors
    /// 格式不符 → `HlcError` 由调用侧以字符串错误承载（v1 oplog 主键只来自 to_key）。
    pub fn from_key(key: &str) -> Option<Hlc> {
        let mut it = key.split('-');
        let phys = u64::from_str_radix(it.next()?, 16).ok()?;
        let logic = u32::from_str_radix(it.next()?, 16).ok()?;
        let device = u64::from_str_radix(it.next()?, 16).ok()?;
        Some(Hlc {
            phys_ms: phys,
            logic,
            device,
        })
    }
}
