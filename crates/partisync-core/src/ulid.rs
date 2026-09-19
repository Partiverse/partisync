//! ULID——Universally Unique Lexicographically Sortable Identifier。
//!
//! 契约见 `docs/specs/M-1-WP07.md`：16 字节（48 bit 毫秒时间戳 + 80 bit 随机）、
//! 26 字符 Crockford Base32、字节序 = 字典序 = 时间序（properties.md L1–L3）。
//!
//! 严格解析决策：大小写不敏感；`I/L/O/U` 不做别名映射（内部主键场景，
//! 互操作导入需求出现时另行 ADR）；长度必须恰为 26；首字符译值 > 7 即溢出。

use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use getrandom::getrandom;

/// Crockford Base32 字母表（排除 I/L/O/U）。
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// 48 bit 毫秒时间戳掩码。
const TS_MASK: u64 = (1 << 48) - 1;

/// 80 bit 随机段掩码。
const RND_MASK: u128 = (1 << 80) - 1;

/// 译码表：ASCII 字节 → 5 bit 译值；无效为 `INVALID`。
const DECODE: [u8; 128] = build_decode();
const INVALID: u8 = 0xFF;

const fn build_decode() -> [u8; 128] {
    let mut table = [INVALID; 128];
    let mut i = 0;
    while i < 32 {
        table[ALPHABET[i] as usize] = i as u8;
        i += 1;
    }
    table
}

/// 128 bit ULID：高 48 bit 毫秒时间戳 + 低 80 bit 随机段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ulid([u8; 16]);

/// 解析与构造错误（SPEC M-1-WP07）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UlidError {
    /// 长度不是 26 字符。
    InvalidLength {
        /// 实际字符数。
        len: usize,
    },
    /// 字母表外字符（含被排除的 I/L/O/U 与非 ASCII）。
    InvalidChar(char),
    /// 整数值 ≥ 2^128（首字符译值 > 7）。
    Overflow,
}

impl fmt::Display for UlidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UlidError::InvalidLength { len } => {
                write!(f, "invalid ULID length: {len} (expected 26)")
            }
            UlidError::InvalidChar(c) => write!(f, "invalid ULID character: {c:?}"),
            UlidError::Overflow => {
                write!(f, "ULID value overflows 128 bits (first char decodes > 7)")
            }
        }
    }
}

impl std::error::Error for UlidError {}

impl Ulid {
    /// 由毫秒时间戳与随机段构造；超宽输入按 48/80 bit 掩码截断。
    #[must_use]
    pub const fn from_parts(timestamp_ms: u64, random: u128) -> Ulid {
        let value = ((timestamp_ms & TS_MASK) as u128) << 80 | (random & RND_MASK);
        Ulid(value.to_be_bytes())
    }

    /// 当前时刻 + OS 熵生成。
    ///
    /// 不保证同毫秒单调（排序需求由 oplog/HLC 承担，SPEC 非目标）。
    /// OS 熵不可用属不可恢复环境错误，直接 panic；
    /// 系统时钟早于 Unix 纪元时时间戳饱和为 0（AI 审查 F2：显式文档化）。
    #[must_use]
    pub fn now() -> Ulid {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let mut buf = [0u8; 10]; // 80 bit
        getrandom(&mut buf).expect("OS entropy unavailable (getrandom failed)");
        let mut rnd = 0u128;
        for &b in &buf {
            rnd = (rnd << 8) | u128::from(b);
        }
        Ulid::from_parts(ms, rnd)
    }

    /// 毫秒时间戳（低 48 bit）。
    #[must_use]
    pub const fn timestamp_ms(&self) -> u64 {
        (u128::from_be_bytes(self.0) >> 80) as u64
    }

    /// 随机段（低 80 bit）。
    #[must_use]
    pub const fn random_part(&self) -> u128 {
        u128::from_be_bytes(self.0) & RND_MASK
    }

    /// 原生 16 字节（BE）——平面 ID 分片路由与二进制键编码用（M3-WP01）。
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Display for Ulid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut v = u128::from_be_bytes(self.0);
        let mut buf = [b'0'; 26]; // 26×5 = 130 bit ≥ 128 bit，首字符覆盖 3 bit + 2 零位
        for i in (0..26).rev() {
            buf[i] = ALPHABET[(v & 31) as usize];
            v >>= 5;
        }
        debug_assert_eq!(v, 0, "编码循环后残留高位应为 0");
        f.write_str(std::str::from_utf8(&buf).expect("Crockford 字母表为 ASCII"))
    }
}

impl FromStr for Ulid {
    type Err = UlidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let len = s.chars().count();
        if len != 26 {
            return Err(UlidError::InvalidLength { len });
        }
        let mut value = 0u128;
        for (i, ch) in s.chars().enumerate() {
            if !ch.is_ascii() {
                return Err(UlidError::InvalidChar(ch));
            }
            let d = DECODE[ch.to_ascii_uppercase() as usize];
            if d == INVALID {
                return Err(UlidError::InvalidChar(ch));
            }
            // 仅首字符受 3 bit 上限约束（26×5=130 bit，高 2 位恒 0）；
            // 后续位置的大译值合法（如 "008…" = 2^118），不得误判溢出。
            if i == 0 && d > 7 {
                return Err(UlidError::Overflow);
            }
            value = (value << 5) | u128::from(d);
        }
        Ok(Ulid(value.to_be_bytes()))
    }
}
