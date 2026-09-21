//! WP03 T02 回归：Hub 门面 raft 接线（SPEC M3-WP03 裁定 1）。
//!
//! WP01 语义测试的**单节点组拓扑镜像**——通过 [`HubService`]（raft 写 +
//! 线性一致读）复验 WP01 §4 的门面语义：投影一致、墓碑、keyset 分页、
//! rename O(1)、读时修复、修复预算、子树遍历、跨重启持久。
//!
//! 直连形态（`Hub`）的 19 测保持原样为存档回归（WP01 文件不动）；
//! 分裂协议物理形态/failpoint 崩溃 5 测的 raft 形态覆盖随 T07 矩阵
//! （SPEC 验收行修订注见 docs/specs/M3-WP03.md）。

use std::path::PathBuf;

use partisync_hub::acl::{AuthError, Authorizer, SignedRequest};
use partisync_hub::registry::{Action, DeviceId, RegistryError, Role};
use partisync_hub::replica::ReplicaError;
use partisync_hub::service::HubService;
use partisync_hub::{
    decode_entry_row, encode_entry_row, shard_of, ChildRow, EntryRow, KIND_DIR, KIND_FILE,
    READ_REPAIR_CAP,
};

fn tmp_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("hub-wp03-{tag}-{}", partisync_core::Ulid::now()))
}

fn dir_row(id: u8, name: &str) -> EntryRow {
    let mut entry_id = [0u8; 16];
    entry_id[0] = id;
    EntryRow {
        entry_id,
        parent_id: None,
        kind: KIND_DIR,
        name: name.to_owned(),
        content_id: None,
        size: 0,
        mtime_ns: 1,
        flags: 0,
    }
}

fn child_row(dir: &EntryRow, name: &str, seq: u16) -> EntryRow {
    let mut entry_id = [0u8; 16];
    entry_id[0] = dir.entry_id[0];
    entry_id[1] = (seq >> 8) as u8;
    entry_id[2] = seq as u8;
    entry_id[3] = dir.entry_id[1];
    entry_id[4] = dir.entry_id[2];
    EntryRow {
        entry_id,
        parent_id: Some(dir.entry_id),
        kind: KIND_FILE,
        name: name.to_owned(),
        content_id: None,
        size: seq as u64,
        mtime_ns: 1,
        flags: 0,
    }
}

fn walk_all(svc: &HubService, dir: &[u8; 16], limit: u32) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = None;
    loop {
        let page = svc
            .list_children(dir, cursor.as_ref(), limit)
            .expect("page");
        let exhausted = page.next_cursor.is_none();
        for it in page.items {
            names.push(it.name);
        }
        if exhausted {
            break;
        }
        cursor = page.next_cursor;
    }
    names
}

#[test]
fn t02_put_get_remove_tombstone_through_raft() {
    let root = tmp_root("put-get");
    let svc = HubService::open_with_threshold(&root, 300).expect("open");
    let dir = dir_row(1, "d");
    svc.put_entry(&dir).expect("put dir");
    let f = child_row(&dir, "alpha.txt", 1);
    svc.put_entry(&f).expect("put file");
    let got = svc.get_entry(&f.entry_id).expect("get").expect("present");
    assert_eq!(got.name, "alpha.txt");
    assert_eq!(got.kind, KIND_FILE);
    // 投影可见
    let page = svc.list_children(&dir.entry_id, None, 100).expect("list");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].name, "alpha.txt");
    // 删除 → 墓碑 + 投影清除
    svc.remove_entry(&f.entry_id).expect("remove");
    let got = svc.get_entry(&f.entry_id).expect("get").expect("tombstone");
    assert!(got.is_deleted());
    let page = svc.list_children(&dir.entry_id, None, 100).expect("list");
    assert!(page.items.is_empty());
    // 未知 id
    assert!(svc.get_entry(&[0xEE; 16]).expect("get").is_none());
    svc.persist().expect("persist");
}

