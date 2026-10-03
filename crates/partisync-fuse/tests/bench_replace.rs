//! bench（M8-WP07-T05；SPEC §3）：整文件替换延迟（1 MiB / 64 MiB）+
//! `/by-hash` 读与一期目录透传读对照（drop-caches 口径为容器外手动档，
//! 本测试取进程内热缓存数字，报告注明口径差异）。

use std::time::Instant;

use partisync_fuse::writeback::WriteBackLog;

fn gen(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| (i as u8).wrapping_mul(seed).wrapping_add(seed))
        .collect()
}

#[test]
fn bench_whole_file_replace_latency() {
    let tmp = tempfile::tempdir().expect("tmp");
    let backing = tmp.path().join("backing");
    std::fs::create_dir_all(&backing).expect("backing");

    for (label, size) in [("1MiB", 1 << 20), ("64MiB", 64 << 20)] {
        let data = gen(size, 7);
        std::fs::write(backing.join("f.bin"), &data).expect("seed");
        let log = WriteBackLog::open(&backing).expect("open");

        // 整文件替换端到端：staging 写入 + WAL append + 压实 + recover 重放
        // （真实挂载面 close 序列的宿主侧等价；rename 到位含在内）。
        let mut samples = Vec::new();
        for round in 0..5u32 {
            let payload = gen(size, 7 + round as u8);
            let t0 = Instant::now();
            let sname = format!("b{round}.staged");
            std::fs::write(log.staging_path(&sname), &payload).expect("stage");
            log.append(&partisync_fuse::writeback::WriteBackOp::Replace {
                path: "f.bin".into(),
                staging: sname,
            })
            .expect("append");
            log.recover().expect("recover");
            samples.push(t0.elapsed().as_millis() as u64);
        }
        samples.sort();
        let p50 = samples[2];
        let p95 = samples[4];
        println!("replace-{label}: samples={samples:?} p50={p50}ms p95={p95}ms");
        std::fs::remove_file(backing.join("f.bin")).ok();
        std::fs::write(backing.join("f.bin"), &data).ok();
    }
}

#[test]
fn bench_by_hash_read_vs_dir_read() {
    let tmp = tempfile::tempdir().expect("tmp");
    let backing = tmp.path().join("backing");
    let cas = tmp.path().join("cas");
    std::fs::create_dir_all(&backing).expect("backing");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt");
    let store = rt
        .block_on(partisync_cas::ChunkStore::open(&cas))
        .expect("cas");

    let payload = gen(4 << 20, 11); // 4 MiB
    let digest = rt.block_on(async { store.put(&payload).await.expect("put") });
    // 目录视图同内容（对照面：一期目录透传读）
    std::fs::write(backing.join("same.bin"), &payload).expect("seed");

    // 热缓存口径（进程内；drop-caches 手动档见报告）
    let t0 = Instant::now();
    let cas_bytes = rt.block_on(async { store.get(&digest).await.expect("cas get") });
    let cas_ms = t0.elapsed().as_millis() as u64;
    let t1 = Instant::now();
    let dir_bytes = std::fs::read(backing.join("same.bin")).expect("dir read");
    let dir_ms = t1.elapsed().as_millis() as u64;

    assert_eq!(cas_bytes, dir_bytes, "by-hash 读与目录读内容一致");
    println!("read-4MiB: by-hash(cas)={cas_ms}ms dir-passthrough={dir_ms}ms");
    // 口径：不劣于一期 = cas 读不高于目录透传 3 倍（同一磁盘，均为 read()）
    assert!(
        cas_ms <= dir_ms.saturating_mul(3).max(50),
        "by-hash 读不得显著劣于目录透传（cas={cas_ms}ms dir={dir_ms}ms）"
    );
    // 写出防优化
    let _ = std::fs::write(tmp.path().join("sink"), cas_bytes.len().to_le_bytes());
}
