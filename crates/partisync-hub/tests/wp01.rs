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

// ============ T05：树操作（rename O(1)/subtree）+ 读时修复 + 崩溃一致性 ============
//
// 契约：docs/specs/M3-WP01.md §4 + 验收标准「rename O(1) 语义」「投影一致性」
// 「崩溃一致性（failpoint 注入分裂协议各步）」。

use partisync_hub::split::failpoint;
use partisync_hub::READ_REPAIR_CAP;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU32, Ordering};

fn t05_root(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("wp01-t05-{tag}-{}", partisync_core::Ulid::now()))
}

fn t05_row(parent: Option<[u8; 16]>, first: u8, name: &str, kind: u8) -> EntryRow {
    let mut entry_id = [0u8; 16];
    entry_id[0] = first;
    EntryRow {
        entry_id,
        parent_id: parent,
        kind,
        name: name.into(),
        content_id: None,
        size: 7,
        mtime_ns: 0,
        flags: 0,
    }
}

#[test]
fn t05_rename_dir_o1_no_descendant_rewrites() {
    let hub = Hub::open(&t05_root("rename-o1")).expect("open");
    let top = dir_row(1, "top");
    let r = t05_row(Some(top.entry_id), 2, "r", KIND_DIR);
    let sub = t05_row(Some(r.entry_id), 3, "sub", KIND_DIR);
    let f1 = t05_row(Some(sub.entry_id), 4, "f1.txt", KIND_FILE);
    let f2 = t05_row(Some(sub.entry_id), 5, "f2.txt", KIND_FILE);
    let f3 = t05_row(Some(r.entry_id), 6, "f3.txt", KIND_FILE);
    for row in [&top, &r, &sub, &f1, &f2, &f3] {
        hub.put_entry(row).expect("put");
    }

    let mut before_entry = hub.entry.iter_entries().expect("scan");
    before_entry.sort_by_key(|e| e.entry_id);
    let mut before_tree = hub.tree.iter_child_rows().expect("scan");
    before_tree.sort_by(|a, b| (a.0, a.1.as_bytes()).cmp(&(b.0, b.1.as_bytes())));

    // 改目录名：仅本行 + 两侧投影迁移
    hub.rename_entry(&r.entry_id, Some(top.entry_id), "r-renamed")
        .expect("rename");

    let mut after_entry = hub.entry.iter_entries().expect("scan");
    after_entry.sort_by_key(|e| e.entry_id);
    let mut after_tree = hub.tree.iter_child_rows().expect("scan");
    after_tree.sort_by(|a, b| (a.0, a.1.as_bytes()).cmp(&(b.0, b.1.as_bytes())));

    // 断言行数不变 = 无子树重写（后代 entry 键与投影键逐一不变）
    assert_eq!(after_entry.len(), before_entry.len());
    assert_eq!(after_tree.len(), before_tree.len());
    for (before, after) in before_entry.iter().zip(&after_entry) {
        assert_eq!(before.entry_id, after.entry_id);
        if before.entry_id == r.entry_id {
            assert_eq!(after.name, "r-renamed");
        } else {
            assert_eq!(before, after, "non-renamed row must be untouched");
        }
    }
    for (before, after) in before_tree.iter().zip(&after_tree) {
        if before.0 == top.entry_id && before.1 == "r" {
            assert_eq!((after.0, after.1.as_str()), (top.entry_id, "r-renamed"));
            assert_eq!(after.2.entry_id, r.entry_id);
        } else {
            assert_eq!(before, after, "non-renamed projection must be untouched");
        }
    }

    // 点查 / LIST / 子树遍历反映新名
    assert_eq!(
        hub.get_entry(&r.entry_id).expect("get").expect("row").name,
        "r-renamed"
    );
    let top_list = hub.list_children(&top.entry_id, None, 10).expect("list");
    let names: Vec<_> = top_list.items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["r-renamed"]);
    let r_list = hub.list_children(&r.entry_id, None, 10).expect("list");
    let names: Vec<_> = r_list.items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["f3.txt", "sub"]);
    let sub_names: Vec<String> = hub.subtree(&r.entry_id).map(|e| e.name).collect();
    assert_eq!(
        sub_names,
        vec!["r-renamed", "f3.txt", "sub", "f1.txt", "f2.txt"]
    );
}