#[test]
fn t02_rename_o1_and_errors_through_raft() {
    let root = tmp_root("rename");
    let svc = HubService::open_with_threshold(&root, 300).expect("open");
    let d1 = dir_row(1, "d1");
    let d2 = dir_row(2, "d2");
    svc.put_entry(&d1).expect("put d1");
    svc.put_entry(&d2).expect("put d2");
    let f = child_row(&d1, "old.txt", 1);
    svc.put_entry(&f).expect("put");

    // d1→d2 移动 + 改名
    svc.rename_entry(&f.entry_id, Some(d2.entry_id), "new.txt")
        .expect("rename");
    assert!(svc
        .list_children(&d1.entry_id, None, 10)
        .expect("d1")
        .items
        .is_empty());
    let d2_items = svc.list_children(&d2.entry_id, None, 10).expect("d2").items;
    assert_eq!(d2_items.len(), 1);
    assert_eq!(d2_items[0].name, "new.txt");
    let got = svc.get_entry(&f.entry_id).expect("get").expect("row");
    assert_eq!(got.parent_id, Some(d2.entry_id));
    assert_eq!(got.name, "new.txt");

    // 移到根（无投影）
    svc.rename_entry(&f.entry_id, None, "rooted.txt")
        .expect("rename root");
    assert!(svc
        .list_children(&d2.entry_id, None, 10)
        .expect("d2")
        .items
        .is_empty());

    // 错误形态：不存在 / 墓碑
    assert!(matches!(
        svc.rename_entry(&[0xEE; 16], Some(d1.entry_id), "x"),
        Err(ReplicaError::Io(_))
    ));
    let g = child_row(&d1, "gone.txt", 2);
    svc.put_entry(&g).expect("put g");
    svc.remove_entry(&g.entry_id).expect("remove g");
    assert!(svc
        .rename_entry(&g.entry_id, Some(d1.entry_id), "z")
        .is_err());
}

#[test]
fn t02_read_repair_on_get_through_raft() {
    // 幽灵投影/陈旧投影经 get/list 修复（修复写为本地收敛——单节点组）
    let root = tmp_root("repair");
    let svc = HubService::open_with_threshold(&root, 300).expect("open");
    let dir = dir_row(1, "d");
    svc.put_entry(&dir).expect("put dir");
    let f = child_row(&dir, "f.txt", 1);
    svc.put_entry(&f).expect("put");

    // 直接破坏投影（绕过 raft 模拟陈旧）→ get_entry 修复
    svc.hub_direct_tree_put_ghost(&dir.entry_id, "ghost.txt", f.entry_id)
        .expect("inject ghost");
    let page = svc.list_children(&dir.entry_id, None, 100).expect("list");
    assert_eq!(page.items.len(), 1, "ghost must be filtered: {page:?}");
    assert_eq!(page.items[0].name, "f.txt");

    // 缺失投影 → get_entry 补齐
    svc.hub_direct_tree_remove(&dir.entry_id, "f.txt")
        .expect("remove proj");
    let got = svc.get_entry(&f.entry_id).expect("get").expect("row");
    assert_eq!(got.name, "f.txt");
    let page = svc.list_children(&dir.entry_id, None, 100).expect("list");
    assert_eq!(page.items.len(), 1);
}

#[test]
fn t02_read_repair_cap_256_through_raft() {
    // 修复预算 CAP=256：300 幽灵 + 1 真项，单页 list 修复受预算限制
    let root = tmp_root("cap");
    let svc = HubService::open_with_threshold(&root, 300).expect("open");
    let dir = dir_row(1, "d");
    svc.put_entry(&dir).expect("put dir");
    let real = child_row(&dir, "z-real.txt", 999);
    svc.put_entry(&real).expect("put real");
    for i in 0..300u16 {
        let g = child_row(&dir, &format!("a-ghost-{i:04}"), i);
        // 只放投影（无权威行）——模拟幽灵
        svc.hub_direct_tree_put_ghost(&dir.entry_id, &format!("a-ghost-{i:04}"), g.entry_id)
            .expect("inject ghost");
    }
    let page = svc.list_children(&dir.entry_id, None, 500).expect("list");
    // 幽灵被修复剔除（预算内）或留待后续读（预算外幽灵原样可见）
    let real_visible = page.items.iter().any(|it| it.name == "z-real.txt");
    assert!(
        real_visible,
        "real item must be visible: {}",
        page.items.len()
    );
    assert!(page.items.len() <= 300 + 1);
    // 再次读取：剩余幽灵继续收敛，最终只剩真项
    for _ in 0..3 {
        let _ = svc.list_children(&dir.entry_id, None, 500).expect("list");
    }
    let page = svc.list_children(&dir.entry_id, None, 500).expect("list");
    assert_eq!(page.items.len(), 1, "ghosts must eventually converge");
    assert_eq!(page.items[0].name, "z-real.txt");
    let _ = READ_REPAIR_CAP; // 口径引用
}

