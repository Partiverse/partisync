//! M8-WP04-T01 基线钉扎：T02/T03 接线前把当前「单节点直连 Hub」的
//! 边界行为固化为可执行基线（SPEC M8-WP04 §3 回归基线项）。
//!
//! 本文件只钉扎**现状**（characterization），不裁决对错：
//! - rename 覆盖已占用名：静默改写投影槽、原持有者权威行残留但隐身；
//! - 同目录命名唯一性违约（同槽异主）：投影以最后写者为准
//!   （src/lib.rs Hub 门面 doc 判例），违约不报错；
//! - `Hub::persist()` 显式刷盘 + 重开恢复；
//! - `list_children` limit=1 分页完备性（极端 limit 下界）。
//!
//! T02/T03 若改变其中任何行为，必须红灯后在本文件留修订注记再改断言
//! （铁律：不静默放宽）。

use partisync_hub::{ChildrenPage, EntryRow, Hub, KIND_DIR, KIND_FILE};

fn root_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "wp04-baseline-{tag}-{}",
        partisync_core::Ulid::now()
    ))
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

fn names_of(page: &ChildrenPage) -> Vec<&str> {
    page.items.iter().map(|i| i.name.as_str()).collect()
}

/// 基线钉扎 1：rename 到已占用名——静默覆盖投影槽。
///
/// 现状（rename_entry_impl）：`put_child` 直接改写目标槽位，不报错；
/// 原持有者权威行完好但失去投影（list 隐身，get_entry 仍可达，
/// 读时修复因槽位被异主持有而「不动」）。
#[test]
fn baseline_rename_to_occupied_name_overwrites_slot_silently() {
    let hub = Hub::open(&root_dir("rename-occupied")).expect("open");
    let dir = dir_row(1, "dir");
    hub.put_entry(&dir).expect("put dir");
    let holder = child_row(&dir, "taken.txt", 1);
    let mover = child_row(&dir, "mover.txt", 2);
    hub.put_entry(&holder).expect("put holder");
    hub.put_entry(&mover).expect("put mover");

    // mover 改名为已占用的 taken.txt：Ok，不报错（现状钉扎）
    hub.rename_entry(&mover.entry_id, Some(dir.entry_id), "taken.txt")
        .expect("rename onto occupied name must succeed (baseline)");

    // 投影槽位归 mover：list 只见 mover（holder 隐身）
    let page = hub.list_children(&dir.entry_id, None, 100).expect("list");
    assert_eq!(names_of(&page), vec!["taken.txt"]);
    assert_eq!(page.items[0].entry_id, mover.entry_id);

    // holder 权威行完好（非墓碑）；其读时修复不抢回异主槽位
    let holder_row = hub.get_entry(&holder.entry_id).expect("get").expect("row");
    assert!(!holder_row.is_deleted());
    let page = hub.list_children(&dir.entry_id, None, 100).expect("list");
    assert_eq!(names_of(&page), vec!["taken.txt"]);

    // mover 以新名可达
    let mover_row = hub.get_entry(&mover.entry_id).expect("get").expect("row");
    assert_eq!(mover_row.name, "taken.txt");
}

/// 基线钉扎 2：同目录命名唯一性违约（同槽异主）——不报错，投影以
/// 最后写者为准（Hub 门面 doc：调用方违约）。
#[test]
fn baseline_same_name_violation_projection_is_last_writer() {
    let hub = Hub::open(&root_dir("dup-name")).expect("open");
    let dir = dir_row(2, "dir");
    hub.put_entry(&dir).expect("put dir");
    let first = child_row(&dir, "dup.txt", 1);
    let second = child_row(&dir, "dup.txt", 2);
    hub.put_entry(&first).expect("put first");
    hub.put_entry(&second)
        .expect("put second (violation, no error)");

    // 权威平面：两行并存，互不覆盖
    assert!(hub.get_entry(&first.entry_id).expect("get").is_some());
    assert!(hub.get_entry(&second.entry_id).expect("get").is_some());

    // 投影平面：最后写者 second 持槽
    let page = hub.list_children(&dir.entry_id, None, 100).expect("list");
    assert_eq!(names_of(&page), vec!["dup.txt"]);
    assert_eq!(page.items[0].entry_id, second.entry_id);

    // first 的读时修复不抢回异主槽位（违约状态稳定，不抖动）
    for _ in 0..3 {
        hub.get_entry(&first.entry_id).expect("get");
        let page = hub.list_children(&dir.entry_id, None, 100).expect("list");
        assert_eq!(page.items[0].entry_id, second.entry_id);
    }
}

/// 基线钉扎 3：`Hub::persist()` 显式刷盘后重开，行与投影均恢复。
///
/// 现状缺口：重开恢复测试全部依赖引擎默认耐久性，persist() 显式
/// 调用从未与重开断言绑定（盘点表 §1-3）。
#[test]
fn baseline_persist_reopen_roundtrip() {
    let root = root_dir("persist");
    let rows: Vec<EntryRow> = {
        let hub = Hub::open(&root).expect("open");
        let dir = dir_row(3, "dir");
        hub.put_entry(&dir).expect("put dir");
        let children: Vec<EntryRow> = (0..8u16)
            .map(|s| child_row(&dir, &format!("f{s}.bin"), s))
            .collect();
        for c in &children {
            hub.put_entry(c).expect("put");
        }
        hub.remove_entry(&children[0].entry_id).expect("remove");
        hub.persist().expect("explicit persist");
        let mut all = vec![dir];
        all.extend(children);
        all
    }; // hub 在此 drop

    let hub = Hub::open(&root).expect("reopen");
    let dir = &rows[0];
    for (idx, row) in rows[1..].iter().enumerate() {
        let got = hub.get_entry(&row.entry_id).expect("get").expect("row");
        assert_eq!(got.entry_id, row.entry_id);
        assert_eq!(got.name, row.name);
        if idx == 0 {
            // 首子在刷盘前已墓碑：墓碑语义跨重开存活（原行对象仍非墓碑态）
            assert!(got.is_deleted(), "tombstone must survive persist+reopen");
        } else {
            assert!(!got.is_deleted());
        }
    }
    let page = hub.list_children(&dir.entry_id, None, 100).expect("list");
    // 首子已墓碑移除，余 7 个可见
    assert_eq!(page.items.len(), 7);
}

/// 基线钉扎 4：`list_children` limit=1 极端分页完备性。
#[test]
fn baseline_list_children_limit_one_walk_complete() {
    let hub = Hub::open(&root_dir("limit-one")).expect("open");
    let dir = dir_row(4, "dir");
    hub.put_entry(&dir).expect("put dir");
    let n = 64u32;
    let expected: Vec<String> = (0..n).map(|s| format!("c{s:03}")).collect();
    for (s, name) in expected.iter().enumerate() {
        hub.put_entry(&child_row(&dir, name, s as u16))
            .expect("put");
    }
    // limit=1 逐页走完 == 有序全量，无重复无遗漏
    let mut got = Vec::new();
    let mut cursor = None;
    loop {
        let page = hub
            .list_children(&dir.entry_id, cursor.as_ref(), 1)
            .expect("list");
        for item in &page.items {
            got.push(item.name.clone());
        }
        match page.next_cursor.as_ref() {
            Some(c) => cursor = Some(c.clone()),
            None => break,
        }
    }
    assert_eq!(got, expected);
}
