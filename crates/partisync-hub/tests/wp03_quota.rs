//! M8-WP03-T02 验收：空间配额（SPEC §2.2/§3）——计量、硬限拒绝、
//! 软告警跨线一次、registry 审计补接（T01 顺延项）。

use partisync_hub::registry::RouteRow;
use partisync_hub::service::HubService;
use partisync_hub::{EntryRow, KIND_DIR, KIND_FILE};
use std::time::Duration;

fn root(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("wp03-quota-{tag}-{}", partisync_core::Ulid::now()))
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

/// 计量准确性：put/remove 增量更新 used；rename 不变；覆盖写按差额。
#[test]
fn t02_meter_tracks_writes() {
    let svc = HubService::open(&root("meter")).expect("open");
    let dir = dir_row(1, "d");
    svc.put_entry(&dir).expect("put dir");
    let a = file_row(&dir, "a", 1, 10);
    let b = file_row(&dir, "b", 2, 20);
    svc.put_entry(&a).expect("put a");
    svc.put_entry(&b).expect("put b");
    assert_eq!(svc.quota_status().expect("status").used_bytes, 30);
    // 覆盖写：a 10 → 25，差额 +15
    let a2 = file_row(&dir, "a", 1, 25);
    svc.put_entry(&a2).expect("overwrite a");
    assert_eq!(svc.quota_status().expect("status").used_bytes, 45);
    // remove 回收
    svc.remove_entry(&b.entry_id).expect("remove b");
    assert_eq!(svc.quota_status().expect("status").used_bytes, 25);
    // rename 不改变计量
    svc.rename_entry(&a.entry_id, Some(dir.entry_id), "a2")
        .expect("rename");
    assert_eq!(svc.quota_status().expect("status").used_bytes, 25);
}

/// 硬限：超限 → QuotaExceeded（结构化错误），状态无变化；跨重开持久。
#[test]
fn t02_hard_limit_rejects() {
    let path = root("hard");
    let svc = HubService::open(&path).expect("open");
    let dir = dir_row(2, "d");
    svc.put_entry(&dir).expect("put dir");
    svc.set_quota(Some(100)).expect("set quota");
    svc.put_entry(&file_row(&dir, "in", 1, 60)).expect("within");
    // 超限
    let err = svc
        .put_entry(&file_row(&dir, "over", 2, 50))
        .expect_err("must reject over-quota put");
    assert!(
        err.to_string().contains("quota exceeded"),
        "unexpected error: {err}"
    );
    // 状态无变化（拒绝先于效果）
    let st = svc.quota_status().expect("status");
    assert_eq!(st.used_bytes, 60);
    assert!(
        svc.get_entry(&file_row(&dir, "over", 2, 50).entry_id)
            .expect("get")
            .is_none(),
        "rejected row must not exist"
    );
    drop(svc);

    // 配额配置跨重开持久
    let svc = HubService::open(&path).expect("reopen");
    let st = svc.quota_status().expect("status");
    assert_eq!(st.limit_bytes, Some(100));
    assert_eq!(st.used_bytes, 60);
}

/// 软告警：跨 80% 触发一次（审计行），持续超线不重复，回落后复位可再触发。
#[test]
fn t02_soft_alert_once_per_crossing() {
    let svc = HubService::open(&root("soft")).expect("open");
    let dir = dir_row(3, "d");
    svc.put_entry(&dir).expect("put dir");
    svc.set_quota(Some(100)).expect("set quota");

    svc.put_entry(&file_row(&dir, "a", 1, 50)).expect("50%");
    assert!(svc.audit_flush(Duration::from_secs(5)));
    let n0 = svc.audit_verify().expect("verify").0;

    svc.put_entry(&file_row(&dir, "b", 2, 35))
        .expect("cross to 85%");
    assert!(svc.audit_flush(Duration::from_secs(5)));
    let n1 = svc.audit_verify().expect("verify").0;
    assert_eq!(n1, n0 + 2, "crossing = put 行 + 一条软告警（仅一次）");

    // 持续超线：不重复（仅常规 put 行）
    svc.put_entry(&file_row(&dir, "c", 3, 10)).expect("95%");
    assert!(svc.audit_flush(Duration::from_secs(5)));
    assert_eq!(svc.audit_verify().expect("verify").0, n1 + 1);

    // 回落复位（删 c → 85 仍超线；再删 b → 50 < 80 复位）
    svc.remove_entry(&file_row(&dir, "c", 3, 10).entry_id)
        .expect("remove c");
    svc.remove_entry(&file_row(&dir, "b", 2, 35).entry_id)
        .expect("remove b");
    // 再跨线：新告警
    svc.put_entry(&file_row(&dir, "e", 4, 40)).expect("90%");
    assert!(svc.audit_flush(Duration::from_secs(5)));
    let n2 = svc.audit_verify().expect("verify").0;
    assert_eq!(
        n1 + 5,
        n2,
        "reset + re-cross：remove c/b + put e + 恰一条新告警"
    );
}

/// registry 管理面审计补接（T01 顺延项）：route_set 入审计链。
#[test]
fn t02_registry_route_audited() {
    let svc = HubService::open(&root("regaudit")).expect("open");
    svc.registry()
        .set_route(RouteRow {
            space_id: "s-a".into(),
            hub_id: 1,
            addr: "127.0.0.1:9100".into(),
            epoch: 1,
        })
        .expect("set route");
    assert!(svc.audit_flush(Duration::from_secs(5)));
    let (n, _) = svc.audit_verify().expect("verify");
    assert!(n >= 1, "registry route change must be audited");
}
