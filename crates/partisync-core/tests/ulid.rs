//! `core::ulid` 规格测试（SPEC M-1-WP07，S3 测试先行）。
//!
//! 单元部分 = 已知向量与解析规则；属性部分 = properties.md 的 L1–L3。
//! 本文件在实现前合入（红），实现后全绿（SOP 执行方案 §3.1-S3/S4）。

use std::str::FromStr;

use partisync_core::{Ulid, UlidError};
use proptest::prelude::*;

const ZERO: &str = "00000000000000000000000000";
const MAX: &str = "7ZZZZZZZZZZZZZZZZZZZZZZZZZ";
const CROCKFORD: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

fn is_crockford(s: &str) -> bool {
    s.len() == 26
        && s.bytes()
            .all(|b| CROCKFORD.contains(&b) && !matches!(b, b'I' | b'L' | b'O' | b'U'))
}

// ---------- 已知向量（SPEC 契约） ----------

#[test]
fn zero_vector() {
    let u = Ulid::from_parts(0, 0);
    assert_eq!(u.to_string(), ZERO);
    assert_eq!(Ulid::from_str(ZERO).unwrap(), u);
    assert_eq!(u.timestamp_ms(), 0);
    assert_eq!(u.random_part(), 0);
}

#[test]
fn max_vector() {
    let u = Ulid::from_parts(0xFFFF_FFFF_FFFF, u128::MAX);
    assert_eq!(u.to_string(), MAX);
    assert_eq!(Ulid::from_str(MAX).unwrap(), u);
    assert_eq!(u.timestamp_ms(), 0xFFFF_FFFF_FFFF);
    assert_eq!(u.random_part(), (1u128 << 80) - 1);
}

#[test]
fn overflow_vector() {
    // 首字符 '8' 译值 8 > 7 → 整数值 ≥ 2^128
    assert_eq!(
        Ulid::from_str("8ZZZZZZZZZZZZZZZZZZZZZZZZZ"),
        Err(UlidError::Overflow)
    );
}

#[test]
fn length_must_be_exactly_26() {
    assert_eq!(
        Ulid::from_str(&ZERO[..25]),
        Err(UlidError::InvalidLength { len: 25 })
    );
    assert_eq!(
        Ulid::from_str(&format!("{ZERO}0")),
        Err(UlidError::InvalidLength { len: 27 })
    );
    assert_eq!(
        Ulid::from_str(""),
        Err(UlidError::InvalidLength { len: 0 })
    );
}

#[test]
fn excluded_and_non_ascii_chars_rejected() {
    // Crockford 排除 I/L/O/U（不做别名映射，SPEC 严格解析决策）
    for c in ['I', 'L', 'O', 'U', 'i', 'l', 'o', 'u'] {
        let s: String = std::iter::once(c).chain(std::iter::repeat('0')).take(26).collect();
        assert_eq!(Ulid::from_str(&s), Err(UlidError::InvalidChar(c)), "char {c}");
    }
    // 非一切字母表字符与非 ASCII
    assert_eq!(
        Ulid::from_str("!0000000000000000000000000"),
        Err(UlidError::InvalidChar('!'))
    );
    assert_eq!(
        Ulid::from_str("中00000000000000000000000000"),
        Err(UlidError::InvalidChar('中'))
    );
}

#[test]
fn parse_is_case_insensitive() {
    let upper = Ulid::from_str(MAX).unwrap();
    let lower = Ulid::from_str(&MAX.to_lowercase()).unwrap();
    let mixed = Ulid::from_str("7zZzZzZzZzZzZzZzZzZzZzZzZz").unwrap();
    assert_eq!(upper, lower);
    assert_eq!(upper, mixed);
}

#[test]
fn field_extraction_roundtrip() {
    let ts = 0x1234_5678_9ABC_u64;
    let rnd = 0x0123_4567_89AB_CDEF_0123_u128;
    let u = Ulid::from_parts(ts, rnd);
    assert_eq!(u.timestamp_ms(), ts);
    assert_eq!(u.random_part(), rnd);
    assert_eq!(Ulid::from_str(&u.to_string()).unwrap(), u);
}

#[test]
fn masking_on_from_parts() {
    // 超宽输入被掩码截断：ts 取低 48 bit，random 取低 80 bit
    let u = Ulid::from_parts(1 << 48, u128::MAX);
    assert_eq!(u.timestamp_ms(), 0);
    assert_eq!(u.random_part(), (1u128 << 80) - 1);
}

// ---------- 排序语义（L2） ----------

#[test]
fn order_is_timestamp_then_random() {
    let a = Ulid::from_parts(1, u128::MAX);
    let b = Ulid::from_parts(2, 0);
    let c = Ulid::from_parts(3, u128::MAX);
    assert!(a < b && b < c, "时间戳严格更小 ⇒ Ulid 更小，随机段不影响");
    assert!(Ulid::from_parts(5, 0) < Ulid::from_parts(5, 1), "同毫秒按随机段排序");
}

// ---------- now() 冒烟 ----------

#[test]
fn now_smoke() {
    let t0 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let u = Ulid::now();
    let t1 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    assert!(
        (t0.saturating_sub(1)..=t1 + 1).contains(&u.timestamp_ms()),
        "now() 时间戳应落在生成时刻附近: {} ∉ [{t0}, {t1}]",
        u.timestamp_ms()
    );
    assert_ne!(u.random_part(), 0, "OS 熵下随机段全零概率 2^-80，出现即失败");
    assert!(is_crockford(&u.to_string()));
}

// ---------- 属性测试（properties.md L1–L3） ----------

proptest! {
    // L1: encode(parse(x)) == x，且字段提取往返一致
    #[test]
    fn prop_roundtrip(ts in 0u64..(1u64 << 48), rnd in any::<u128>()) {
        let u = Ulid::from_parts(ts, rnd);
        let parsed = Ulid::from_str(&u.to_string()).unwrap();
        prop_assert_eq!(parsed, u);
        prop_assert_eq!(parsed.timestamp_ms(), ts);
        prop_assert_eq!(parsed.random_part(), rnd & ((1u128 << 80) - 1));
    }

    // L2: ts1 < ts2 ⇒ ulid(ts1, ·) < ulid(ts2, ·)（随机段任意）
    #[test]
    fn prop_ordering_by_timestamp(
        ts1 in 0u64..(1u64 << 48),
        diff in 1u64..1000u64,
        r1 in any::<u128>(),
        r2 in any::<u128>(),
    ) {
        let ts2 = (ts1.saturating_add(diff)).min((1u64 << 48) - 1);
        prop_assume!(ts2 > ts1);
        prop_assert!(Ulid::from_parts(ts1, r1) < Ulid::from_parts(ts2, r2));
    }

    // L3: Display 恒 26 字符且严格落在 Crockford 字母表内
    #[test]
    fn prop_display_format(ts in 0u64..(1u64 << 48), rnd in any::<u128>()) {
        let s = Ulid::from_parts(ts, rnd).to_string();
        prop_assert!(is_crockford(&s), "非 Crockford 输出: {s}");
    }
}