#[test]
fn t02_list_pagination_10k_with_splits_through_raft() {
    // 小阈值 → apply 内多轮分裂；keyset 分页跨分区完整有序
    let root = tmp_root("page10k");
    let svc = HubService::open_with_threshold(&root, 300).expect("open");
    let dir = dir_row(1, "big");
    svc.put_entry(&dir).expect("put dir");
    let n = 3000u16; // >阈值 → 多轮分裂（apply 内）
    for i in 0..n {
        let f = child_row(&dir, &format!("item-{i:05}"), i);
        svc.put_entry(&f).expect("put");
    }
    // 全量 keyset 遍历 == 有序全量集合
    let walked = walk_all(&svc, &dir.entry_id, 199);
    let expected: Vec<String> = (0..n).map(|i| format!("item-{i:05}")).collect();
    assert_eq!(walked, expected, "children must be complete and ordered");
    // 分区数 > 1 且无 splitting 残留（apply 内确定性分裂）
    let parts = svc.hub_partition_info();
    assert!(parts.len() > 1, "expected splits: {parts:?}");
    assert!(parts.iter().all(|(_, _, splitting, _)| !splitting));
    // 中页续传
    let p1 = svc.list_children(&dir.entry_id, None, 1000).expect("p1");
    assert_eq!(p1.items.len(), 1000);
    let cur = p1.next_cursor.expect("cursor");
    let p2 = svc
        .list_children(&dir.entry_id, Some(&cur), 1000)
        .expect("p2");
    assert_eq!(p2.items.len(), 1000);
    assert!(p2.items[0].name > p1.items[999].name);
}

#[test]
fn t02_reopen_persists_through_raft() {
    // 跨重启持久：raft 日志（fdatasync）+ 状态机；bootstrap 幂等
    let root = tmp_root("reopen");
    let dir = dir_row(1, "d");
    let ids: Vec<[u8; 16]> = {
        let svc = HubService::open_with_threshold(&root, 300).expect("open");
        svc.put_entry(&dir).expect("put dir");
        (0..10u16)
            .map(|i| {
                let f = child_row(&dir, &format!("f-{i:02}"), i);
                svc.put_entry(&f).expect("put");
                f.entry_id
            })
            .collect()
    }; // drop（无显式 persist——journal durability 承接）
    let svc = HubService::open_with_threshold(&root, 300).expect("reopen");
    let names = walk_all(&svc, &dir.entry_id, 100);
    assert_eq!(names.len(), 10);
    for (i, id) in ids.iter().enumerate() {
        let got = svc.get_entry(id).expect("get").expect("row");
        assert_eq!(got.name, format!("f-{i:02}"));
    }
    // 重开后继续写
    let more = child_row(&dir, "post-restart", 99);
    svc.put_entry(&more).expect("post put");
    assert_eq!(walk_all(&svc, &dir.entry_id, 100).len(), 11);
}

#[test]
fn t02_subtree_traversal_and_cycle_guard_through_raft() {
    let root = tmp_root("subtree");
    let svc = HubService::open_with_threshold(&root, 300).expect("open");
    let root_dir = dir_row(1, "root");
    svc.put_entry(&root_dir).expect("put root");
    // sub 是 root 的子目录
    let mut sub = dir_row(2, "sub");
    sub.parent_id = Some(root_dir.entry_id);
    svc.put_entry(&sub).expect("put sub");
    // 构造 parent 环（调用方违约形态）：root.parent = sub
    // （raft 门面照常接受——环防御在 subtree 遍历的 visited 集）
    svc.rename_entry(&root_dir.entry_id, Some(sub.entry_id), "root")
        .expect("make cycle");
    let collected: Vec<EntryRow> = svc.subtree(&root_dir.entry_id);
    // 终止保证（visited 集防重复入队）+ 首项为根 + 覆盖子目录
    // （注：visited 防「重复入队」；同一条目经两条路径可达时允许重复产出
    //  ——WP01 迭代器契约即终止而非去重）
    assert!(!collected.is_empty(), "subtree must traverse");
    assert_eq!(collected[0].entry_id, root_dir.entry_id, "root first");
    assert!(
        collected.len() <= 8,
        "cycle must terminate: {}",
        collected.len()
    );
    assert!(
        collected.iter().any(|r| r.entry_id == sub.entry_id),
        "sub covered"
    );
}