#[test]
fn t05_rename_move_between_dirs_root_and_errors() {
    let hub = Hub::open(&t05_root("rename-move")).expect("open");
    let t1 = dir_row(1, "t1");
    let t2 = dir_row(2, "t2");
    let f = t05_row(Some(t1.entry_id), 3, "a.txt", KIND_FILE);
    for row in [&t1, &t2, &f] {
        hub.put_entry(row).expect("put");
    }

    // 目录间移动
    hub.rename_entry(&f.entry_id, Some(t2.entry_id), "b.txt")
        .expect("move");
    let got = hub.get_entry(&f.entry_id).expect("get").expect("row");
    assert_eq!(got.parent_id, Some(t2.entry_id));
    assert_eq!(got.name, "b.txt");
    assert!(hub
        .list_children(&t1.entry_id, None, 10)
        .expect("list")
        .items
        .is_empty());
    assert_eq!(
        hub.list_children(&t2.entry_id, None, 10)
            .expect("list")
            .items[0]
            .name,
        "b.txt"
    );

    // 移到根（无投影）
    hub.rename_entry(&f.entry_id, None, "rooted.txt")
        .expect("to root");
    let got = hub.get_entry(&f.entry_id).expect("get").expect("row");
    assert_eq!(got.parent_id, None);
    assert_eq!(got.name, "rooted.txt");
    assert!(hub
        .list_children(&t2.entry_id, None, 10)
        .expect("list")
        .items
        .is_empty());
    assert!(hub
        .tree
        .iter_child_rows()
        .expect("scan")
        .iter()
        .all(|(_, _, r)| r.entry_id != f.entry_id));

    // 从根移入目录
    hub.rename_entry(&f.entry_id, Some(t1.entry_id), "back.txt")
        .expect("into dir");
    assert_eq!(
        hub.list_children(&t1.entry_id, None, 10)
            .expect("list")
            .items[0]
            .name,
        "back.txt"
    );

    // 不存在 / 已墓碑 → EntryMissing
    assert!(matches!(
        hub.rename_entry(&[0xEE; 16], Some(t1.entry_id), "x")
            .unwrap_err(),
        partisync_hub::HubError::EntryMissing
    ));
    hub.remove_entry(&f.entry_id).expect("remove");
    assert!(matches!(
        hub.rename_entry(&f.entry_id, Some(t2.entry_id), "y")
            .unwrap_err(),
        partisync_hub::HubError::EntryMissing
    ));
}

#[test]
fn t05_subtree_traversal_and_cycle_guard() {
    let hub = Hub::open(&t05_root("subtree")).expect("open");
    let a = dir_row(1, "A");
    let b = t05_row(Some(a.entry_id), 2, "b", KIND_DIR);
    let z = t05_row(Some(a.entry_id), 3, "z.txt", KIND_FILE);
    let c = t05_row(Some(b.entry_id), 4, "c", KIND_DIR);
    let y = t05_row(Some(b.entry_id), 5, "y.txt", KIND_FILE);
    let x = t05_row(Some(c.entry_id), 6, "x.txt", KIND_FILE);
    for row in [&a, &b, &z, &c, &y, &x] {
        hub.put_entry(row).expect("put");
    }
    // BFS：根 → 层内名字序
    let want = [
        a.entry_id, b.entry_id, z.entry_id, c.entry_id, y.entry_id, x.entry_id,
    ];
    let ids: Vec<[u8; 16]> = hub.subtree(&a.entry_id).map(|e| e.entry_id).collect();
    assert_eq!(ids, want);

    // 构造 parent 环（调用方违约形态）：A 的父改为其后代 c → A→b→c→A。
    // visited 防御必须保证遍历终止。
    let mut a_cycled = a.clone();
    a_cycled.parent_id = Some(c.entry_id);
    hub.put_entry(&a_cycled).expect("re-put with cycle parent");
    let count = hub.subtree(&a.entry_id).count();
    assert_eq!(
        count, 7,
        "cycle must terminate: root A, b, z, c, y, x + A-as-child"
    );
}

