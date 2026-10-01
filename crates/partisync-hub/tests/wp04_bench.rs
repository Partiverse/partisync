//! M8-WP04-T03d 基准探针（显式运行：`cargo test -p partisync-hub
//! --test wp04_bench -- --ignored --nocapture`；结果登记
//! docs/reports/bench/M8-WP04-hub-raft.md 与 closure §3）。
//!
//! 口径对照：
//! - RouteQuery P99：基线 M5-WP02 298µs（同 `route_for` 路径）；
//! - hub 写吞吐：raft 单笔提交（无既有基线，本报告为首次建线）——
//!   与 M5-WP04 508k evt/s **不同口径**（那是批量 drain 的 SM apply），
//!   如实标注不硬比。

use partisync_hub::registry::RouteRow;
use partisync_hub::service::{HubCmd, HubService};
use partisync_hub::{EntryRow, KIND_DIR, KIND_FILE};
use std::time::{Duration, Instant};

fn bench_root(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("wp04-bench-{tag}-{}", partisync_core::Ulid::now()))
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

fn percentiles(mut xs: Vec<Duration>) -> (Duration, Duration, Duration, Duration) {
    xs.sort();
    let n = xs.len();
    (xs[n / 2], xs[n * 95 / 100], xs[n * 99 / 100], xs[n - 1])
}

#[test]
#[ignore = "基准：显式运行并登记 docs/reports/bench/M8-WP04-hub-raft.md"]
fn t03d_route_and_apply_bench() {
    let svc = HubService::open(&bench_root("main")).expect("open");
    let dir = dir_row(1, "d");
    svc.put_entry(&dir).expect("put dir");
    svc.registry()
        .set_route(RouteRow {
            space_id: "s-bench".into(),
            hub_id: 1,
            addr: "127.0.0.1:9100".into(),
            epoch: 1,
        })
        .expect("set route");

    // A) RouteQuery P99（n=2000；本地命中 + 未知两路混合）
    let mut lat = Vec::new();
    for i in 0..2000u32 {
        let space = if i % 2 == 0 { "s-bench" } else { "s-unknown" };
        let t = Instant::now();
        svc.route_for(space, 1).expect("route");
        lat.push(t.elapsed());
    }
    let (p50, p95, p99, mx) = percentiles(lat);
    println!(
        "route_for n=2000: p50 {:?} / p95 {:?} / p99 {:?} / max {:?} (基线 M5-WP02 p99 298µs)",
        p50, p95, p99, mx
    );

    // B) raft 单笔提交吞吐（n=1000 顺序 put；首次建线）
    let n = 1000u32;
    let t = Instant::now();
    for i in 0..n {
        svc.put_entry(&file_row(&dir, &format!("b{i:05}"), i as u16, 8))
            .expect("put");
    }
    let wall = t.elapsed();
    println!(
        "raft submit throughput: {} ops in {:?} → {:.0} ops/s（debug 构建；与 M5-WP04 508k evt/s 非同口径）",
        n,
        wall,
        n as f64 / wall.as_secs_f64()
    );

    // C) 幂等提交开销对照（T03a 信封 vs 普通提交；n=300）
    let mut idem = Vec::new();
    for i in 0..300u32 {
        let mut req_id = [0u8; 16];
        req_id[..4].copy_from_slice(&i.to_be_bytes());
        let t = Instant::now();
        svc.submit_idempotent(
            req_id,
            &HubCmd::Put(file_row(&dir, &format!("i{i:05}"), (i % 600) as u16, 8)),
        )
        .expect("idempotent submit");
        idem.push(t.elapsed());
    }
    let (p50i, _, p99i, _) = percentiles(idem);
    println!(
        "submit_idempotent n=300: p50 {:?} / p99 {:?}（含信封编解码 + r-dedup 查写）",
        p50i, p99i
    );
    svc.crash();
    drop(svc);
}
