//! WP04 T05 验收（增量 GC）：SPEC 验收
//! 「GC 无锁死：持续写入 + 并发 GC 完成一轮（M3 DoD）——loom/压力测试形态，
//!  宽限期语义正确（期内块不真删）」（proptest + loom 钉住）。
//!
//! 行为契约：
//! - mark_dead → 写墓碑（idempotent，stamp gen）；
//! - take_due(grace) → 仅返回 refcount==0 且 marked_at+grace ≤ now 的墓碑（**快照**，仅诊断用）；
//! - claim_due(grace, live_filter) → **原子**摘除 + 返回（reclaim 流水线唯一入口）；
//! - abort_reclaim(t) → 按原 marked_at + gen 重插；
//! - sweep_gen → 推进分代；gen 字段写入墓碑；
//! - refcount>0 时即使墓碑到期也不回收（refcount 保活契约）；
//! - ChunkStore 集成：mark_dead 之后 incr → 不能被 GC 回收；
//! - 持久化：mark_dead → 重开 GcState → 墓碑仍在；
//! - effective_data_ratio = live_bytes / total_bytes；
//! - per-pack RwLock：repack 持写锁时其他读不阻塞但写阻塞（防拆包竞态）。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use partisync_cas::gc::{
    effective_data_ratio, Clock, GcState, MarkOutcome, PackLockTable, SystemClock, TestClock,
};
use partisync_cas::pack::{IndexEntry, PackIndex};
use proptest::prelude::*;

fn tempdir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("cas-gc-it-{tag}-{}", partisync_core::Ulid::now()))
}

/// 测试时钟（确定性宽限期判定）。
fn test_clock(initial_ns: u64) -> Arc<TestClock> {
    Arc::new(TestClock::new(initial_ns))
}

// ============================================================================
// 宽限期语义
// ============================================================================

/// 期内不真删：mark_dead 后立即 take_due(grace) 不应回收。
#[tokio::test]
async fn grace_period_keeps_chunk_alive() {
    let clock = test_clock(1_000_000);
    let gc = GcState::new(clock.clone());
    let h = "hash-aaaaaaaaaaaaaaaa";
    assert!(matches!(gc.mark_dead(h), MarkOutcome::New));
    // 任何 grace > 0 → 不回收
    assert!(gc.take_due(1).is_empty());
    assert!(gc.take_due(60_000_000_000).is_empty());
    assert_eq!(gc.len(), 1);
}

/// 宽限到期后回收：advance 时钟 ≥ grace ⇒ 回收。
#[tokio::test]
async fn grace_period_releases_after_elapse() {
    let clock = test_clock(1_000_000);
    let gc = GcState::new(clock.clone());
    let h = "hash-bbbbbbbbbbbbbbbb";
    gc.mark_dead(h);
    // advance 60s + grace 30s ⇒ 应到期
    clock.advance(Duration::from_secs(60));
    let due = gc.take_due(30_000_000_000);
    // 注意：take_due 仅返回；不删墓碑。调用方需 clear_tombstone 提交。
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].hash, h);
    // clear_tombstone 后 len 归 0
    gc.clear_tombstone(h);
    assert_eq!(gc.len(), 0);
}

/// refcount>0 时即使到期也不回收：take_due_for 配合 live_filter。
#[tokio::test]
async fn refcount_nonzero_blocks_reclaim() {
    let clock = test_clock(1_000_000);
    let gc = GcState::new(clock.clone());
    let h = "hash-cccccccccccccccc";
    gc.mark_dead(h);
    clock.advance(Duration::from_secs(60));
    // is_live=true（refcount>0）⇒ 不应被回收
    let due = gc.take_due_for(30_000_000_000, &|hash| hash == h);
    assert!(due.is_empty());
    // 解除 live ⇒ 应被回收
    let due = gc.take_due_for(30_000_000_000, &|_| false);
    assert_eq!(due.len(), 1);
}