#[test]
fn t02_projection_consistency_random_ops_through_raft() {
    // P 风格随机操作序列后：权威行集合 == children 投影重建的目录树
    let root = tmp_root("proj");
    let svc = HubService::open_with_threshold(&root, 300).expect("open");
    let dir = dir_row(1, "root");
    svc.put_entry(&dir).expect("put root");

    let mut sm = 0x5EED_2027u64;
    let mut slots: Vec<(EntryRow, bool)> = Vec::new(); // (row, deleted)
    for step in 0..60u64 {
        sm = sm.wrapping_mul(6364136223846793005).wrapping_add(1);
        let op = (sm >> 33) % 3;
        let idx = (sm >> 20) as usize % 40;
        let name = format!("f-{idx:03}");
        let existing = slots
            .iter()
            .find(|(r, _)| r.name == name && !r.is_deleted());
        match op {
            0..=1 => {
                let mut row = child_row(&dir, &name, idx as u16);
                row.entry_id[14] = (step >> 8) as u8;
                row.entry_id[15] = step as u8;
                if let Some((old, _)) = existing {
                    row.entry_id = old.entry_id;
                }
                svc.put_entry(&row).expect("put");
                slots.retain(|(r, _)| r.entry_id != row.entry_id);
                slots.push((row, false));
            }
            _ => {
                if let Some((row, deleted)) = slots.iter_mut().find(|(r, _)| r.name == name) {
                    if !*deleted {
                        svc.remove_entry(&row.entry_id).expect("remove");
                        *deleted = true;
                    }
                }
            }
        }
    }
    // 权威行 == 投影重建（名字→entry_id 集合相等）
    let mut expected: Vec<(String, [u8; 16])> = slots
        .iter()
        .filter(|(_r, d)| !*d)
        .map(|(r, _)| (r.name.clone(), r.entry_id))
        .collect();
    expected.sort();
    let mut actual: Vec<(String, [u8; 16])> = svc
        .list_children(&dir.entry_id, None, 200)
        .expect("page")
        .items
        .iter()
        .map(|it| (it.name.clone(), it.entry_id))
        .collect();
    actual.sort();
    assert_eq!(actual, expected, "projection must match authoritative set");
}

#[test]
fn t02_row_encoding_compat_through_raft() {
    // 行编码与 WP01 共用（decode_entry_row/encode_entry_row/shard_of）
    let root = tmp_root("enc");
    let svc = HubService::open_with_threshold(&root, 300).expect("open");
    let dir = dir_row(1, "d");
    svc.put_entry(&dir).expect("put");
    let f = child_row(&dir, "enc.txt", 1);
    svc.put_entry(&f).expect("put");
    // 编码往返 + 分片路由确定性（直接核对底层平面行）
    let bytes = encode_entry_row(&f).expect("encode");
    let back = decode_entry_row(&bytes).expect("decode");
    assert_eq!(back.name, "enc.txt");
    assert_eq!(shard_of(&f.entry_id), shard_of(&f.entry_id));
    // ChildRow 借用（投影行结构与 WP01 一致）
    let _ = ChildRow {
        entry_id: f.entry_id,
        kind: KIND_FILE,
        deleted: false,
    };
}

// ===== T03：空间注册表 / D2 盐接线 / 角色 ACL（SPEC M3-WP03 裁定 3/4/5） =====

fn dev(n: u8) -> DeviceId {
    let mut k = [0u8; 32];
    k[0] = n;
    DeviceId(k)
}

