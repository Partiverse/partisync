//! ProviderCaps——Provider 能力协商载体（SPEC M0-WP01 骨架；P9 的决策依据）。
//!
//! 设计原则（rclone 后端异构性的教训）：哈希/mtime/原子性差异用**显式能力位**表达，
//! 同步策略按 `change_detection_needs` 集中降级，而非散落在调用点。
//! Default 为保守全无——未声明的能力一律视为不存在（P9 前提）。

/// 后端可提供的校验/哈希能力。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HashCaps {
    pub md5: bool,
    pub sha256: bool,
    pub crc64nvme: bool,
    pub blake3: bool,
}

impl HashCaps {
    /// 双端是否有任何共同哈希（共同 = 两端都声明）。
    #[must_use]
    pub fn intersect(self, remote: Self) -> bool {
        (self.md5 && remote.md5)
            || (self.sha256 && remote.sha256)
            || (self.crc64nvme && remote.crc64nvme)
            || (self.blake3 && remote.blake3)
    }
}

/// mtime 可用精度（None = 后端不提供可靠 mtime）。
/// 派生序 = 精度序（None < Seconds < … < Nanos），`>= Seconds` 即可用启发式比较。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum MtimePrecision {
    #[default]
    None,
    Seconds,
    Millis,
    Micros,
    Nanos,
}

/// Provider 能力声明（`#[non_exhaustive]`：新增能力位不破坏下游 match）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct ProviderCaps {
    pub hash: HashCaps,
    pub mtime: MtimePrecision,
    pub atomic_rename: bool,
    pub server_side_copy: bool,
    pub presign_put: bool,
    pub event_stream: bool,
    pub multipart: bool,
}

/// 变更检测策略降级档位（劣化序：Checksum > SizeMtime > SizeOnly）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeDetection {
    /// 有共同哈希 ⇒ 直接比对内容指纹（最可靠）。
    Checksum,
    /// 无共同哈希但 mtime ≥ 秒 ⇒ 尺寸+修改时间启发式。
    SizeMtime,
    /// 只能比尺寸（最弱，误判率最高——相同尺寸不同内容无法区分）。
    SizeOnly,
}

/// 策略降级纯函数（P9 的行为出口：caps 决定路径，调用点无 if 链）。
#[must_use]
pub fn change_detection_needs(
    local: HashCaps,
    remote: HashCaps,
    mtime: MtimePrecision,
) -> ChangeDetection {
    if local.intersect(remote) {
        ChangeDetection::Checksum
    } else if mtime >= MtimePrecision::Seconds {
        ChangeDetection::SizeMtime
    } else {
        ChangeDetection::SizeOnly
    }
}