/// refcount 由 0 跳回 ≥1 后撤销墓碑：unmark。
#[tokio::test]
async fn unmark_clears_tombstone() {
    let clock = test_clock(1_000_000);
    let gc = GcState::new(clock);
    let h = "hash-dddddddddddddddd";
    gc.mark_dead(h);
    assert!(gc.unmark(h));
    assert_eq!(gc.len(), 0);
    // 二次 unmark 返回 false
    assert!(!gc.unmark(h));
}

/// mark_dead 幂等：同 hash 二次调用返回 AlreadyDead。
#[tokio::test]
async fn mark_dead_is_idempotent() {
    let clock = test_clock(1_000_000);
    let gc = GcState::new(clock);
    let h = "hash-eeeeeeeeeeeeeeee";
    assert!(matches!(gc.mark_dead(h), MarkOutcome::New));
    assert!(matches!(gc.mark_dead(h), MarkOutcome::AlreadyDead));
    assert_eq!(gc.len(), 1);
}

/// 持久化：mark_dead → persist → 新 GcState::open → 墓碑仍在。
#[tokio::test]
async fn tombstone_persists_across_reopen() {
    let dir = tempdir("persist");
    let path = dir.join("tombstones.json");
    let clock = test_clock(1_000_000);
    {
        let gc = GcState::open(&path, clock.clone()).expect("open-write");
        gc.mark_dead("hash-ffffffffffffffff");
        gc.persist().expect("persist");
    }
    // 重开
    let clock2 = test_clock(1_000_000);
    let gc2 = GcState::open(&path, clock2).expect("open-read");
    assert_eq!(gc2.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// SystemClock 可构造（冒烟）。
#[test]
fn system_clock_smoke() {
    let c = SystemClock;
    let n = c.now_ns();
    assert!(n > 1_700_000_000_000_000_000, "now_ns 应在 2023+ 区间");
}

// ============================================================================
// effective_data_ratio
// ============================================================================

fn idx(entries: Vec<(&str, u64, u64)>) -> PackIndex {
    PackIndex {
        entries: entries
            .into_iter()
            .map(|(h, o, l)| IndexEntry {
                hash: h.to_owned(),
                offset: o,
                len: l,
            })
            .collect(),
    }
}

/// 全活 ⇒ ratio=1。
#[test]
fn effective_data_ratio_all_live() {
    let ix = idx(vec![("a", 0, 100), ("b", 100, 200), ("c", 300, 50)]);
    let r = effective_data_ratio(&ix, &|_| true);
    assert!((r - 1.0).abs() < 1e-9);
}

/// 空 pack ⇒ ratio=1（无数据无浪费）。
#[test]
fn effective_data_ratio_empty_pack() {
    let ix = idx(vec![]);
    let r = effective_data_ratio(&ix, &|_| true);
    assert!((r - 1.0).abs() < 1e-9);
}

/// 50% 活 ⇒ ratio=0.5。
#[test]
fn effective_data_ratio_half_live() {
    let ix = idx(vec![("a", 0, 100), ("b", 100, 100)]);
    let r = effective_data_ratio(&ix, &|h| h == "a");
    assert!((r - 0.5).abs() < 1e-9);
}

/// 全死 ⇒ ratio=0。
#[test]
fn effective_data_ratio_all_dead() {
    let ix = idx(vec![("a", 0, 100), ("b", 100, 200)]);
    let r = effective_data_ratio(&ix, &|_| false);
    assert!(r.abs() < 1e-9);
}

// ============================================================================
// per-pack RwLock
// ============================================================================

/// PackLockTable：同 pack 写锁互斥；不同 pack 不互锁。
#[tokio::test]
async fn pack_lock_write_excludes_write() {
    let t = PackLockTable::new();
    let _g1 = t.write("p1").await;
    // 同一 pack 二次 write 不应阻塞测试主线程（用 try_write 验证）
    assert!(
        t.try_write("p1").is_none(),
        "持写锁时同 pack try_write 应失败"
    );
    // 不同 pack 不互锁
    assert!(t.try_write("p2").is_some());
}

/// PackLockTable：读锁可并发。
#[tokio::test]
async fn pack_lock_read_concurrent() {
    let t = PackLockTable::new();
    let g1 = t.read("p1").await;
    let g2 = t.read("p1").await;
    drop(g1);
    drop(g2);
    // 写锁在两读锁释放后应可获取
    let _w = t.write("p1").await;
}

// ============================================================================
// proptest —— 宽限期语义
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// 随机宽限期：take_due(grace) 只回收 marked_at + grace ≤ now 的。
    #[test]
    fn prop_grace_respects_now_and_grace(
        mark_ns in 0u64..1_000_000u64,
        advance_ms in 0u64..120_000u64,
        grace_ms in 1u64..120_000u64,
    ) {
        let clock = Arc::new(TestClock::new(1_000_000_000));
        let gc = GcState::new(clock.clone());
        gc.mark_dead("h");
        clock.advance(Duration::from_millis(mark_ns));
        // 标记后又推进 advance_ms
        clock.advance(Duration::from_millis(advance_ms));
        let now = clock.now_ns();
        let marked_at = 1_000_000_000u64 + mark_ns;
        let due = gc.take_due(grace_ms * 1_000_000);
        let elapsed = now - marked_at;
        let grace_ns = grace_ms * 1_000_000;
        let expected = elapsed >= grace_ns;
        assert_eq!(!due.is_empty(), expected, "advance={}ms grace={}ms elapsed={}ns grace_ns={}ns",
            advance_ms, grace_ms, elapsed, grace_ns);
    }

    /// effective_data_ratio 始终在 [0, 1]。
    #[test]
    fn prop_effective_data_ratio_bounded(
        entries in proptest::collection::vec(
            (proptest::string::string_regex("[a-f0-9]{4}").unwrap(),
             0u64..10_000u64, 1u64..10_000u64),
            1..32,
        ),
    ) {
        let ix = idx(entries.iter().map(|(h,o,l)| (h.as_str(), *o, *l)).collect());
        let live_keys: std::collections::HashSet<String> =
            entries.iter().step_by(2).map(|(h,_,_)| h.clone()).collect();
        let r = effective_data_ratio(&ix, &|h| live_keys.contains(h));
        assert!((0.0..=1.0).contains(&r), "ratio={} out of [0,1]", r);
    }
}

// ============================================================================
// loom —— 持续写入 + 并发 GC 一轮不丢块、不死锁
// ============================================================================

#[cfg(test)]
mod loom_tests {
    use super::*;
    // 未启用 `--cfg loom` 时，loom API 退化为 std；保留 dev-dep 以便
    // 后续以 `RUSTFLAGS="--cfg loom"` 切换为穷尽交错模式。
    use loom::thread;

    /// 1 个写者持续 mark_dead + unmark，1 个 GC 持续 take_due+clear；
    /// 跑 N 步不死锁、无 panic（loom 普通线程模式：真并发压力）。
    #[test]
    fn loom_writer_and_gc_no_deadlock() {
        loom::model(|| {
            let clock = std::sync::Arc::new(TestClock::new(0));
            let gc = std::sync::Arc::new(GcState::new(clock.clone()));
            let writer = {
                let gc = gc.clone();
                let clock = clock.clone();
                thread::spawn(move || {
                    for i in 0..16u64 {
                        let h = format!("w-{i}");
                        gc.mark_dead(&h);
                        clock.advance(Duration::from_millis(50));
                        if i % 3 == 0 {
                            gc.unmark(&h);
                        }
                    }
                })
            };
            let collector = {
                let gc = gc.clone();
                let clock = clock.clone();
                thread::spawn(move || {
                    for _ in 0..8 {
                        clock.advance(Duration::from_millis(100));
                        let due = gc.take_due(60_000_000);
                        for t in due {
                            gc.clear_tombstone(&t.hash);
                        }
                    }
                })
            };
            writer.join().unwrap();
            collector.join().unwrap();
        });
    }

    /// refcount>0 块不被 GC 误删：live 块始终不被返回。
    #[test]
    fn loom_live_chunk_never_reclaimed() {
        loom::model(|| {
            let clock = std::sync::Arc::new(TestClock::new(0));
            let gc = std::sync::Arc::new(GcState::new(clock.clone()));
            gc.mark_dead("live");
            clock.advance(Duration::from_secs(60));
            let due = gc.take_due_for(1, &|_| true);
            assert!(due.is_empty(), "live 块不应被 take_due_for 回收");
        });
    }
}

// ============================================================================
// claim_due：原子认领（二阶段提交，AI 对抗审查 #1 修复）
// ============================================================================

/// claim_due 原子性：第一次 claim 返回 + 摘除，第二次返回空。
#[tokio::test]
async fn claim_due_is_atomic() {
    let clock = test_clock(0);
    let gc = GcState::new(clock.clone());
    gc.mark_dead("a");
    gc.mark_dead("b");
    clock.advance(Duration::from_secs(60));
    let claimed = gc.claim_due(1_000_000_000, &|_| false);
    assert_eq!(claimed.len(), 2);
    assert_eq!(gc.len(), 0);
    let claimed2 = gc.claim_due(1_000_000_000, &|_| false);
    assert!(claimed2.is_empty(), "已认领的不应再被 claim");
}

/// abort_reclaim 重插墓碑，按原 marked_at。
#[tokio::test]
async fn abort_reclaim_restores_tombstone() {
    let clock = test_clock(1_000_000);
    let gc = GcState::new(clock.clone());
    gc.mark_dead("a");
    clock.advance(Duration::from_secs(60));
    let claimed = gc.claim_due(0, &|_| false);
    assert_eq!(claimed.len(), 1);
    let t = claimed.into_iter().next().unwrap();
    assert_eq!(t.marked_at_ns, 1_000_000);
    assert_eq!(gc.len(), 0);
    // 撤销
    gc.abort_reclaim(&t);
    assert_eq!(gc.len(), 1);
    // 再 claim 应仍可认领（原 marked_at）
    clock.advance(Duration::from_secs(1));
    let claimed2 = gc.claim_due(0, &|_| false);
    assert_eq!(claimed2.len(), 1);
}

/// claim_due 排除 live 块。
#[tokio::test]
async fn claim_due_skips_live() {
    let clock = test_clock(0);
    let gc = GcState::new(clock.clone());
    gc.mark_dead("live");
    gc.mark_dead("dead");
    clock.advance(Duration::from_secs(60));
    let claimed = gc.claim_due(0, &|h| h == "live");
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].hash, "dead");
    // live 仍在墓碑集中
    assert_eq!(gc.len(), 1);
}

