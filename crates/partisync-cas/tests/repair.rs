//! WP04 T03 验收（修复限流 + 修复队列）：SPEC 验收
//! 「修复限流：令牌桶实测修复带宽 ≤ 配额（10%）± 抖动容差」与
//! 「修复队列持久化，重启续跑」（proptest 钉住）。

use std::sync::Arc;
use std::time::{Duration, Instant};

use partisync_cas::repair::{RepairItem, RepairLimiter, RepairQueue};
use proptest::prelude::*;

fn tempdir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cas-repair-it-{tag}-{}",
        partisync_core::Ulid::now()
    ));
    dir
}

// ---------------------------------------------------------------------------
// 令牌桶限流（SPEC 验收：≤ 配额 ± 抖动容差）
// ---------------------------------------------------------------------------

#[test]
fn limiter_budget_bound_over_window() {
    // 1MB/s 配额，1MB 桶；连续 200ms 尝试累加获取；
    // 总消耗上界 = 桶初始容量 + 速率 × 时间 = budget × (1 + T)。
    let budget = 1_000_000u64;
    let lim = RepairLimiter::with_capacity(budget, budget);
    let lim = Arc::new(lim);
    let start = Instant::now();
    let window = Duration::from_millis(200);
    let mut total: u64 = 0;
    while start.elapsed() < window {
        loop {
            let chunk = 4096u64;
            if lim.try_acquire(chunk) {
                total = total.saturating_add(chunk);
            } else {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let elapsed_secs = start.elapsed().as_secs_f64();
    // 上界 = 桶 + 速率 × T × (1 + 5% 抖动)；5% 容差吸收时钟漂移。
    let allowed = (budget as f64 * (1.0 + elapsed_secs) * 1.05).ceil() as u64;
    assert!(
        total <= allowed,
        "limiter over-budget: total={total} allowed≤{allowed} elapsed={elapsed_secs:.3}s",
    );
}

proptest! {
    /// 桶内不变量：rate=0 时连续 try_acquire 累加获取量 ≤ capacity。
    /// 不模拟时间推进——只有初始桶容量的字节可获取；超容量的 try 必返 false。
    #[test]
    fn prop_limiter_no_refill_no_overdraft(
        capacity in 1u64..10_000u64,
        ops in proptest::collection::vec(1u64..512u64, 1..100),
    ) {
        let lim = RepairLimiter::with_capacity(0, capacity);
        let mut total: u64 = 0;
        for op in ops {
            if lim.try_acquire(op) {
                total = total.saturating_add(op);
            }
            prop_assert!(total <= capacity, "overdraft: total={total} capacity={capacity}");
        }
    }
}

// ---------------------------------------------------------------------------
// 持久化修复队列（SPEC 验收：重启续跑）
// ---------------------------------------------------------------------------

#[test]
fn queue_persists_across_close_reopen() {
    let dir = tempdir("persist");
    {
        let q = RepairQueue::open(&dir).expect("open1");
        for i in 0..5u8 {
            q.enqueue(&RepairItem {
                pack_id: format!("pack-{i}"),
                shard_idx: i,
                retry_count: 0,
                last_attempt_ns: 0,
                backoff_ns: 0,
            })
            .unwrap();
        }
        q.persist().unwrap();
    }
    let q2 = RepairQueue::open(&dir).expect("reopen");
    assert_eq!(q2.len(), 5);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    let due = q2.take_due(100, now).unwrap();
    assert_eq!(due.len(), 5);
    let _ = std::fs::remove_dir_all(&dir);
}

proptest! {
    /// 队列幂等：相同 (pack_id, shard_idx) 多次 enqueue 仍只占一条。
    #[test]
    fn prop_queue_enqueues_are_idempotent(
        pack_id in "[a-z0-9]{4,16}",
        shard_idx in 0u8..14,
    ) {
        let dir = tempdir("idem");
        let q = RepairQueue::open(&dir).unwrap();
        for _ in 0..5 {
            q.enqueue(&RepairItem {
                pack_id: pack_id.clone(),
                shard_idx,
                retry_count: 0,
                last_attempt_ns: 0,
                backoff_ns: 0,
            })
            .unwrap();
        }
        q.persist().unwrap();
        assert_eq!(q.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// take_due 严格到期：last_attempt + backoff ≤ now 才返回；不满足
    /// 的条目即使存在也不返回（不影响其它条目的可见性）。
    #[test]
    fn prop_take_due_respects_backoff(
        n in 2usize..=20usize,
        now_offset_ns in 0u64..1_000_000_000u64,
    ) {
        let dir = tempdir("due");
        let q = RepairQueue::open(&dir).unwrap();
        let now: u64 = 1_700_000_000_000_000_000; // 固定起点
        for i in 0..n {
            q.enqueue(&RepairItem {
                pack_id: format!("p{i}"),
                shard_idx: i as u8,
                retry_count: 0,
                last_attempt_ns: now,
                backoff_ns: (i as u64 + 1) * 1_000_000, // 1ms / 2ms / ...
            })
            .unwrap();
        }
        q.persist().unwrap();
        let probe = now + now_offset_ns;
        let due = q.take_due(n, probe).unwrap();
        // 返回的每条都必须真正到期
        for item in &due {
            assert!(item.last_attempt_ns + item.backoff_ns <= probe);
        }
        // 未到期的条目不应被返回
        let expected_due_count = (0..n)
            .filter(|i| (i + 1) as u64 * 1_000_000 <= now_offset_ns)
            .count();
        prop_assert_eq!(due.len(), expected_due_count);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