#[test]
fn t05_read_repair_on_get() {
    let hub = Hub::open(&t05_root("repair-get")).expect("open");
    let top = dir_row(1, "top");
    let f = t05_row(Some(top.entry_id), 2, "f.txt", KIND_FILE);
    hub.put_entry(&top).expect("put");
    hub.put_entry(&f).expect("put");

    // 投影缺失（崩溃间隙形态）→ get 修复
    hub.tree
        .remove_child(&top.entry_id, "f.txt")
        .expect("raw remove");
    assert!(hub
        .list_children(&top.entry_id, None, 10)
        .expect("list")
        .items
        .is_empty());
    hub.get_entry(&f.entry_id).expect("get").expect("row");
    let slot = hub
        .tree
        .get_child(&top.entry_id, "f.txt")
        .expect("get slot")
        .expect("repaired");
    assert_eq!(slot.entry_id, f.entry_id);
    assert_eq!(slot.kind, KIND_FILE);

    // 投影陈旧（kind 过期）→ get 改写
    hub.tree
        .put_child(
            &top.entry_id,
            "f.txt",
            &ChildRow {
                entry_id: f.entry_id,
                kind: KIND_DIR,
                deleted: false,
            },
        )
        .expect("raw stale");
    hub.get_entry(&f.entry_id).expect("get");
    let slot = hub
        .tree
        .get_child(&top.entry_id, "f.txt")
        .expect("get slot")
        .expect("present");
    assert_eq!(slot.kind, KIND_FILE);

    // 墓碑残留投影 → get 清除
    hub.remove_entry(&f.entry_id).expect("remove");
    hub.tree
        .put_child(
            &top.entry_id,
            "f.txt",
            &ChildRow {
                entry_id: f.entry_id,
                kind: KIND_FILE,
                deleted: false,
            },
        )
        .expect("raw residue");
    assert!(hub
        .get_entry(&f.entry_id)
        .expect("get")
        .expect("tombstone")
        .is_deleted());
    assert!(hub
        .tree
        .get_child(&top.entry_id, "f.txt")
        .expect("get slot")
        .is_none());
}

#[test]
fn t05_read_repair_on_list_ghosts() {
    let hub = Hub::open(&t05_root("repair-list")).expect("open");
    let top = dir_row(1, "top");
    let t2 = dir_row(2, "t2");
    let a = t05_row(Some(top.entry_id), 3, "a.txt", KIND_FILE);
    let h = t05_row(Some(top.entry_id), 4, "h.txt", KIND_FILE);
    let m = t05_row(Some(top.entry_id), 5, "m.txt", KIND_FILE);
    for row in [&top, &t2, &a, &h, &m] {
        hub.put_entry(row).expect("put");
    }
    hub.remove_entry(&h.entry_id).expect("remove h");
    hub.rename_entry(&m.entry_id, Some(t2.entry_id), "m2.txt")
        .expect("move m");

    // 注入三种幽灵：从未写入 / 墓碑残留 / 迁走残留
    let ghost_id = [0xE1u8; 16];
    for (name, id) in [
        ("g1.txt", ghost_id),
        ("h.txt", h.entry_id),
        ("m.txt", m.entry_id),
    ] {
        hub.tree
            .put_child(
                &top.entry_id,
                name,
                &ChildRow {
                    entry_id: id,
                    kind: KIND_FILE,
                    deleted: false,
                },
            )
            .expect("raw ghost");
    }

    // 返回页只含权威子项
    let page = hub.list_children(&top.entry_id, None, 100).expect("list");
    let names: Vec<_> = page.items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["a.txt"]);
    // 幽灵槽位已被清除；m 的新家槽位不受影响
    let raw = hub.tree.iter_child_rows().expect("scan");
    assert!(raw
        .iter()
        .all(|(_, n, _)| !matches!(n.as_str(), "g1.txt" | "h.txt" | "m.txt")));
    assert!(raw
        .iter()
        .any(|(d, n, r)| *d == t2.entry_id && n == "m2.txt" && r.entry_id == m.entry_id));
}