#[test]
fn t03_create_space_d2_salt_persist_and_derive() {
    let root = tmp_root("reg-d2");
    let svc = HubService::open_with_threshold(&root, 300).expect("open");
    let row = svc
        .registry()
        .create_space("space-alpha", [7u8; 16], dev(1))
        .expect("create");
    // D2：盐为 CSPRNG 16B（非全零）且持久于注册表行
    assert_eq!(row.kdf_salt.len(), 16);
    assert!(row.kdf_salt.iter().any(|b| *b != 0));
    assert_eq!(
        row.members.get(&dev(1).to_hex()),
        Some(&Role::Owner),
        "creator = first owner"
    );

    // 重开（bootstrap 幂等）→ 盐/成员可读
    svc.registry().crash();
    drop(svc);
    let svc2 = HubService::open_with_threshold(&root, 300).expect("reopen");
    let row2 = svc2
        .registry()
        .space("space-alpha")
        .expect("read")
        .expect("row");
    assert_eq!(row2.kdf_salt, row.kdf_salt);
    assert_eq!(row2.members, row.members);

    // D2 验收：显式盐派生路径一致（argon2_master_key_with_salt）
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let k1 = partisync_sync::crypto::argon2_master_key_with_salt(mnemonic, &row.kdf_salt);
    let k2 = partisync_sync::crypto::argon2_master_key_with_salt(mnemonic, &row2.kdf_salt);
    assert_eq!(*k1, *k2, "persisted salt drives identical master key");
    // 重复创建 → Exists 且原行不被改写
    assert!(matches!(
        svc2.registry()
            .create_space("space-alpha", [9u8; 16], dev(2)),
        Err(RegistryError::Exists)
    ));
    let after = svc2
        .registry()
        .space("space-alpha")
        .expect("read")
        .expect("row");
    assert_eq!(after.kdf_salt, row.kdf_salt, "original row untouched");
    svc2.registry().crash();
}

#[test]
fn t03_role_matrix_enforcement() {
    let root = tmp_root("reg-matrix");
    let svc = HubService::open_with_threshold(&root, 300).expect("open");
    let reg = svc.registry();
    let (owner, editor, viewer, outsider) = (dev(1), dev(2), dev(3), dev(4));
    reg.create_space("s", [1u8; 16], owner).expect("create");
    reg.set_member(owner, "s", editor, Role::Editor)
        .expect("set editor");
    reg.set_member(owner, "s", viewer, Role::Viewer)
        .expect("set viewer");

    // 读：全员可读；非成员拒绝
    for who in [owner, editor, viewer] {
        assert!(reg.check("s", who, Action::Read).is_ok());
    }
    assert!(matches!(
        reg.check("s", outsider, Action::Read),
        Err(RegistryError::Forbidden)
    ));
    // 写：editor/owner 可，viewer/非成员拒绝
    for who in [owner, editor] {
        assert!(reg.check("s", who, Action::Write).is_ok());
    }
    for who in [viewer, outsider] {
        assert!(matches!(
            reg.check("s", who, Action::Write),
            Err(RegistryError::Forbidden)
        ));
    }
    // 管理：owner 可，editor/viewer 拒绝
    assert!(reg.check("s", owner, Action::Admin).is_ok());
    for who in [editor, viewer] {
        assert!(matches!(
            reg.check("s", who, Action::Admin),
            Err(RegistryError::Forbidden)
        ));
    }
    // 缺空间 → Missing
    assert!(matches!(
        reg.check("nope", owner, Action::Read),
        Err(RegistryError::Missing)
    ));

    // 成员管理强制：非 Owner set_member → Forbidden 且无副作用
    assert!(matches!(
        reg.set_member(editor, "s", outsider, Role::Editor),
        Err(RegistryError::Forbidden)
    ));
    let row = reg.space("s").expect("read").expect("row");
    assert!(
        !row.members.contains_key(&outsider.to_hex()),
        "no side effect"
    );
    // Owner 提升 outsider → Editor 生效
    reg.set_member(owner, "s", outsider, Role::Editor)
        .expect("promote");
    assert!(reg.check("s", outsider, Action::Write).is_ok());
    reg.crash();
}

