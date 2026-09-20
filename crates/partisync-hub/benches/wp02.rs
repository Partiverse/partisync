//! WP02 宏基准（SPEC M3-WP02 T07）：复制导入吞吐 / failover / 转移实测。
//!
//! 非 criterion 微基准——3 节点进程内组（各节点独立 DB + loopback TCP），
//! 参数驱动 + Instant 计时 + 报告人工登记（M2 KPI 同款口径）。release 下
//! 运行：
//!
//! ```text
//! cargo bench -p partisync-hub --bench wp02 -- import 100000   # 复制导入 N 条
//! cargo bench -p partisync-hub --bench wp02 -- drill 5         # failover/转移各 k 轮
//! ```
//!
//! 库根默认 `{CARGO_TARGET_TMPDIR}/wp02-bench`（cargo clean 即回收）。
//! 口径：导入 = leader 串行 client_write（小写形态，与 WP01 批导入不可
//! 直接比——衰减系数按本口径登记）；failover/转移 = 演练 WallClock。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use openraft::error::ClientWriteError;
use partisync_hub::replica::{NodeConfig, Replica, ReplicaError, WriteHandle};

const PAYLOAD_LEN: usize = 128;
/// 在途写窗口（openraft 对在途写合批刷盘——流水线口径，window=1 即串行）。
const WINDOW: usize = 64;

fn bench_root() -> PathBuf {
    let base = option_env!("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("wp02-bench")
}

fn alloc_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port()
}

/// 起一个 3 节点组（演练口径：选举 300-600ms / 心跳 50ms，T05 稳定形态）。
async fn spawn_cluster(root: &std::path::Path, tag: &str) -> Vec<Replica> {
    let ids: Vec<u64> = (1..=3).collect();
    let addrs: BTreeMap<u64, String> = ids
        .iter()
        .map(|id| (*id, format!("127.0.0.1:{}", alloc_port())))
        .collect();
    let mut nodes = Vec::new();
    for id in ids {
        let cfg = NodeConfig {
            node_id: id,
            addr: addrs[&id].clone(),
            db_root: root.join(format!("{tag}-n{id}")),
            group_id: 1,
            members: addrs.clone(),
            election_timeout_ms: (300, 600),
            heartbeat_interval_ms: 50,
        };
        nodes.push(Replica::open(&cfg).await.expect("open replica"));
    }
    for n in &nodes {
        n.bootstrap().await.expect("bootstrap");
    }
    nodes
}