#[test]
fn t05_read_repair_cap_256() {
    let hub = Hub::open(&t05_root("repair-cap")).expect("open");
    let top = dir_row(1, "top");
    let a = t05_row(Some(top.entry_id), 2, "a.txt", KIND_FILE);
    hub.put_entry(&top).expect("put");
    hub.put_entry(&a).expect("put");
    for i in 0..300u64 {
        let mut id = [0u8; 16];
        id[8..].copy_from_slice(&(10_000 + i).to_be_bytes());
        hub.tree
            .put_child(
                &top.entry_id,
                &format!("ghost-{i:03}"),
                &ChildRow {
                    entry_id: id,
                    kind: KIND_FILE,
                    deleted: false,
                },
            )
            .expect("raw ghost");
    }

    let ghost_count = |hub: &Hub| {
        hub.tree
            .iter_child_rows()
            .expect("scan")
            .iter()
            .filter(|(d, n, _)| *d == top.entry_id && n.starts_with("ghost-"))
            .count()
    };
    // 单次调用修复补写 ≤ 256：超出的幽灵不进页但仍在存储
    let page = hub.list_children(&top.entry_id, None, 1000).expect("list");
    assert_eq!(page.items.len(), 1, "ghosts must never enter the page");
    assert_eq!(ghost_count(&hub), 300 - READ_REPAIR_CAP as usize);
    // 后续读继续修复直至清零
    let page2 = hub.list_children(&top.entry_id, None, 1000).expect("list");
    assert_eq!(page2.items.len(), 1);
    assert_eq!(ghost_count(&hub), 0);
}

static T05_CASE: AtomicU32 = AtomicU32::new(0);