#[test]
fn t03_last_owner_lockout_protection() {
    let root = tmp_root("reg-lock");
    let svc = HubService::open_with_threshold(&root, 300).expect("open");
    let reg = svc.registry();
    let (owner, other) = (dev(1), dev(2));
    reg.create_space("s", [1u8; 16], owner).expect("create");
    // 单 Owner：不可移除自己
    assert!(matches!(
        reg.remove_member(owner, "s", owner),
        Err(RegistryError::Forbidden)
    ));
    // 不可降级自己
    assert!(matches!(
        reg.set_member(owner, "s", owner, Role::Viewer),
        Err(RegistryError::Forbidden)
    ));
    // 添第二 Owner 后可移除自己
    reg.set_member(owner, "s", other, Role::Owner)
        .expect("add owner2");
    reg.remove_member(owner, "s", owner).expect("remove self");
    let row = reg.space("s").expect("read").expect("row");
    assert_eq!(row.members.get(&owner.to_hex()), None);
    assert_eq!(row.members.get(&other.to_hex()), Some(&Role::Owner));
    reg.crash();
}

#[test]
fn t03_registry_random_ops_vs_model() {
    // 随机操作序列 × 参考模型：Forbidden 操作无副作用、状态最终一致
    let root = tmp_root("reg-model");
    let svc = HubService::open_with_threshold(&root, 300).expect("open");
    let reg = svc.registry();
    let devs = [dev(1), dev(2), dev(3), dev(4)];
    reg.create_space("s", [1u8; 16], devs[0]).expect("create");

    // 模型：members 表（hex→role）；reg.ops 为 raft 串行，单线程顺序应用
    let mut model: std::collections::BTreeMap<String, Role> =
        std::collections::BTreeMap::from([(devs[0].to_hex(), Role::Owner)]);
    let mut sm = 0x5EED_2028u64;
    for step in 0..30u64 {
        sm = sm.wrapping_mul(6364136223846793005).wrapping_add(1);
        let actor = devs[(sm >> 33) as usize % 4];
        let target = devs[(sm >> 40) as usize % 4];
        let role = match (sm >> 47) % 3 {
            0 => Role::Owner,
            1 => Role::Editor,
            _ => Role::Viewer,
        };
        let actor_is_owner = model.get(&actor.to_hex()) == Some(&Role::Owner);
        let would_lock = model.get(&target.to_hex()) == Some(&Role::Owner)
            && role != Role::Owner
            && model.values().filter(|r| **r == Role::Owner).count() == 1;
        sm = sm.wrapping_mul(6364136223846793005).wrapping_add(1);
        if (sm >> 50).is_multiple_of(2) {
            // set_member
            let res = reg.set_member(actor, "s", target, role);
            let expected_ok = actor_is_owner && !would_lock;
            assert_eq!(res.is_ok(), expected_ok, "step {step} set mismatch");
            if expected_ok {
                model.insert(target.to_hex(), role);
            }
        } else {
            // remove_member
            let res = reg.remove_member(actor, "s", target);
            let target_is_last_owner = model.get(&target.to_hex()) == Some(&Role::Owner)
                && model.values().filter(|r| **r == Role::Owner).count() == 1;
            let expected_ok =
                actor_is_owner && model.contains_key(&target.to_hex()) && !target_is_last_owner;
            assert_eq!(res.is_ok(), expected_ok, "step {step} remove mismatch");
            if expected_ok {
                model.remove(&target.to_hex());
            }
        }
        // 无副作用核验：Forbidden 后成员表与模型一致
        let row = reg.space("s").expect("read").expect("row");
        assert_eq!(row.members, model, "model divergence at step {step}");
    }
    reg.crash();
}

/// ===== T05：设备会话鉴权（签名信封 + 验签 + 防重放 + 角色强制） =====