fn percentile(mut xs: Vec<u128>) -> (u128, u128, u128) {
    xs.sort_unstable();
    (
        xs[xs.len() / 2],
        xs[(xs.len() as f64 * 0.99).round() as usize - 1],
        xs[xs.len() - 1],
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or_else(|| {
        eprintln!("用法: wp02-bench <import N | drill [k]>");
        std::process::exit(2);
    });
    let rt = tokio::runtime::Runtime::new().expect("rt");
    match cmd {
        "import" => {
            let n: u64 = args.get(2).expect("N").parse().expect("N 数值");
            rt.block_on(import(n));
        }
        "drill" => {
            let k: u64 = args.get(2).map(|s| s.parse().expect("k")).unwrap_or(3);
            rt.block_on(drill(k));
        }
        other => {
            eprintln!("未知子命令: {other}");
            std::process::exit(2);
        }
    }
}

async fn import(n: u64) {
    let root = bench_root();
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("mkdir");
    let nodes = spawn_cluster(&root, "import").await;
    let leader_id = nodes[0]
        .wait_leader(Duration::from_secs(10))
        .await
        .expect("leader");

    let payload: Vec<u8> = (0..PAYLOAD_LEN).map(|i| (i % 251) as u8).collect();
    let started = Instant::now();
    let mut last_idx = 0u64;
    let mut pending = Vec::with_capacity(WINDOW);
    let mut acked = 0u64;
    let mut cur = leader_id; // 跟随易主：选举切换后 ForwardToLeader 指路重试

    // 提交一条（leader 易主自动跟随）
    async fn submit_one(nodes: &[Replica], cur: &mut u64, payload: &[u8]) -> WriteHandle {
        loop {
            let node = nodes.iter().find(|x| x.node_id() == *cur).expect("node");
            match node.submit(payload.to_vec()).await {
                Ok(h) => return h,
                Err(ReplicaError::ClientWrite(ClientWriteError::ForwardToLeader(f))) => {
                    if let Some(id) = f.leader_id {
                        *cur = id;
                    }
                }
                Err(e) => panic!("submit: {e}"),
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    while acked < n {
        while pending.len() < WINDOW && (acked as usize + pending.len()) < n as usize {
            pending.push(submit_one(&nodes, &mut cur, &payload).await);
        }
        // 每轮确认队首一个（FIFO）
        let handle = pending.remove(0);
        match handle.ack().await {
            Ok(idx) => {
                last_idx = last_idx.max(idx);
                acked += 1;
            }
            Err(ReplicaError::ClientWrite(ClientWriteError::ForwardToLeader(f))) => {
                // 该条目未提交：跟随新 leader 重提交（计数不减）
                if let Some(id) = f.leader_id {
                    cur = id;
                }
                let h = submit_one(&nodes, &mut cur, &payload).await;
                pending.insert(0, h);
            }
            Err(e) => panic!("ack: {e}"),
        }
        if acked.is_multiple_of(10_000) && acked > 0 {
            let secs = started.elapsed().as_secs_f64();
            eprintln!(
                "import {acked}/{} ({:.1}%)  rate {:.0}/s",
                n,
                100.0 * acked as f64 / n as f64,
                acked as f64 / secs
            );
        }
    }

    // 全组可读确认（follower applied 追平 = 端到端复制完成口径）
    for node in &nodes {
        node.wait_applied(last_idx, Duration::from_secs(30))
            .await
            .expect("applied");
    }
    let secs = started.elapsed().as_secs_f64();
    println!("== import (3 节点复制，leader 流水线 submit，window={WINDOW}) ==");
    println!("entries: {n}");
    println!("payload_bytes: {PAYLOAD_LEN}");
    println!("wall_s: {secs:.2}");
    println!("throughput_eps: {:.0}", n as f64 / secs);
    println!("decay_vs_wp01_109k: {:.4}", (n as f64 / secs) / 109_000.0);
    for node in &nodes {
        node.crash().await;
    }
}

async fn drill(k: u64) {
    let root = bench_root();
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("mkdir");

    let mut failover_ms = Vec::new();
    let mut transfer_ms = Vec::new();
    for round in 0..k {
        let nodes = spawn_cluster(&root, &format!("drill-{round}")).await;
        let leader_id = nodes[0]
            .wait_leader(Duration::from_secs(10))
            .await
            .expect("leader");
        let leader = nodes
            .iter()
            .find(|n| n.node_id() == leader_id)
            .expect("leader");
        let others: Vec<&Replica> = nodes.iter().filter(|n| n.node_id() != leader_id).collect();

        // 写 8 条 ACK 后 kill leader
        let mut last_idx = 0u64;
        for i in 0..8u64 {
            last_idx = leader
                .write(vec![i as u8; PAYLOAD_LEN])
                .await
                .expect("write");
        }
        let started = Instant::now();
        leader.crash().await;
        let nl;
        loop {
            if let Some(x) = others.iter().find(|n| n.is_leader()) {
                nl = *x;
                break;
            }
            assert!(started.elapsed() < Duration::from_secs(10), "failover >10s");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        nl.wait_applied(last_idx, Duration::from_secs(5))
            .await
            .expect("applied");
        failover_ms.push(started.elapsed().as_millis());

        // 转移：新 leader 停选举 → 幸存者触发选举
        let target = others
            .iter()
            .find(|n| n.node_id() != nl.node_id())
            .expect("target");
        nl.pause_election();
        let t0 = Instant::now();
        target.trigger_elect().await.expect("trigger");
        loop {
            if target.is_leader() {
                break;
            }
            assert!(t0.elapsed() < Duration::from_secs(5), "transfer >5s");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        transfer_ms.push(t0.elapsed().as_millis());

        for n in &nodes {
            n.crash().await;
        }
    }
    let (f_p50, f_p99, f_max) = percentile(failover_ms.clone());
    let (t_p50, t_p99, t_max) = percentile(transfer_ms.clone());
    println!("== drill (k={k}, 演练口径: crash=core 停止，转移=elect 门禁+trigger) ==");
    println!("failover_ms_p50: {f_p50}");
    println!("failover_ms_p99: {f_p99}");
    println!("failover_ms_max: {f_max}");
    println!("transfer_ms_p50: {t_p50}");
    println!("transfer_ms_p99: {t_p99}");
    println!("transfer_ms_max: {t_max}");
}