proptest::proptest! {
    // 32 cases：每 case 独立 fjall Database（256 keyspace 生命周期 ≈10s，
    // IO 绑定）——32 × ≤200 随机操作序列已满足 P 风格验收；case 数随
    // keyspace 模型收敛（SPEC 风险条目，T06 复核）再上调。
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(32))]

    /// 投影一致性（SPEC 验收）：随机 put/remove/rename 后，
    /// 权威行集合 == children 投影重建的目录树（无幽灵子项、无孤儿投影）。
    #[test]
    fn t05_projection_consistency_random_ops(
        ops in proptest::collection::vec(
            (
                proptest::num::u8::ANY,     // kind: 0=put 1=remove 其余=rename
                proptest::num::u8::ANY,     // 目标选择
                proptest::num::u8::ANY,     // rename: 0=根，其余=dir[(p-1)%3]；put: dir[p%3]
                proptest::num::u8::ANY,     // 名字槽 n{n:02}
            ),
            0..200,
        ),
    ) {
        let tag = format!("prop-{}", T05_CASE.fetch_add(1, Ordering::Relaxed));
        let hub = Hub::open(&t05_root(&tag)).expect("open");
        let dirs = [dir_row(1, "d1"), dir_row(2, "d2"), dir_row(3, "d3")];
        for d in &dirs {
            hub.put_entry(d).expect("put dir");
        }
        let name_of = |n: u8| format!("n{n:02}");

        struct MRow {
            parent: Option<[u8; 16]>,
            name: String,
            deleted: bool,
        }
        let mut slots: std::collections::HashMap<(Option<[u8; 16]>, String), [u8; 16]> =
            std::collections::HashMap::new();
        let mut rows: std::collections::HashMap<[u8; 16], MRow> = std::collections::HashMap::new();
        let mut live: Vec<[u8; 16]> = Vec::new();
        let mut counter: u64 = 0;

        for (kind, t, p, n) in ops {
            match kind {
                0 => {
                    // put：仅落在空槽（碰撞形态超出 hub 投影层职责）
                    let dir = dirs[(p % 3) as usize].entry_id;
                    let name = name_of(n);
                    if slots.contains_key(&(Some(dir), name.clone())) {
                        continue;
                    }
                    counter += 1;
                    let mut id = [0u8; 16];
                    id[8..].copy_from_slice(&counter.to_be_bytes());
                    hub.put_entry(&EntryRow {
                        entry_id: id,
                        parent_id: Some(dir),
                        kind: KIND_FILE,
                        name: name.clone(),
                        content_id: None,
                        size: 1,
                        mtime_ns: 0,
                        flags: 0,
                    })
                    .expect("put");
                    slots.insert((Some(dir), name.clone()), id);
                    rows.insert(id, MRow { parent: Some(dir), name, deleted: false });
                    live.push(id);
                }
                1 if !live.is_empty() => {
                    let id = live.swap_remove((t as usize) % live.len());
                    hub.remove_entry(&id).expect("remove");
                    let r = rows.get(&id).expect("modeled");
                    if let Some(parent) = r.parent {
                        slots.remove(&(Some(parent), r.name.clone()));
                    }
                    rows.get_mut(&id).expect("modeled").deleted = true;
                }
                _ if !live.is_empty() => {
                    // rename：p==0 → 根，否则 dir[(p-1)%3]
                    let id = live[(t as usize) % live.len()];
                    let (old_parent, old_name) = {
                        let r = rows.get(&id).expect("modeled");
                        (r.parent, r.name.clone())
                    };
                    let new_parent = if p == 0 {
                        None
                    } else {
                        Some(dirs[((p - 1) % 3) as usize].entry_id)
                    };
                    let new_name = name_of(n);
                    if old_parent == new_parent && old_name == new_name {
                        continue;
                    }
                    if new_parent.is_some()
                        && slots.contains_key(&(new_parent, new_name.clone()))
                    {
                        continue;
                    }
                    hub.rename_entry(&id, new_parent, &new_name).expect("rename");
                    if let Some(op) = old_parent {
                        slots.remove(&(Some(op), old_name));
                    }
                    if let Some(np) = new_parent {
                        slots.insert((Some(np), new_name.clone()), id);
                    }
                    let r = rows.get_mut(&id).expect("modeled");
                    r.parent = new_parent;
                    r.name = new_name;
                }
                _ => {}
            }
        }

        // 1) 每目录 keyset 全遍历 == 模型槽位名集合（读时修复生效后的视图）
        for dir in &dirs {
            let mut model_names: Vec<String> = slots
                .keys()
                .filter(|(d, _)| *d == Some(dir.entry_id))
                .map(|(_, n)| n.clone())
                .collect();
            model_names.sort();
            let walked = walk_all(&hub, &dir.entry_id, 64);
            prop_assert_eq!(walked, model_names);
        }
        // 2) 原始投影行集合 == 模型槽位集合（无幽灵、无缺失、id 相等）
        type SlotEntry = ((Option<[u8; 16]>, String), [u8; 16]);
        let mut raw: Vec<SlotEntry> = hub
            .tree
            .iter_child_rows()
            .expect("scan")
            .into_iter()
            .map(|(d, n, r)| ((Some(d), n), r.entry_id))
            .collect();
        raw.sort();
        let mut model: Vec<SlotEntry> = slots.iter().map(|(k, v)| (k.clone(), *v)).collect();
        model.sort();
        prop_assert_eq!(raw, model);
        // 3) 权威行 == 模型（含墓碑）
        for (id, r) in &rows {
            let got = hub.get_entry(id).expect("get").expect("present");
            prop_assert_eq!(got.parent_id, r.parent);
            prop_assert_eq!(&got.name, &r.name);
            prop_assert_eq!(got.is_deleted(), r.deleted);
        }
    }
}

