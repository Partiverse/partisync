//! WP04 T02 验收：pack v2 + RS(10,4) 纠删（SPEC 验收「EC 正确性」「pack
//! 读写语义」「含索引区损坏重建」——proptest 钉住）。

use partisync_cas::ec::{EcCodec, DATA_SHARDS, PARITY_SHARDS};
use partisync_cas::pack::{build_pack, index_map, read_block, read_index};
use proptest::prelude::*;

fn gen_bytes(size: usize) -> Vec<u8> {
    let mut sm = 0x5EED_2029u64;
    (0..size)
        .map(|_| {
            sm = sm.wrapping_mul(6364136223846793005).wrapping_add(1);
            (sm >> 24) as u8
        })
        .collect()
}

proptest! {
    /// RS(10,4)：任意长度数据 → 编码 → 随机丢失 ≤4 分片 → 重建逐字节还原。
    #[test]
    fn prop_ec_roundtrip_with_losses(size in 1usize..40_000, drop_n in 0usize..=4usize) {
        let data = gen_bytes(size);
        let codec = EcCodec::new().expect("codec");
        let mut shards: Vec<Option<Vec<u8>>> =
            codec.encode(&data).expect("encode").into_iter().map(Some).collect();
        // 确定性伪随机选片（proptest 可复现）
        let mut sm = 0xC0FFEEu64 ^ size as u64;
        let mut dropped = std::collections::HashSet::new();
        while dropped.len() < drop_n {
            sm = sm.wrapping_mul(6364136223846793005).wrapping_add(1);
            dropped.insert(((sm >> 33) as usize) % (DATA_SHARDS + PARITY_SHARDS));
        }
        for i in dropped {
            shards[i] = None;
        }
        codec.reconstruct(&mut shards).expect("reconstruct");
        let restored: Vec<Vec<u8>> = shards.into_iter().flatten().collect();
        prop_assert_eq!(shards_to_data_pub(&restored, size), data);
    }

    /// pack 读写语义：任意块集（1–50 块，0–2KB）→ build → 逐块读回逐字节
    /// 相等 + 索引条目数一致。
    #[test]
    fn prop_pack_roundtrip(n in 1usize..=50usize) {
        let blocks: Vec<(String, Vec<u8>)> = (0..n)
            .map(|i| (format!("{i:064}"), gen_bytes(1 + i * 37 % 2048)))
            .collect();
        let pack = build_pack(&blocks).expect("build");
        let index = read_index(&pack).expect("index");
        prop_assert_eq!(index.entries.len(), n);
        let map = index_map(&index);
        for (hash, bytes) in &blocks {
            let got = read_block(&pack, &index, hash).expect("read");
            prop_assert_eq!(&got, bytes);
            prop_assert!(map.contains_key(hash));
        }
    }

    /// EC + 索引区损坏重建：build → 在分片流内随机破坏 ≤4 个分片（含索引
    /// 所在前段）→ EC 重建 → 索引与全部块仍可读回。
    #[test]
    fn prop_pack_corruption_rebuild(n in 2usize..=30usize, corrupt in 1usize..=4usize) {
        let blocks: Vec<(String, Vec<u8>)> = (0..n)
            .map(|i| (format!("{i:064}"), gen_bytes(16 + i * 53 % 512)))
            .collect();
        let pack = build_pack(&blocks).expect("build");
        let header_len = partisync_cas::pack::HEADER_LEN + partisync_cas::pack::SHARD_HASH_LEN;

        // 模拟损坏：直接改写分片字节（≤4 片，避开 64B header）
        let mut sm = 0xDEAD_BEEFu64 ^ n as u64 ^ corrupt as u64;
        let mut pack = pack;
        let mut touched: std::collections::HashSet<usize> = std::collections::HashSet::new();
        while touched.len() < corrupt {
            sm = sm.wrapping_mul(6364136223846793005).wrapping_add(1);
            let shard_idx = ((sm >> 33) as usize) % (DATA_SHARDS + PARITY_SHARDS);
            if !touched.insert(shard_idx) {
                continue;
            }
            let slen = shard_len_of(&pack);
            let start = header_len + shard_idx * slen;
            for b in &mut pack[start..start + slen / 2] {
                *b ^= 0xA5;
            }
        }

        // EC 重建后索引与全部块可读
        let index = read_index(&pack).expect("index after rebuild");
        for (hash, bytes) in &blocks {
            let got = read_block(&pack, &index, hash).expect("read after rebuild");
            prop_assert_eq!(&got, bytes);
        }
    }
}

// 测试辅助（proptest 宏内可见）
use partisync_cas::ec::shards_to_data as shards_to_data_pub;

fn shard_len_of(pack: &[u8]) -> usize {
    u64::from_be_bytes(pack[24..32].try_into().expect("fixed")) as usize
}
