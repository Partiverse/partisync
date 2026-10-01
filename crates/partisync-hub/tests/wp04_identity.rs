//! M8-WP04-T02 验收：节点身份入构造 + `m-meta` 持久化（SPEC §2.1）。
//!
//! 契约：身份与组拓扑首开写入 `m-meta`、重开核对；同 root 异身份/
//! 异拓扑显式拒绝；`None` = 单节点降级（与既有 `Hub::open` 语义
//! 等价，行为由 wp04_baseline 基线兜底）。

use partisync_hub::service::{HubService, HubServiceConfig, NodeIdentity};

fn t02_root(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("wp04-t02-{tag}-{}", partisync_core::Ulid::now()))
}

fn cfg(root: &std::path::Path, identity: Option<NodeIdentity>) -> HubServiceConfig {
    HubServiceConfig {
        root: root.to_path_buf(),
        split_threshold: 4_000_000,
        election_timeout_ms: (150, 300),
        heartbeat_interval_ms: 50,
        identity,
    }
}

fn ident(node_id: u64, group_id: u64) -> NodeIdentity {
    let addr = "127.0.0.1:0".to_owned();
    NodeIdentity {
        node_id,
        addr: addr.clone(),
        group_id,
        members: [(node_id, addr)].into_iter().collect(),
    }
}

#[test]
fn t02_default_identity_reopen_ok() {
    let root = t02_root("default-reopen");
    {
        let svc = HubService::open(&root).expect("open default");
        let id = svc.identity();
        // 单节点降级身份与既有硬编码逐字一致
        assert_eq!(id.node_id, 1);
        assert_eq!(id.group_id, 1);
        assert_eq!(id.members.len(), 1);
        assert!(id.members.contains_key(&1));
    } // drop → runtime 关停
    let svc = HubService::open(&root).expect("reopen default must match stored identity");
    assert_eq!(svc.identity().node_id, 1);
}

#[test]
fn t02_identity_mismatch_rejected() {
    let root = t02_root("mismatch");
    {
        let _svc = HubService::open(&root).expect("open default");
    }
    let err = match HubService::open_with_config(cfg(&root, Some(ident(2, 1)))) {
        Ok(_) => panic!("same root + different node_id must be rejected"),
        Err(e) => e,
    };
    assert!(
        err.to_string().contains("identity mismatch"),
        "unexpected error: {err}"
    );
}

#[test]
fn t02_explicit_identity_persisted_and_restored() {
    let root = t02_root("explicit");
    {
        let svc = HubService::open_with_config(cfg(&root, Some(ident(5, 7))))
            .expect("open explicit identity");
        assert_eq!(svc.identity().node_id, 5);
        assert_eq!(svc.identity().group_id, 7);
    }
    // 同身份重开：核对通过
    let svc = HubService::open_with_config(cfg(&root, Some(ident(5, 7))))
        .expect("reopen with same identity");
    assert_eq!(svc.identity().group_id, 7);
    drop(svc);
    // 同 root 换 node_id：拒绝
    let err = match HubService::open_with_config(cfg(&root, Some(ident(6, 7)))) {
        Ok(_) => panic!("node_id change on same root must be rejected"),
        Err(e) => e,
    };
    assert!(err.to_string().contains("identity mismatch"));
}

#[test]
fn t02_identity_members_must_contain_self() {
    let root = t02_root("no-self");
    let bad = NodeIdentity {
        node_id: 5,
        addr: "127.0.0.1:0".to_owned(),
        group_id: 7,
        members: std::iter::once((1u64, "127.0.0.1:0".to_owned())).collect(),
    };
    let err = match HubService::open_with_config(cfg(&root, Some(bad))) {
        Ok(_) => panic!("members without self node_id must be rejected"),
        Err(e) => e,
    };
    assert!(
        err.to_string().contains("must contain self"),
        "unexpected error: {err}"
    );
}