/// 崩溃场景骨架：布防 → 写入直到注入点 panic → 重开（恢复收尾）→
/// 断言服务可用、键集完整、无 splitting 残留。
fn t05_crash_scenario(tag: &str, point: &'static str, hits: usize) {
    let root = t05_root(tag);
    let dir = dir_row(1, "crash-dir");
    // 阈值 150 → 首分裂于第 ~151 笔（hits=1 点）；hit=2 的 move 点在
    // 第 ~302 笔（第二分裂）命中——500 笔预算足够，写入为 fsync 绑定故取小
    let total = 500usize;
    let names: Vec<String> = (0..total).map(|i| format!("item-{i:04}")).collect();
    let mut attempted = 0usize;
    failpoint::arm(point, hits);
    {
        let hub = Hub::open_with_threshold(&root, 150).expect("open");
        hub.put_entry(&dir).expect("put dir");
        for (i, name) in names.iter().enumerate() {
            attempted += 1;
            let fired = catch_unwind(AssertUnwindSafe(|| {
                hub.put_entry(&child_row(&dir, name, i as u16))
                    .expect("put");
            }));
            if fired.is_err() {
                break; // 注入命中：进程死亡形态（该笔效果已随批提交）
            }
        }
        failpoint::disarm_all();
        drop(hub);
    }
    assert!(attempted < total, "injection never fired: {point}");

    // 重开：恢复路径收尾 splitting，服务恢复
    let hub = Hub::open_with_threshold(&root, 150).expect("reopen after crash");
    let parts = hub.tree.partition_info();
    assert!(
        parts.iter().all(|(_, _, splitting, _)| !splitting),
        "splitting residue after recovery: {parts:?}"
    );
    // 键集完整：含注入时正在分裂的那一笔（其行与投影均先于分裂提交）
    let expected: Vec<String> = names[..attempted].to_vec();
    assert_eq!(walk_all(&hub, &dir.entry_id, 199), expected);
    for (i, name) in names.iter().enumerate().take(attempted) {
        let row = hub
            .get_entry(&child_row(&dir, name, i as u16).entry_id)
            .expect("get")
            .expect("row");
        assert!(!row.is_deleted());
        assert_eq!(row.name, *name);
    }
    // 服务可用：恢复后继续写入与读取
    for i in attempted..attempted + 50 {
        hub.put_entry(&child_row(&dir, &format!("post-{i:04}"), i as u16))
            .expect("post-crash put");
    }
    assert_eq!(walk_all(&hub, &dir.entry_id, 500).len(), attempted + 50);
}

#[test]
fn t05_crash_after_meta_commit() {
    // 注入点：分裂元数据原子批已提交、目标 keyspace 未创建（最恶劣窗口）
    t05_crash_scenario("crash-meta", failpoint::PT_META_COMMITTED, 1);
}

#[test]
fn t05_crash_mid_move() {
    // 注入点：分裂第 3 步搬移批提交后（move_keys 循环内；hits=2 → 第一次
    // 分裂计数消耗、第二次分裂命中。小阈值下每分裂单批搬移——多批部分搬移
    // 形态与单批在协议上同构：每批原子，恢复重放幂等）
    t05_crash_scenario("crash-move", failpoint::PT_MOVE_BATCH, 2);
}

#[test]
fn t05_crash_after_moves_before_active() {
    // 注入点：搬移完成、置 active 批未提交
    t05_crash_scenario("crash-done", failpoint::PT_MOVES_DONE, 1);
}

#[test]
fn t05_crash_during_recovery_replay() {
    // 恢复自身中途再崩：布防 move 点后重开 → recover_splits 重放中 panic
    // → 再重开（无布防）→ 收尾完成、键集完整。
    let root = t05_root("crash-recovery");
    let dir = dir_row(1, "crash-dir");
    let names: Vec<String> = (0..500).map(|i| format!("item-{i:04}")).collect();
    let mut attempted = 0usize;
    {
        failpoint::arm(failpoint::PT_META_COMMITTED, 1);
        let hub = Hub::open_with_threshold(&root, 150).expect("open");
        hub.put_entry(&dir).expect("put dir");
        for (i, name) in names.iter().enumerate() {
            attempted += 1;
            let fired = catch_unwind(AssertUnwindSafe(|| {
                hub.put_entry(&child_row(&dir, name, i as u16))
                    .expect("put");
            }));
            if fired.is_err() {
                break;
            }
        }
        failpoint::disarm_all();
        drop(hub);
    }
    assert!(attempted < 500, "first injection never fired");

    // 第二次打开：恢复重放被注入打断
    failpoint::arm(failpoint::PT_MOVE_BATCH, 1);
    let interrupted = catch_unwind(AssertUnwindSafe(|| {
        let _hub = Hub::open_with_threshold(&root, 150).expect("reopen");
    }));
    failpoint::disarm_all();
    assert!(
        interrupted.is_err(),
        "recovery replay must be interruptible"
    );

    // 第三次打开：恢复收尾完成
    let hub = Hub::open_with_threshold(&root, 150).expect("final reopen");
    let parts = hub.tree.partition_info();
    assert!(parts.iter().all(|(_, _, splitting, _)| !splitting));
    let expected: Vec<String> = names[..attempted].to_vec();
    assert_eq!(walk_all(&hub, &dir.entry_id, 199), expected);
}
