//! 分片路由（SPEC M3-WP01 §2）：entry 平面按平面 ID 哈希分片。
//!
//! 路由哈希一律 `blake3::hash`（一次性哈希）；禁止 `blake3::derive_key`
//! ——后者属 KDF 冻结面（债务 D1，SEC-AUDIT-2026-M2-001 划界）。
//! ULID 时间有序，不允许按 id 直接 mod 分片（写热点）——必须过哈希。

use blake3::Hasher;

/// entry 平面固定分片数（SPEC M3-WP01 裁定 1：永不分裂）。
pub const SHARD_COUNT: usize = 256;

/// 平面 ID → 哈希分片号（确定性、均匀性验收见 tests/wp01.rs）。
#[must_use]
pub fn shard_of(entry_id: &[u8; 16]) -> u8 {
    let mut hasher = Hasher::new();
    hasher.update(entry_id);
    let digest = hasher.finalize();
    let bytes = digest.as_bytes();
    // blake3 输出前 16 bit 取整模 256（SPEC 公式：u16 BE % 256）
    (u16::from_be_bytes([bytes[0], bytes[1]]) % SHARD_COUNT as u16) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shard_of_is_deterministic() {
        let id = [0x01u8; 16];
        assert_eq!(shard_of(&id), shard_of(&id));
        let id2 = [0xFFu8; 16];
        assert_eq!(shard_of(&id2), shard_of(&id2));
    }

    #[test]
    fn shard_of_stays_in_range() {
        for i in 0..1000u64 {
            let mut id = [0u8; 16];
            id[..8].copy_from_slice(&i.to_be_bytes());
            assert!((shard_of(&id) as usize) < SHARD_COUNT);
        }
    }
}
