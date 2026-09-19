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

// ============ T04：children range 平面 + 动态分裂 ============

use partisync_hub::{ChildRow, EntryRow, Hub, KIND_DIR, KIND_FILE};

fn t04_root(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("wp01-t04-{tag}-{}", partisync_core::Ulid::now()))
}

fn dir_row(id: u8, name: &str) -> EntryRow {
    let mut entry_id = [0u8; 16];
    entry_id[0] = id;
    EntryRow {
        entry_id,
        parent_id: None,
        kind: KIND_DIR,
        name: name.into(),
        content_id: None,
        size: 0,
        mtime_ns: 0,
        flags: 0,
    }
}

fn child_row(dir: &EntryRow, name: &str, seq: u16) -> EntryRow {
    let mut entry_id = [0u8; 16];
    entry_id[0] = dir.entry_id[0];
    entry_id[14..].copy_from_slice(&seq.to_be_bytes());
    EntryRow {
        entry_id,
        parent_id: Some(dir.entry_id),
        kind: KIND_FILE,
        name: name.into(),
        content_id: None,
        size: seq as u64,
        mtime_ns: 0,
        flags: 0,
    }
}

/// 全量 keyset 遍历一个目录（用 cursor 翻页直到尽头）。
fn walk_all(hub: &Hub, dir_id: &[u8; 16], limit: u32) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = None;
    loop {
        let page = hub
            .list_children(dir_id, cursor.as_ref(), limit)
            .expect("list");
        let n = page.items.len();
        names.extend(page.items.iter().map(|i| i.name.clone()));
        match page.next_cursor {
            Some(c) => cursor = Some(c),
            None => break,
        }
        if n == 0 {
            break; // 防御：游标推进但空页（不应发生）
        }
    }
    names
}

#[test]
fn t04_projection_consistency_and_tombstone() {
    let hub = Hub::open(&t04_root("proj")).expect("open");
    let dir = dir_row(1, "root-dir");
    hub.put_entry(&dir).expect("put dir");
    let a = child_row(&dir, "alpha.txt", 1);
    let b = child_row(&dir, "beta.txt", 2);
    hub.put_entry(&a).expect("put a");
    hub.put_entry(&b).expect("put b");

    let page = hub.list_children(&dir.entry_id, None, 100).expect("list");
    let names: Vec<_> = page.items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["alpha.txt", "beta.txt"]);
    assert!(page.next_cursor.is_none());

    // 删除 = 权威墓碑 + 投影行删除（列表不再可见）
    hub.remove_entry(&a.entry_id).expect("remove");
    assert!(hub
        .get_entry(&a.entry_id)
        .expect("get")
        .expect("tombstone")
        .is_deleted());
    let page = hub.list_children(&dir.entry_id, None, 100).expect("list");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].name, "beta.txt");

    // 根条目（无 parent）不产生投影
    let root = dir_row(9, "orphan-check");
    hub.put_entry(&root).expect("put root");
    assert!(hub
        .list_children(&root.entry_id, None, 10)
        .expect("list")
        .items
        .is_empty());
}

#[test]
fn t04_list_pagination_completeness_10k() {
    let hub = Hub::open(&t04_root("pager")).expect("open");
    let dir = dir_row(2, "big-dir");
    hub.put_entry(&dir).expect("put dir");
    let n = 10_000u32;
    for seq in 0..n {
        let name = format!("child-{seq:05}");
        hub.put_entry(&child_row(&dir, &name, seq as u16))
            .expect("put");
    }
    // 分页完备：limit=137 走全遍历 == 有序全量集合，无重复无遗漏
    let walked = walk_all(&hub, &dir.entry_id, 137);
    let expected: Vec<String> = (0..n).map(|s| format!("child-{s:05}")).collect();
    assert_eq!(walked, expected);
    // 从中间页续传：cursor 之前的不再现
    let mid = hub
        .list_children(&dir.entry_id, None, 5000)
        .expect("first half");
    let cursor = mid.next_cursor.expect("cursor at 5000");
    let rest = hub
        .list_children(&dir.entry_id, Some(&cursor), 6000)
        .expect("rest");
    assert_eq!(mid.items.len(), 5000);
    assert_eq!(rest.items.len(), (n - 5000) as usize);
    assert!(rest.items[0].name > mid.items[4999].name);
}

#[test]
fn t04_split_property_small_threshold_multi_round() {
    // 注入小阈值触发多轮分裂：分裂前后全量键集相等、无丢键无重键、
    // 跨分区的单目录 children 列表仍完整有序
    let root = t04_root("split");
    let hub = Hub::open_with_threshold(&root, 300).expect("open");
    let dirs = [dir_row(1, "d1"), dir_row(2, "d2"), dir_row(3, "d3")];
    for d in &dirs {
        hub.put_entry(d).expect("put dir");
    }
    // 随机键序写入（伪 LCG 洗牌），3 目录 × 700 子项 = 2100 行 → 必然多轮分裂
    let mut rng: u64 = 0x5EED_2026;
    let mut names: Vec<(usize, String, u16)> = (0..3usize)
        .flat_map(|d| (0..700u16).map(move |s| (d, format!("item-{s:04}"), s)))
        .collect();
    for i in (1..names.len()).rev() {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let j = (rng >> 33) as usize % (i + 1);
        names.swap(i, j);
    }
    for (d, name, s) in &names {
        hub.put_entry(&child_row(&dirs[*d], name, *s)).expect("put");
    }

    // 分区数 > 1 且无 splitting 残留
    let parts = hub.tree.partition_info();
    assert!(
        parts.len() > 1,
        "expected multiple partitions after splits: {parts:?}"
    );
    assert!(parts.iter().all(|(_, _, splitting, _)| !splitting));

    // 每目录 children 全量 keyset 遍历 == 有序全量集合（跨分区边界完整）
    for (d, dir) in dirs.iter().enumerate() {
        let walked = walk_all(&hub, &dir.entry_id, 97);
        let expected: Vec<String> = (0..700u16).map(|s| format!("item-{s:04}")).collect();
        assert_eq!(walked, expected, "dir {d} children incomplete after splits");
    }

    // reopen：路由表与数据恢复一致（meta 持久化生效）
    drop(hub);
    let hub = Hub::open_with_threshold(&root, 300).expect("reopen");
    let dir = &dirs[0];
    let walked = walk_all(&hub, &dir.entry_id, 97);
    assert_eq!(walked.len(), 700);
}

#[test]
fn t04_childrow_roundtrip() {
    let row = ChildRow {
        entry_id: [7u8; 16],
        kind: KIND_DIR,
        deleted: true,
    };
    let buf = row.encode();
    assert_eq!(buf.len(), 18);
    assert_eq!(ChildRow::decode(&buf).expect("decode"), row);
    assert_eq!(
        ChildRow::decode(&buf[..17]),
        Err(partisync_hub::EncodeError::UnexpectedEof)
    );
}
