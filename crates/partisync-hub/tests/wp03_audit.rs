//! M8-WP03-T01 验收：Hub 审计日志（SPEC §2.1/§3；P18 链完整性）。

use partisync_hub::service::{HubCmd, HubService};
use partisync_hub::{EntryRow, KIND_DIR, KIND_FILE};
use std::time::Duration;

fn root(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("wp03-audit-{tag}-{}", partisync_core::Ulid::now()))
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

fn id_hex(id: &[u8; 16]) -> String {
    id.iter().map(|b| format!("{b:02x}")).collect()
}

/// 事件字段完备 + 顺序一致：put/remove/rename 依序产生审计行，
/// 五字段齐备、actor = hub-1、结果正确；幂等去重命中不产生审计行。
#[test]
fn t01_event_fields_and_order() {
    let path = root("fields");
    let svc = HubService::open(&path).expect("open");
    let dir = dir_row(1, "d");
    svc.put_entry(&dir).expect("put dir");
    let a = file_row(&dir, "a.txt", 1, 5);
    svc.put_entry(&a).expect("put a");
    svc.remove_entry(&a.entry_id).expect("remove a");
    // rename 不存在条目 = 业务拒绝（也应有审计行，result=entry_missing）
    let missing = file_row(&dir, "zz", 9, 0);
    let r = svc
        .submit_idempotent(
            [9u8; 16],
            &HubCmd::Rename(missing.entry_id, None, "x".into()),
        )
        .expect("rename submit");
    assert_eq!(r, vec![0x01]);
    // 同 req_id 再来一次（若为幂等信封）→ 去重命中，不得新增审计行
    assert!(
        svc.audit_flush(Duration::from_secs(5)),
        "flush before baseline"
    );
    let count_before = svc.audit_verify().expect("verify").0;
    let r2 = svc
        .submit_idempotent(
            [9u8; 16],
            &HubCmd::Rename(missing.entry_id, None, "y".into()),
        )
        .expect("dedup rename submit");
    assert_eq!(r2, vec![0x01]);
    assert!(
        svc.audit_flush(Duration::from_secs(5)),
        "flush after replay"
    );
    let count_after = svc.audit_verify().expect("verify").0;
    assert_eq!(count_after, count_before, "dedup hit must not audit");
    drop(svc);

    // 直读 keyspace 校验字段与顺序
    let db = fjall::Database::open(fjall::Config::new(&path)).expect("db");
    let ks = db
        .keyspace("r-audit", fjall::KeyspaceCreateOptions::default)
        .expect("ks");
    let mut rows = Vec::new();
    for guard in ks.iter() {
        let (_, v) = guard.into_inner().expect("row");
        rows.push(serde_json::from_slice::<partisync_hub::audit::AuditRow>(&v).expect("decode"));
    }
    assert_eq!(rows.len(), 4, "put,put,remove,rename-missing = 4 rows");
    let actions: Vec<&str> = rows.iter().map(|r| r.action.as_str()).collect();
    assert_eq!(actions, vec!["put", "put", "remove", "rename"]);
    assert!(rows.iter().all(|r| r.actor == "hub-1"));
    assert_eq!(rows[2].result, "ok");
    assert_eq!(rows[3].result, "entry_missing");
    // object = entry id hex（remove 行对象 = a 的 id）
    assert_eq!(rows[2].object, id_hex(&a.entry_id));
}

/// P18 链完整性：篡改 keyspace 任一行 → verify 必报。
#[test]
fn t01_chain_tamper_detected() {
    let path = root("tamper");
    {
        let svc = HubService::open(&path).expect("open");
        let dir = dir_row(2, "d");
        svc.put_entry(&dir).expect("put");
        svc.put_entry(&file_row(&dir, "f", 1, 1)).expect("put");
        assert!(svc.audit_flush(Duration::from_secs(5)));
        assert_eq!(svc.audit_verify().expect("verify").0, 2);
    }
    // 直接改库：翻转第一行一个字节
    let db = fjall::Database::open(fjall::Config::new(&path)).expect("db");
    let ks = db
        .keyspace("r-audit", fjall::KeyspaceCreateOptions::default)
        .expect("ks");
    let first_key = ks.iter().next().expect("row").into_inner().expect("kv").0;
    let mut raw: Vec<u8> = ks.get(&first_key).expect("get").expect("val").to_vec();
    raw[10] ^= 0xFF;
    ks.insert(&*first_key, raw).expect("tamper");
    drop(ks);
    drop(db);

    // 重开服务校验必须失败（重建 sink 链状态读到被篡改行，verify 复算抓出）
    let svc = HubService::open(&path).expect("reopen");
    assert!(
        svc.audit_verify().is_err(),
        "tampered chain must fail verification"
    );
}

/// 导出：JSONL 行数 == 审计行数，链头链尾独立复算一致。
#[test]
fn t01_export_head_tail() {
    let path = root("export");
    let svc = HubService::open(&path).expect("open");
    let dir = dir_row(3, "d");
    svc.put_entry(&dir).expect("put");
    for i in 0..5u16 {
        svc.put_entry(&file_row(&dir, &format!("f{i}"), i, i as u64))
            .expect("put");
    }
    assert!(svc.audit_flush(Duration::from_secs(5)));
    let out =
        std::env::temp_dir().join(format!("wp03-audit-export-{}", partisync_core::Ulid::now()));
    let info = svc.audit_export(&out).expect("export");
    assert_eq!(info.rows, 6);
    drop(svc);

    // 文件逐行 = JSONL；独立重算链
    let text = std::fs::read_to_string(&out).expect("read");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 6);
    let mut prev = partisync_hub::audit::GENESIS.to_owned();
    for (i, line) in lines.iter().enumerate() {
        let row: partisync_hub::audit::AuditRow = serde_json::from_str(line).expect("decode");
        assert_eq!(row.seq, i as u64 + 1);
        assert_eq!(row.prev_hash, prev);
        assert_eq!(row.hash, row.compute_hash());
        prev = row.hash.clone();
    }
    assert_eq!(info.tail, prev);
    assert_eq!(info.head, {
        let row: partisync_hub::audit::AuditRow = serde_json::from_str(lines[0]).expect("d");
        row.hash
    });
}