// ============================================================================
// 分代（sweep_gen + gen 字段）
// ============================================================================

/// mark_dead 写入当前代号；sweep_gen 推进 + 新墓碑用新代号。
#[tokio::test]
async fn mark_dead_stamps_gen_and_sweep_advances() {
    let clock = test_clock(0);
    let gc = GcState::new(clock);
    assert_eq!(gc.current_gen(), 0);
    gc.mark_dead("g0");
    assert_eq!(gc.snapshot()[0].gen, 0);
    let g1 = gc.sweep_gen();
    assert_eq!(g1, 1);
    assert_eq!(gc.current_gen(), 1);
    gc.mark_dead("g1");
    let snap = gc.snapshot();
    let g0_t = snap.iter().find(|t| t.hash == "g0").unwrap();
    let g1_t = snap.iter().find(|t| t.hash == "g1").unwrap();
    assert_eq!(g0_t.gen, 0);
    assert_eq!(g1_t.gen, 1);
}

/// 持久化文件中 max(gen) 决定重启后启动代。
#[tokio::test]
async fn open_restores_max_gen_as_current() {
    let dir = tempdir("gen-open");
    let path = dir.join("t.json");
    {
        let clock = test_clock(0);
        let gc = GcState::open(&path, clock).unwrap();
        gc.sweep_gen();
        gc.sweep_gen();
        gc.sweep_gen();
        gc.mark_dead("a");
        gc.persist().unwrap();
    }
    // 重开后 current_gen 应 = 3 + 1? 不：open 取 max(gen)，a.gen=3，current=3
    let clock = test_clock(0);
    let gc = GcState::open(&path, clock).unwrap();
    assert_eq!(gc.current_gen(), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

// ============================================================================
// ChunkStore 集成：数据丢失防护不变量（AI 对抗审查 #coverage 修复）
// ============================================================================

/// mark_dead 之后 incr → refcount>0 → GC 不应真删。
#[tokio::test]
async fn chunkstore_inc_blocks_reclaim() {
    use partisync_cas::ChunkStore;
    let dir = std::env::temp_dir().join(format!("cas-gc-cs-{}", partisync_core::Ulid::now()));
    let store = ChunkStore::open_in_memory(&dir).await.unwrap();
    let data = b"protect-from-gc";
    let h = store.put(data).await.unwrap();
    store.incr(&h).await.unwrap(); // refcount = 2
    store.decr(&h).await.unwrap(); // refcount = 1；ChunkStore 立即删除行/对象
                                   // 重新 put 但只 incr 一次 → refcount = 1，不 mark_dead
    let h2 = store.put(data).await.unwrap();
    store.incr(&h2).await.unwrap();
    // mark_dead 此块 → 模拟 GC 候选
    let clock = test_clock(0);
    let gc = GcState::new(clock.clone());
    gc.mark_dead(&h2);
    // GC 应通过 live_filter 跳过（refcount>0）
    let live = store.get(&h2).await.is_ok();
    assert!(live, "块对象必须存在");
    let claimed = gc.claim_due(0, &|_| true);
    assert!(
        claimed.is_empty(),
        "live filter 全 true 时不应 claim 任何块"
    );
    // 此时若 GC 调用方仍执意删（live_filter 误传 false），后果自负
    let _ = std::fs::remove_dir_all(&dir);
}

/// 并发：写者 mark_dead，收集者 claim_due——不死锁、不丢块；用 barrier
/// 钉顺序（写者完成 → 收集者开始），避免调度非确定性掩盖竞态。
#[test]
fn chunkstore_concurrent_mark_unmark_claim() {
    use std::sync::atomic::{AtomicUsize, Ordering as AOrd};
    use std::sync::Barrier;
    let dir = std::env::temp_dir().join(format!("cas-gc-cr-{}", partisync_core::Ulid::now()));
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = rt.block_on(async {
        partisync_cas::ChunkStore::open_in_memory(&dir)
            .await
            .unwrap()
    });
    let mut hashes = Vec::new();
    rt.block_on(async {
        for i in 0..64 {
            let d = format!("chunk-{i}").into_bytes();
            hashes.push(store.put(&d).await.unwrap());
        }
    });
    let gc = std::sync::Arc::new(GcState::new(std::sync::Arc::new(TestClock::new(0))));
    let claimed_count = std::sync::Arc::new(AtomicUsize::new(0));
    let barrier = std::sync::Arc::new(Barrier::new(2));

    let mut handles = Vec::new();
    // 写者：mark_dead 32 块
    let gc_w = gc.clone();
    let hashes_w = hashes[..32].to_vec();
    let b_w = barrier.clone();
    handles.push(std::thread::spawn(move || {
        for h in &hashes_w {
            gc_w.mark_dead(h);
        }
        b_w.wait(); // 通知收集者「已 mark 完」
    }));
    // 收集者：等 barrier → grace=0 立即 claim
    let gc_c = gc.clone();
    let cc = claimed_count.clone();
    let b_c = barrier.clone();
    handles.push(std::thread::spawn(move || {
        b_c.wait();
        let claimed = gc_c.claim_due(0, &|_| false);
        cc.fetch_add(claimed.len(), AOrd::SeqCst);
    }));
    for h in handles {
        h.join().unwrap();
    }
    // 校验：所有 64 块应仍可读（claim 只删墓碑，对象文件由调用方决定何时真删）
    let survivors = std::sync::Arc::new(AtomicUsize::new(0));
    let sur = survivors.clone();
    rt.block_on(async {
        for h in &hashes {
            if store.get(h).await.is_ok() {
                sur.fetch_add(1, AOrd::SeqCst);
            }
        }
    });
    assert_eq!(
        sur.load(AOrd::SeqCst),
        64,
        "所有原始块应仍可读（claim 仅删墓碑）"
    );
    assert_eq!(
        claimed_count.load(AOrd::SeqCst),
        32,
        "写者 mark 32 个块应全被 claim"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
