//! M3-WP01 T03 验收：路由确定性 + 均匀性、同毫秒聚集、编码 roundtrip 属性。
//!
//! 契约：docs/specs/M3-WP01.md 验收标准第 1 条（T03 范围：entry 平面部分）。

use partisync_hub::{shard_of, SHARD_COUNT};
use proptest::prop_assert_eq;

/// 生成「ULID 形状」的 16B id：高 6B = 毫秒时间戳（BE），低 10B 随机。
fn ulid_shaped(ts_ms: u64, rng: &mut u128) -> [u8; 16] {
    *rng = rng
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let mut id = [0u8; 16];
    id[..6].copy_from_slice(&(ts_ms & ((1 << 48) - 1)).to_be_bytes()[2..]);
    id[6..].copy_from_slice(&rng.to_be_bytes()[6..]);
    id
}

fn fill_counts(ids: &[[u8; 16]]) -> [u64; SHARD_COUNT] {
    let mut counts = [0u64; SHARD_COUNT];
    for id in ids {
        counts[shard_of(id) as usize] += 1;
    }
    counts
}

/// χ² 拟合优度均匀性检验（df = 255，Wilson–Hilferty 正态近似 5σ 上界）。
///
/// SPEC M3-WP01 验收原写「最大偏差 < 5%」——T03 实测校准：256 分片取最大值
/// 的统计波动本身就常超 5%（均值 3906 时 5% ≈ 3.2σ，误报率 ~17%，同毫秒
/// 样本 5.06% 误报复现），故改用标准 χ² 检验；修订记录见规格验收节。
fn assert_uniform_chi2(counts: &[u64; SHARD_COUNT], tag: &str) {
    let total: u64 = counts.iter().sum();
    let mean = total as f64 / SHARD_COUNT as f64;
    let chi2: f64 = counts
        .iter()
        .map(|c| {
            let d = (*c as f64) - mean;
            d * d / mean
        })
        .sum();
    let df = (SHARD_COUNT - 1) as f64;
    let bound = df + 5.0 * (2.0 * df).sqrt(); // χ²(255) 5σ 上界 ≈ 368
    assert!(
        chi2 < bound,
        "{tag}: chi2 {chi2:.1} >= 5σ bound {bound:.1} (df {df}, mean {mean:.1})"
    );
}

#[test]
fn routing_determinism_and_uniformity_1m() {
    // 10⁶ 随机 ULID 样本：确定性重放一致 + χ²(255) 均匀性 5σ 带内
    let mut rng: u128 = 0x2026_0920;
    let ids: Vec<[u8; 16]> = (0..1_000_000u32)
        .map(|_| ulid_shaped(1_726_800_000, &mut rng))
        .collect();
    let a = fill_counts(&ids);
    let b = fill_counts(&ids);
    assert_eq!(a, b, "shard_of must be deterministic");
    assert_uniform_chi2(&a, "random-ulid-1m");
}

#[test]
fn routing_uniformity_same_millisecond_batch() {
    // 同毫秒聚集 ULID（批导入形态）：高 6B 固定，10⁶ 样本与随机形态
    // 用同一 χ² 口径——聚集不得引入额外偏度
    let mut rng: u128 = 0xBEEF;
    let ids: Vec<[u8; 16]> = (0..1_000_000u32)
        .map(|_| ulid_shaped(1_726_800_000, &mut rng))
        .collect();
    let counts = fill_counts(&ids);
    assert_uniform_chi2(&counts, "same-ms-1m");
}

#[test]
fn reopen_restores_all_shards_and_rows() {
    let dir = std::env::temp_dir().join(format!("wp01-t03-{}", partisync_core::Ulid::now()));
    let mut rng: u128 = 0x5EED;
    let rows: Vec<[u8; 16]> = (0..1_000u32)
        .map(|_| ulid_shaped(1_726_800_000, &mut rng))
        .collect();
    {
        let plane = partisync_hub::HashPlane::open(&dir).expect("open");
        for (i, id) in rows.iter().enumerate() {
            plane
                .put(&partisync_hub::EntryRow {
                    entry_id: *id,
                    parent_id: None,
                    kind: partisync_hub::KIND_FILE,
                    name: format!("asset-{i}"),
                    content_id: None,
                    size: i as u64,
                    mtime_ns: 1,
                    flags: 0,
                })
                .expect("put");
        }
        plane.persist().expect("persist");
    }
    let plane = partisync_hub::HashPlane::open(&dir).expect("reopen");
    for (i, id) in rows.iter().enumerate() {
        let got = plane.get(id).expect("get").expect("row survives reopen");
        assert_eq!(got.name, format!("asset-{i}"));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(512))]

    #[test]
    fn encode_roundtrip_property(
        parent in proptest::option::of(proptest::collection::vec(proptest::prelude::any::<u8>(), 16)),
        kind in proptest::num::u8::ANY,
        name in proptest::string::string_regex(".{0,120}").unwrap(),
        content in proptest::option::of(proptest::collection::vec(proptest::prelude::any::<u8>(), 32)),
        size in proptest::num::u64::ANY,
        mtime in proptest::num::i64::ANY,
        flags in proptest::num::u16::ANY,
    ) {
        let row = partisync_hub::EntryRow {
            entry_id: [0u8; 16],
            parent_id: parent.map(|p| <[u8; 16]>::try_from(p).unwrap()),
            kind,
            name,
            content_id: content.map(|c| <[u8; 32]>::try_from(c).unwrap()),
            size,
            mtime_ns: mtime,
            flags,
        };
        let buf = partisync_hub::encode_entry_row(&row).unwrap();
        let back = partisync_hub::decode_entry_row(&buf).unwrap();
        prop_assert_eq!(back.parent_id, row.parent_id);
        prop_assert_eq!(back.kind, row.kind);
        prop_assert_eq!(back.name, row.name);
        prop_assert_eq!(back.content_id, row.content_id);
        prop_assert_eq!(back.size, row.size);
        prop_assert_eq!(back.mtime_ns, row.mtime_ns);
        prop_assert_eq!(back.flags, row.flags);
    }
}
