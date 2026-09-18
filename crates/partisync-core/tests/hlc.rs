//! `core::hlc` 规格测试（SPEC M0-WP01，S3 测试先行）。
//!
//! P5 不变量：任意操作序列下严格单调；recv 后严格大于 remote；
//! 跨设备全序（排序键 (phys, logic, device)）。

use partisync_core::hlc::{Hlc, HlcError};
use proptest::prelude::*;

// ---------- 初始状态 ----------

#[test]
fn new_starts_at_zero() {
    let h = Hlc::new(7);
    assert_eq!((h.phys_ms(), h.logic(), h.device()), (0, 0, 7));
}

#[test]
fn from_wall_takes_wall_clock() {
    let h = Hlc::from_wall(7, 1234);
    assert_eq!((h.phys_ms(), h.logic(), h.device()), (1234, 0, 7));
}

#[test]
fn from_raw_roundtrip() {
    // oplog 持久化重建：字段提取 == 原始三元组
    let h = Hlc::from_raw(9, 555, 42);
    assert_eq!((h.device(), h.phys_ms(), h.logic()), (9, 555, 42));
}

// ---------- tick（本地事件） ----------

#[test]
fn tick_wall_advance_resets_logic() {
    let mut h = Hlc::from_wall(1, 100);
    h.tick(200).unwrap();
    assert_eq!(
        (h.phys_ms(), h.logic()),
        (200, 0),
        "墙钟前进 ⇒ phys 更新、logic 归零"
    );
}

#[test]
fn tick_wall_regression_keeps_phys_and_bumps_logic() {
    // P5 核心：墙钟回拨时逻辑位保序
    let mut h = Hlc::from_wall(1, 100);
    h.tick(100).unwrap();
    assert_eq!((h.phys_ms(), h.logic()), (100, 1), "同毫秒 ⇒ logic+1");
    h.tick(50).unwrap();
    assert_eq!(
        (h.phys_ms(), h.logic()),
        (100, 2),
        "回拨 ⇒ 保持 phys、logic 续增"
    );
}

#[test]
fn tick_is_strictly_increasing() {
    let mut h = Hlc::new(1);
    let mut prev = h;
    for w in [50u64, 50, 40, 30, 60, 60, 1] {
        h.tick(w).unwrap();
        assert!(h > prev, "tick 后必须严格大于前值");
        prev = h;
    }
}

#[test]
fn tick_overflow_returns_err() {
    // 经 from_raw 构造边界状态（合法状态域），验证溢出错误而非 panic
    let mut h = Hlc::from_raw(1, 100, u32::MAX);
    assert_eq!(h.tick(100), Err(HlcError::CounterOverflow));
    // 溢出后状态不变（操作原子性）
    assert_eq!((h.phys_ms(), h.logic()), (100, u32::MAX));
    // 墙钟前进时可越过逻辑位溢出
    assert!(h.tick(101).is_ok());
    assert_eq!((h.phys_ms(), h.logic()), (101, 0));
}

// ---------- recv（远端事件合并） ----------

#[test]
fn recv_merge_semantics() {
    // c = max(w, phys, r.phys) 四分支逐一验证（v1.1：wall 领先分支由属性测试发现）
    let mut a = Hlc::from_wall(1, 100);
    let r = Hlc::from_raw(2, 300, 5);
    a.recv(200, r).unwrap();
    assert_eq!(
        (a.phys_ms(), a.logic()),
        (300, 6),
        "r.phys 最大 ⇒ logic = r.logic+1"
    );

    let mut b = Hlc::from_wall(1, 500);
    b.recv(200, r).unwrap();
    assert_eq!(
        (b.phys_ms(), b.logic()),
        (500, 1),
        "自身 phys 最大 ⇒ logic+1"
    );

    let mut c = Hlc::from_raw(1, 300, 7);
    c.recv(200, Hlc::from_raw(2, 300, 5)).unwrap();
    assert_eq!((c.phys_ms(), c.logic()), (300, 8), "同 phys ⇒ max(logic)+1");

    // v1.1 新增：wall 同时领先双方 ⇒ 新纪元 logic = 1（接收事件本身计数）
    let mut d = Hlc::from_wall(1, 100);
    d.recv(900, Hlc::from_raw(2, 300, 5)).unwrap();
    assert_eq!(
        (d.phys_ms(), d.logic()),
        (900, 1),
        "wall 领先 ⇒ phys=wall、logic=1"
    );
}

#[test]
fn recv_is_strictly_greater_than_remote() {
    let mut a = Hlc::from_wall(1, 100);
    let r = Hlc::from_raw(2, 900, 77);
    a.recv(50, r).unwrap();
    assert!(a > r, "recv 后必须严格大于 remote（去重需要）");
}

#[test]
fn recv_overflow_returns_err() {
    let mut a = Hlc::from_raw(1, 100, u32::MAX);
    let r = Hlc::from_raw(2, 100, 5);
    assert_eq!(a.recv(100, r), Err(HlcError::CounterOverflow));
    // 操作原子性：失败后状态不变（同 tick_overflow_returns_err）
    assert_eq!((a.phys_ms(), a.logic()), (100, u32::MAX));
}

// ---------- 跨设备全序（P5 后半） ----------

#[test]
fn total_order_across_devices() {
    let a = Hlc::from_raw(1, 100, 5);
    let b = Hlc::from_raw(2, 100, 5);
    assert_ne!(a, b, "同 phys 同 logic 不同 device ⇒ 不相等");
    assert!(a < b, "device 参与排序键");

    let c = Hlc::from_raw(9, 100, 4);
    assert!(a > c, "phys 相同 ⇒ logic 先于 device 比较");

    let d = Hlc::from_raw(1, 99, u32::MAX);
    assert!(a > d, "phys 优先于一切");
}

// ---------- P5 属性测试 ----------

#[derive(Debug, Clone)]
enum Op {
    Tick(u64),
    Recv(u64, u64, u32), // wall, remote_phys, remote_logic
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0u64..1000).prop_map(Op::Tick),
        (0u64..1000, 0u64..1000, 0u32..1000).prop_map(|(w, p, l)| Op::Recv(w, p, l)),
    ]
}

proptest! {
    /// P5: 任意墙钟（含回拨）与任意事件序列 ⇒ 严格单调、recv 后 self > remote、
    /// 状态域内（logic < MAX-1000）无错误。
    #[test]
    fn prop_monotonic_under_chaos(
        device in 0u64..16,
        start in 0u64..1000,
        ops in prop::collection::vec(op_strategy(), 1..200),
    ) {
        let mut h = Hlc::from_wall(device, start);
        for op in ops {
            let prev = h;
            match op {
                Op::Tick(w) => h.tick(w).unwrap(),
                Op::Recv(w, rp, rl) => {
                    let remote = Hlc::from_raw((device + 1) % 16, rp, rl);
                    h.recv(w, remote).unwrap();
                    prop_assert!(h > remote, "recv 后 self > remote");
                }
            }
            prop_assert!(h > prev, "每步严格递增");
        }
    }
}
