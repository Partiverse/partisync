//! M8-WP04-T03c 验收：分裂×复制崩溃矩阵接线（RFC M3-WP02 §6，P8 口径）。
//!
//! 单节点组拓扑（v0.1）下可执行相位：
//! - **M1/M2（raft 服务面）**：wp03 `t07_m3_kill_during_split_meta_commit_recover`
//!   已以 `PT_META_COMMITTED` failpoint 覆盖（本文件不重复，核销表引用）；
//! - **M4**：新组 bootstrap 期间 kill → 控制面重试 bootstrap（幂等）
//!   ——连续 kill/reopen 循环，键集与身份必须完整；
//! - **M5**：控制面写初始数据中 kill → 断点续写由**同键 upsert + 系统
//!   层幂等去重（T03a `r-dedup`）**收敛——重放同 req_id 不得双应用；
//! - **M6**：合并视图（注册表持久化 + T02 m-meta）——kill 后路由视图
//!   稳定、同空间单行无重键、身份不漂移；
//! - **M3（follower 复制期 kill leader）**：需多节点组拓扑，v0.1 不可
//!   执行——维持规格态，触发条件 = T03 多节点演化（group_id→store
//!   接线），核销表如实登记。

use partisync_hub::registry::RouteRow;
use partisync_hub::service::{HubCmd, HubService, RouteDecision};
use partisync_hub::{EntryRow, KIND_DIR, KIND_FILE};

fn cm_root(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("wp04-cm-{tag}-{}", partisync_core::Ulid::now()))
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

fn req(tag: u8, seq: u16) -> [u8; 16] {
    let mut id = [0u8; 16];
    id[0] = tag;
    id[2..4].copy_from_slice(&seq.to_be_bytes());
    id
}

/// M4：bootstrap 期间 kill → 控制面重试（连续 kill/reopen 循环）。
/// 不变量：新组空日志可重 bootstrap（幂等）；键集不回退；身份不漂移。
#[test]
fn m4_bootstrap_retry_cycles_converge() {
    let root = cm_root("m4");
    let svc = HubService::open(&root).expect("open");
    let dir = dir_row(1, "d");
    svc.put_entry(&dir).expect("put dir");
    for i in 0..10u16 {
        svc.put_entry(&file_row(&dir, &format!("f{i}"), i, i as u64))
            .expect("put");
    }
    svc.crash();
    drop(svc);

    // 两轮 kill/reopen：bootstrap 重试均为幂等成功
    for cycle in 0..2u8 {
        let svc = HubService::open(&root).unwrap_or_else(|e| {
            panic!("reopen cycle {cycle} must succeed (bootstrap idempotent): {e}")
        });
        assert_eq!(svc.identity().node_id, 1, "identity must not drift");
        let page = svc.list_children(&dir.entry_id, None, 100).expect("list");
        assert_eq!(page.items.len(), 10, "keyset must survive cycle {cycle}");
        svc.crash();
        drop(svc);
    }
    // 终态：服务可用，可继续 raft 写
    let svc = HubService::open(&root).expect("final open");
    svc.put_entry(&file_row(&dir, "post", 99, 1))
        .expect("post-recovery put");
}

/// M5：控制面写初始数据中 kill → 断点续写收敛。
/// 不变量：同 req_id 重放抑制双应用（T03a r-dedup）；同键 upsert 收敛；
/// 键集完整。
#[test]
fn m5_initial_data_resume_converges() {
    let root = cm_root("m5");
    let svc = HubService::open(&root).expect("open");
    let dir = dir_row(2, "d");
    svc.put_entry(&dir).expect("put dir");
    let n = 20u16;
    for i in 0..n {
        let row = file_row(&dir, &format!("f{i:02}"), i, 1);
        let r = svc
            .submit_idempotent(req(0xE1, i), &HubCmd::Put(row))
            .expect("submit");
        assert_eq!(r, vec![0x00]);
    }
    svc.crash();
    drop(svc);

    // 重开：断点续写 = 全量重放同 req_id（同键 upsert 口径）
    let svc = HubService::open(&root).expect("reopen");
    for i in 0..n {
        let row = file_row(&dir, &format!("f{i:02}"), i, 1);
        let r = svc
            .submit_idempotent(req(0xE1, i), &HubCmd::Put(row))
            .expect("resume submit");
        assert_eq!(r, vec![0x00], "resume replay of req {i}");
    }
    // 系统层去重：同 req_id 换载荷（size=999）必须被抑制
    let mutated = file_row(&dir, "f00", 0, 999);
    let r = svc
        .submit_idempotent(req(0xE1, 0), &HubCmd::Put(mutated))
        .expect("mutated replay");
    assert_eq!(r, vec![0x00]);
    let got = svc
        .get_entry(&file_row(&dir, "f00", 0, 1).entry_id)
        .expect("get")
        .expect("row");
    assert_eq!(got.size, 1, "no double-apply across kill/reopen");
    // 键集完整
    let page = svc.list_children(&dir.entry_id, None, 100).expect("list");
    assert_eq!(page.items.len(), n as usize);
}

/// M6：分裂期合并视图中 kill → 注册表持久化 + m-meta 身份不漂移。
/// 不变量：路由视图稳定（Local/Redirect 不漂移）；同空间单行无重键。
#[test]
fn m6_merged_view_registry_persistence() {
    let root = cm_root("m6");
    let svc = HubService::open(&root).expect("open");
    svc.registry()
        .set_route(RouteRow {
            space_id: "s-merged".into(),
            hub_id: 1,
            addr: "127.0.0.1:9100".into(),
            epoch: 1,
        })
        .expect("set route");
    assert_eq!(
        svc.route_for("s-merged", 1).expect("route"),
        RouteDecision::Local
    );
    svc.crash();
    drop(svc);

    // 重开：合并视图稳定——路由行持久、身份不漂移、无重键
    let svc = HubService::open(&root).expect("reopen");
    assert_eq!(svc.identity().node_id, 1, "m-meta identity must persist");
    assert_eq!(
        svc.route_for("s-merged", 1).expect("route"),
        RouteDecision::Local,
        "routing view must survive kill"
    );
    assert_eq!(
        svc.route_for("s-merged", 2).expect("route"),
        RouteDecision::Redirect {
            hub_id: 1,
            addr: "127.0.0.1:9100".into()
        },
        "redirect branch must survive kill"
    );
    let routes = svc.registry().routes().expect("routes");
    let s_merged: Vec<_> = routes.iter().filter(|r| r.space_id == "s-merged").collect();
    assert_eq!(s_merged.len(), 1, "no duplicate keys in merged view");
}
