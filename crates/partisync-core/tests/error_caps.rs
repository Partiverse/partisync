//! `core::error` 与 `core::caps` 规格测试（SPEC M0-WP01）。

use std::io::ErrorKind as K;

use partisync_core::caps::{
    change_detection_needs, ChangeDetection, HashCaps, MtimePrecision, ProviderCaps,
};
use partisync_core::error::{classify_io, PartisyError, Severity};

// ---------- classify_io（表驱动，SPEC 映射表逐项） ----------

#[test]
fn classify_io_retryable_set() {
    for kind in [
        K::TimedOut,
        K::ConnectionRefused,
        K::ConnectionReset,
        K::ConnectionAborted,
        K::NetworkUnreachable,
        K::Interrupted,
        K::WouldBlock,
        K::NotFound,
    ] {
        assert_eq!(classify_io(kind), Severity::Retryable, "{kind:?}");
    }
}

#[test]
fn classify_io_fatal_set() {
    for kind in [
        K::PermissionDenied,
        K::InvalidData,
        K::InvalidInput,
        K::Unsupported,
        K::StorageFull,
        K::WriteZero,
        K::OutOfMemory,
    ] {
        assert_eq!(classify_io(kind), Severity::Fatal, "{kind:?}");
    }
}

#[test]
fn classify_io_never_returns_interrupted() {
    // Interrupted 是应用层显式语义，classify_io 恒不产出（含 io kind 同名项）
    for kind in [
        K::TimedOut,
        K::ConnectionRefused,
        K::ConnectionReset,
        K::ConnectionAborted,
        K::NetworkUnreachable,
        K::Interrupted,
        K::WouldBlock,
        K::NotFound,
        K::PermissionDenied,
        K::InvalidData,
        K::InvalidInput,
        K::Unsupported,
        K::StorageFull,
        K::WriteZero,
        K::OutOfMemory,
    ] {
        assert_ne!(classify_io(kind), Severity::Interrupted);
    }
}

#[test]
fn partisy_error_display_and_source() {
    let e = PartisyError::new(Severity::Retryable);
    assert_eq!(e.to_string(), "retryable");
    let e = PartisyError::with_source(
        Severity::Fatal,
        Box::new(std::io::Error::new(K::PermissionDenied, "denied")),
    );
    assert_eq!(e.to_string(), "fatal: denied");
    assert!(std::error::Error::source(&e).is_some());
}

// ---------- ProviderCaps / change_detection_needs（P9 出口） ----------

fn caps(hash_bits: (bool, bool, bool, bool), mtime: MtimePrecision) -> HashCaps {
    let _ = mtime;
    HashCaps {
        md5: hash_bits.0,
        sha256: hash_bits.1,
        crc64nvme: hash_bits.2,
        blake3: hash_bits.3,
    }
}

#[test]
fn default_caps_are_conservative() {
    let c = ProviderCaps::default();
    assert_eq!(c.mtime, MtimePrecision::None);
    assert!(!c.atomic_rename);
    assert!(!c.server_side_copy);
    assert!(!c.presign_put);
    assert!(!c.event_stream);
    assert!(!c.multipart);
    assert_eq!(c.hash, HashCaps::default());
    // 保守 Default ⇒ 最弱策略
    assert_eq!(
        change_detection_needs(c.hash, c.hash, c.mtime),
        ChangeDetection::SizeOnly
    );
}

#[test]
fn change_detection_degradation_order() {
    let md5 = caps((true, false, false, false), MtimePrecision::Seconds);
    // 共同哈希 ⇒ Checksum（即使 mtime 完美）
    assert_eq!(
        change_detection_needs(md5, md5, MtimePrecision::Nanos),
        ChangeDetection::Checksum
    );
    // 无共同哈希 + mtime 秒级 ⇒ SizeMtime
    let sha_only = caps((false, true, false, false), MtimePrecision::Seconds);
    assert_eq!(
        change_detection_needs(sha_only, md5, MtimePrecision::Seconds),
        ChangeDetection::SizeMtime
    );
    // 无哈希 + mtime None ⇒ SizeOnly
    assert_eq!(
        change_detection_needs(
            HashCaps::default(),
            HashCaps::default(),
            MtimePrecision::None
        ),
        ChangeDetection::SizeOnly
    );
    // 亚秒精度仍可 SizeMtime（≥Seconds 判定按可用性）
    assert_eq!(
        change_detection_needs(
            HashCaps::default(),
            HashCaps::default(),
            MtimePrecision::Nanos
        ),
        ChangeDetection::SizeMtime
    );
}

#[test]
fn hash_intersect_symmetry_and_specificity() {
    let a = caps((true, false, false, false), MtimePrecision::None);
    let b = caps((false, true, false, false), MtimePrecision::None);
    assert!(!a.intersect(b) && !b.intersect(a), "不同哈希无交集");
    let blake3_only = caps((false, false, false, true), MtimePrecision::None);
    assert!(blake3_only.intersect(blake3_only));
}

#[test]
fn hash_intersect_exhaustive_two_bit_truth_table() {
    // S5（D5 变异抽检）：intersect 的布尔算子变异缺测试区分。
    // 对 {md5, sha256} 两 bit 穷举 4×4 组合，期望值 = 逐哈希 AND 的逻辑或
    // ——任意 `||`↔`&&` 变异都改变至少一个组合的结果。
    for l_md5 in [false, true] {
        for l_sha in [false, true] {
            for r_md5 in [false, true] {
                for r_sha in [false, true] {
                    let l = caps((l_md5, l_sha, false, false), MtimePrecision::None);
                    let r = caps((r_md5, r_sha, false, false), MtimePrecision::None);
                    let expected = (l_md5 && r_md5) || (l_sha && r_sha);
                    assert_eq!(
                        l.intersect(r),
                        expected,
                        "l=({l_md5},{l_sha}) r=({r_md5},{r_sha})"
                    );
                }
            }
        }
    }
}

#[test]
fn hash_intersect_crc_and_blake3_terms() {
    // 覆盖第 3/4 项：单独命中与混合命中
    let crc = caps((false, false, true, false), MtimePrecision::None);
    let blake = caps((false, false, false, true), MtimePrecision::None);
    let crc_blake = caps((false, false, true, true), MtimePrecision::None);
    assert!(!crc.intersect(blake));
    assert!(crc.intersect(crc_blake));
    assert!(blake.intersect(crc_blake));
}
