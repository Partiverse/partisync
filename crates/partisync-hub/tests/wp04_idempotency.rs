//! M8-WP04-T03a 验收：请求 id 幂等去重（SPEC §2.4，RFC M3-WP02 §3.1）。
//!
//! 契约：同 `req_id` 重放返回首次响应、不产生双应用；去重表随 apply
//! 落盘（`r-dedup`），崩溃/重开后语义保持；不同 `req_id` 互不影响。

use partisync_hub::service::{HubCmd, HubService};
use partisync_hub::{EntryRow, KIND_DIR, KIND_FILE};

fn t03a_root(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("wp04-t03a-{tag}-{}", partisync_core::Ulid::now()))
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

fn file_row(dir: &EntryRow, name: &str, seq: u16, size: u64) -> EntryRow {
    let mut entry_id = [0u8; 16];
    entry_id[0] = dir.entry_id[0];
    entry_id[14..].copy_from_slice(&seq.to_be_bytes());
    EntryRow {
        entry_id,
        parent_id: Some(dir.entry_id),
        kind: KIND_FILE,
        name: name.into(),
        content_id: None,
        size,
        mtime_ns: 0,
        flags: 0,
    }
}

fn req(tag: u8) -> [u8; 16] {
    let mut id = [0u8; 16];
    id[0] = tag;
    id
}

/// 同 req_id 重放：第二次提交（载荷不同）不得产生双应用——权威行保持
/// 首次内容；两次响应均为首次结果（0x00）。
#[test]
fn t03a_dedup_skips_second_application() {
    let svc = HubService::open(&t03a_root("dedup")).expect("open");
    let dir = dir_row(1, "dir");
    svc.put_entry(&dir).expect("put dir");
    let row = file_row(&dir, "f.bin", 1, 100);

    let r1 = svc
        .submit_idempotent(req(0xA1), &HubCmd::Put(row.clone()))
        .expect("first submit");
    assert_eq!(r1, vec![0x00]);

    // 同 req_id、不同载荷（size=999）：必须被去重，不得改写权威行
    let mutated = file_row(&dir, "f.bin", 1, 999);
    let r2 = svc
        .submit_idempotent(req(0xA1), &HubCmd::Put(mutated))
        .expect("replay submit");
    assert_eq!(r2, vec![0x00], "replay returns first result");

    let got = svc.get_entry(&row.entry_id).expect("get").expect("row");
    assert_eq!(got.size, 100, "second application must be suppressed");
}

/// 业务拒绝（EntryMissing）也计首次结果：重放返回 0x01，且换载荷
/// 重放不得「借尸」执行新效果。
#[test]
fn t03a_first_result_sticks_on_reject() {
    let svc = HubService::open(&t03a_root("reject")).expect("open");
    let dir = dir_row(2, "dir");
    svc.put_entry(&dir).expect("put dir");
    let missing = {
        let mut id = [0u8; 16];
        id[0] = 9;
        id
    };

    // Rename 到不存在的条目 = EntryMissing（业务拒绝，响应 0x01）
    let r1 = svc
        .submit_idempotent(req(0xB1), &HubCmd::Rename(missing, None, "x".into()))
        .expect("first submit");
    assert_eq!(r1, vec![0x01], "business reject flag");

    // 同 req_id 换成有效 Put：命中去重 → 不执行
    let row = file_row(&dir, "ghost.bin", 2, 7);
    let r2 = svc
        .submit_idempotent(req(0xB1), &HubCmd::Put(row.clone()))
        .expect("replay submit");
    assert_eq!(r2, vec![0x01], "replay returns first result");
    assert!(svc.get_entry(&row.entry_id).expect("get").is_none());
}

/// 去重表随 apply 落盘：重开后同 req_id 重放仍返回首次结果、不改状态。
#[test]
fn t03a_dedup_persists_across_reopen() {
    let root = t03a_root("persist");
    let row = {
        let svc = HubService::open(&root).expect("open");
        let dir = dir_row(3, "dir");
        svc.put_entry(&dir).expect("put dir");
        let row = file_row(&dir, "f.bin", 1, 42);
        svc.submit_idempotent(req(0xC1), &HubCmd::Put(row.clone()))
            .expect("submit");
        row
    }; // drop

    let svc = HubService::open(&root).expect("reopen");
    let dir = dir_row(3, "dir");
    let mutated = file_row(&dir, "f.bin", 1, 1234);
    let r = svc
        .submit_idempotent(req(0xC1), &HubCmd::Put(mutated))
        .expect("replay after reopen");
    assert_eq!(r, vec![0x00]);

    let got = svc.get_entry(&row.entry_id).expect("get").expect("row");
    assert_eq!(got.size, 42, "dedup table must survive reopen");
}

/// 不同 req_id 同内容：各自正常应用（去重不误伤普通提交）。
#[test]
fn t03a_different_ids_apply_independently() {
    let svc = HubService::open(&t03a_root("indep")).expect("open");
    let dir = dir_row(4, "dir");
    svc.put_entry(&dir).expect("put dir");

    for seq in 0..3u16 {
        let row = file_row(&dir, &format!("f{seq}.bin"), seq, seq as u64);
        let r = svc
            .submit_idempotent(req(0xD0 + seq as u8), &HubCmd::Put(row))
            .expect("submit");
        assert_eq!(r, vec![0x00]);
    }
    let page = svc.list_children(&dir.entry_id, None, 100).expect("list");
    assert_eq!(page.items.len(), 3);
}
