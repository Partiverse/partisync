//! M8-WP03-T03 验收：多租户最小面（SPEC §2.3 + spike F1/F2 探针；
//! docs/reviews/M8-WP03-tenant-spike.md）。

use partisync_hub::registry::RouteRow;
use partisync_hub::service::{HubService, RouteDecision};

fn root(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("wp03-tenant-{tag}-{}", partisync_core::Ulid::now()))
}

fn dev() -> partisync_hub::registry::DeviceId {
    partisync_hub::registry::DeviceId::from_hex(&"0".repeat(64)).expect("dev")
}

fn route(space: &str, hub_id: u64, epoch: u64) -> RouteRow {
    RouteRow {
        space_id: space.into(),
        hub_id,
        addr: format!("127.0.0.1:9{hub_id:03}"),
        epoch,
    }
}

/// Assign/Unassign CRUD + 一空间一租户约束 + 换绑拒绝 + 幂等重 assign。
#[test]
fn t03_assign_constraints() {
    let svc = HubService::open(&root("assign")).expect("open");
    svc.registry()
        .create_space("s1", [1u8; 16], dev())
        .expect("create space");
    // 空间不存在 → Missing
    let err = svc
        .registry()
        .assign_tenant("no-such", "acme")
        .expect_err("missing space");
    assert!(matches!(
        err,
        partisync_hub::registry::RegistryError::Missing
    ));
    // 正常归属
    svc.registry().assign_tenant("s1", "acme").expect("assign");
    assert_eq!(
        svc.registry().tenant_of("s1").expect("of").as_deref(),
        Some("acme")
    );
    // 同租户重 assign = 幂等 OK
    svc.registry()
        .assign_tenant("s1", "acme")
        .expect("idempotent");
    // 他租户换绑 → Forbidden
    let err = svc
        .registry()
        .assign_tenant("s1", "rival")
        .expect_err("reassign must be forbidden");
    assert!(matches!(
        err,
        partisync_hub::registry::RegistryError::Forbidden
    ));
    // 显式两步换绑
    svc.registry().unassign_tenant("s1").expect("unassign");
    svc.registry()
        .assign_tenant("s1", "rival")
        .expect("reassign");
    assert_eq!(
        svc.registry().tenant_of("s1").expect("of").as_deref(),
        Some("rival")
    );
    // 无归属 unassign → Missing
    let err = svc
        .registry()
        .unassign_tenant("s-none")
        .expect_err("unassign unowned");
    assert!(matches!(
        err,
        partisync_hub::registry::RegistryError::Missing
    ));
}

/// F1：他租户 → Unknown（存在性不泄露）；公共/同租户维持三分类。
#[test]
fn t03_route_visibility() {
    let svc = HubService::open(&root("route")).expect("open");
    for (space, tenant) in [
        ("s-pub", None),
        ("s-acme", Some("acme")),
        ("s-rival", Some("rival")),
    ] {
        svc.registry()
            .create_space(space, [2u8; 16], dev())
            .expect("create");
        if let Some(t) = tenant {
            svc.registry().assign_tenant(space, t).expect("assign");
        }
        svc.registry().set_route(route(space, 2, 1)).expect("route");
    }
    // acme viewer：公共/同租户可见（Redirect 到 hub 2），他租户 → Unknown
    assert_eq!(
        svc.route_for_tenant("s-acme", 1, Some("acme"))
            .expect("route"),
        RouteDecision::Redirect {
            hub_id: 2,
            addr: "127.0.0.1:9002".into()
        }
    );
    assert_eq!(
        svc.route_for_tenant("s-pub", 1, Some("acme"))
            .expect("route"),
        RouteDecision::Redirect {
            hub_id: 2,
            addr: "127.0.0.1:9002".into()
        }
    );
    assert_eq!(
        svc.route_for_tenant("s-rival", 1, Some("acme"))
            .expect("route"),
        RouteDecision::Unknown,
        "cross-tenant space must be invisible"
    );
    // None viewer：仅公共可见
    assert_eq!(
        svc.route_for_tenant("s-pub", 1, None).expect("route"),
        RouteDecision::Redirect {
            hub_id: 2,
            addr: "127.0.0.1:9002".into()
        }
    );
    assert_eq!(
        svc.route_for_tenant("s-acme", 1, None).expect("route"),
        RouteDecision::Unknown
    );
}

/// F2：routes_tenant 过滤三域；routes() 保留管理员全量。
#[test]
fn t03_routes_tenant_filter() {
    let svc = HubService::open(&root("list")).expect("open");
    for (space, tenant) in [
        ("s-pub", None),
        ("s-acme", Some("acme")),
        ("s-rival", Some("rival")),
    ] {
        svc.registry()
            .create_space(space, [3u8; 16], dev())
            .expect("create");
        if let Some(t) = tenant {
            svc.registry().assign_tenant(space, t).expect("assign");
        }
        svc.registry().set_route(route(space, 1, 1)).expect("route");
    }
    let acme = svc.registry().routes_tenant(Some("acme")).expect("list");
    let mut names: Vec<&str> = acme.iter().map(|r| r.space_id.as_str()).collect();
    names.sort();
    assert_eq!(
        names,
        vec!["s-acme", "s-pub"],
        "union of public + viewer tenant"
    );
    let none = svc.registry().routes_tenant(None).expect("list");
    assert_eq!(none.len(), 1);
    assert_eq!(none[0].space_id, "s-pub");
    // 管理员全量视图不受影响
    assert_eq!(svc.registry().routes().expect("all").len(), 3);
}

/// 审计：tenant_assign/unassign 入审计链（含拒绝结果）。
#[test]
fn t03_tenant_audited() {
    let svc = HubService::open(&root("audit")).expect("open");
    svc.registry()
        .create_space("s1", [4u8; 16], dev())
        .expect("create");
    svc.registry().assign_tenant("s1", "acme").expect("assign");
    assert!(svc.audit_flush(std::time::Duration::from_secs(5)));
    let (n0, _) = svc.audit_verify().expect("verify");
    assert!(n0 >= 1);
    // 拒绝路径也入审计
    let _ = svc.registry().assign_tenant("s1", "rival");
    assert!(svc.audit_flush(std::time::Duration::from_secs(5)));
    let (n1, _) = svc.audit_verify().expect("verify");
    assert_eq!(n1, n0 + 1, "rejected assign must be audited");
}
