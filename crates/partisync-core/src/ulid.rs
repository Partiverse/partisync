//! ULID——Universally Unique Lexicographically Sortable Identifier。
//!
//! 契约见 `docs/specs/M-1-WP07.md`：16 字节（48 bit 毫秒时间戳 + 80 bit 随机）、
//! 26 字符 Crockford Base32、字节序 = 字典序 = 时间序。
//!
//! **本文件当前为 S3 测试先行的桩实现**（SOP 执行方案 §3.1）：测试已按规格批准，
//! 实现由后续任务完成（红→绿演示，见 SPEC 验收标准）。

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ulid([u8; 16]);

#[expect(dead_code, reason = "S3 桩阶段：InvalidChar/Overflow 尚无构造点，S4 实现后移除")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UlidError {
    InvalidLength { len: usize },
    InvalidChar(char),
    Overflow,
}

impl Ulid {
    #[must_use]
    pub const fn from_parts(_timestamp_ms: u64, _random: u128) -> Ulid {
        Ulid([0; 16]) // S4：按位组装（掩码 48/80 bit）
    }

    #[must_use]
    pub fn now() -> Ulid {
        Ulid::from_parts(0, 0) // S4：系统毫秒 + getrandom
    }

    #[must_use]
    pub const fn timestamp_ms(&self) -> u64 {
        0 // S4
    }

    #[must_use]
    pub const fn random_part(&self) -> u128 {
        0 // S4
    }
}

impl fmt::Display for Ulid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("00000000000000000000000000") // S4：Crockford 编码
    }
}

impl FromStr for Ulid {
    type Err = UlidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Err(UlidError::InvalidLength {
            len: s.chars().count(),
        }) // S4：严格解析
    }
}