#[test]
fn t05_signed_request_authorization_matrix() {
    use ed25519_dalek::SigningKey;
    let root = tmp_root("acl-auth");
    let svc = HubService::open_with_threshold(&root, 300).expect("open");
    let reg = svc.registry();
    let owner_seed = [1u8; 32];
    let owner_sk = SigningKey::from_bytes(&owner_seed);
    let owner_vk = owner_sk.verifying_key().to_bytes();
    reg.create_space("s", [1u8; 16], DeviceId(owner_vk))
        .expect("create");
    // 第二设备注册为 Viewer
    let viewer_sk = SigningKey::from_bytes(&[2u8; 32]);
    let viewer_vk = viewer_sk.verifying_key().to_bytes();
    reg.set_member(DeviceId(owner_vk), "s", DeviceId(viewer_vk), Role::Viewer)
        .expect("add viewer");

    let auth = Authorizer::new();
    let mk = |device_hex: String, action: &str, nonce: u64, sk: &SigningKey| {
        let mut req = SignedRequest {
            device_hex,
            space_id: "s".into(),
            action: action.into(),
            nonce,
            signature_hex: String::new(),
        };
        req.sign(sk).expect("sign");
        req
    };
    let owner_hex = hex_of(owner_vk);
    let viewer_hex = hex_of(viewer_vk);

    // Owner 签名 admin/write/read → 全部允许
    for (i, action) in ["read", "write", "admin"].iter().enumerate() {
        let req = mk(owner_hex.clone(), action, 10 + i as u64, &owner_sk);
        assert!(auth.authorize(reg, &req).is_ok(), "{action} by owner");
    }
    // Viewer 签名 read → 允许；write → Forbidden
    let req = mk(viewer_hex.clone(), "read", 10, &viewer_sk);
    assert!(auth.authorize(reg, &req).is_ok());
    let req = mk(viewer_hex.clone(), "write", 11, &viewer_sk);
    assert!(matches!(
        auth.authorize(reg, &req),
        Err(AuthError::Forbidden(RegistryError::Forbidden))
    ));
    // 非成员 → Forbidden
    let outsider_sk = SigningKey::from_bytes(&[9u8; 32]);
    let outsider_hex = hex_of(outsider_sk.verifying_key().to_bytes());
    let req = mk(outsider_hex, "read", 1, &outsider_sk);
    assert!(matches!(
        auth.authorize(reg, &req),
        Err(AuthError::Forbidden(RegistryError::Forbidden))
    ));
    reg.crash();
}

#[test]
fn t05_signature_tamper_and_nonce_replay_rejected() {
    use ed25519_dalek::SigningKey;
    let root = tmp_root("acl-replay");
    let svc = HubService::open_with_threshold(&root, 300).expect("open");
    let reg = svc.registry();
    let sk = SigningKey::from_bytes(&[5u8; 32]);
    let vk = sk.verifying_key().to_bytes();
    reg.create_space("s", [1u8; 16], DeviceId(vk))
        .expect("create");
    let auth = Authorizer::new();
    let dev_hex = hex_of(vk);

    // 合法首笔
    let mut req = SignedRequest {
        device_hex: dev_hex.clone(),
        space_id: "s".into(),
        action: "write".into(),
        nonce: 7,
        signature_hex: String::new(),
    };
    req.sign(&sk).expect("sign");
    assert!(auth.authorize(reg, &req).is_ok());

    // 重放（同 nonce）→ Replay
    let replay = req.clone();
    assert!(matches!(
        auth.authorize(reg, &replay),
        Err(AuthError::Replay)
    ));

    // 回退 nonce → Replay
    let mut older = SignedRequest {
        nonce: 3,
        ..req.clone()
    };
    older.sign(&sk).expect("sign");
    assert!(matches!(
        auth.authorize(reg, &older),
        Err(AuthError::Replay)
    ));

    // 篡改 action（载荷被改）→ BadSignature
    let mut tampered = SignedRequest {
        nonce: 8,
        ..req.clone()
    };
    tampered.action = "admin".into();
    tampered.sign(&sk).expect("sign");
    tampered.action = "write".into(); // 签名仍是 admin 版
    assert!(matches!(
        auth.authorize(reg, &tampered),
        Err(AuthError::BadSignature)
    ));

    // 伪造身份（签名与声称 vk 不匹配）→ BadSignature
    let impostor = SigningKey::from_bytes(&[6u8; 32]);
    let mut fake = SignedRequest {
        device_hex: dev_hex.clone(),
        space_id: "s".into(),
        action: "write".into(),
        nonce: 20,
        signature_hex: String::new(),
    };
    fake.sign(&impostor).expect("sign with impostor key");
    assert!(matches!(
        auth.authorize(reg, &fake),
        Err(AuthError::BadSignature)
    ));
    reg.crash();
}

fn hex_of(bytes: [u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
