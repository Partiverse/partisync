//! T03 crash 矩阵（SPEC M8-WP07 §3「crash 矩阵」）：kill -9 两相位等价
//! 模拟——①Replace 日志已落盘、rename 未执行（recover 重放应用）；
//! ②staging 有 blob、日志未落盘（「暂存中 crash」→ backing 本就未动，
//! recover 清扫孤儿）。两相位后 backing 均无半提交；重放幂等。

use std::fs;
use std::path::Path;

use partisync_fuse::writeback::{WriteBackLog, WriteBackOp};
use proptest::prelude::*;

fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for e in fs::read_dir(dir).expect("read_dir").flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p
                    .strip_prefix(root)
                    .expect("strip")
                    .to_string_lossy()
                    .into_owned();
                if rel.starts_with(".partisync-writeback") {
                    continue;
                }
                out.push((rel, fs::read(&p).expect("read")));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// 相位①：WAL Replace 已落盘（blob 已在 staging），rename 前 crash
    /// → recover 重放：backing 内容 = staging 内容（整文件，无半提交）；
    /// 再 recover 一次结果不变（幂等）。
    #[test]
    fn t03_crash_phase_a_replace_wal_applied_on_recover(
        content in proptest::collection::vec(any::<u8>(), 0..4096),
        old_len in 1usize..64,
    ) {
        let tmp = tempfile::tempdir().expect("tmp");
        let backing = tmp.path().join("backing");
        fs::create_dir_all(&backing).expect("backing");
        let old: Vec<u8> = (0..old_len).map(|i| (i % 251) as u8 + 1).collect();
        fs::write(backing.join("f.bin"), &old).expect("seed");

        // staging blob 落盘（「写完 staging」相位）
        let log = WriteBackLog::open(&backing).expect("open");
        let staging = log.staging_path("s1.staged");
        fs::write(&staging, &content).expect("stage blob");
        // 日志先落盘（P16），rename（apply）前 crash：
        log.append(&WriteBackOp::Replace {
            path: "f.bin".into(),
            staging: "s1.staged".into(),
        }).expect("append");
        drop(log);

        // 重挂 recover：整文件替换生效
        let log = WriteBackLog::open(&backing).expect("reopen");
        log.recover().expect("recover");
        assert_eq!(fs::read(backing.join("f.bin")).expect("read"), content,
            "recover 后 backing 应为 staging 内容（整文件替换，无半提交）");
        // 再 recover 一次：幂等（无变化）
        let log = WriteBackLog::open(&backing).expect("reopen 2");
        log.recover().expect("recover 2");
        assert_eq!(fs::read(backing.join("f.bin")).expect("read"), content);
    }

    /// 相位②：staging blob 在、日志未落盘（「暂存中 crash」）→ recover
    /// 清扫孤儿 → backing 保持旧内容（丢弃写不产生半提交）。
    #[test]
    fn t03_crash_phase_b_orphan_staging_swept(
        orphan in proptest::collection::vec(any::<u8>(), 1..2048),
        old in proptest::collection::vec(any::<u8>(), 1..64),
    ) {
        let tmp = tempfile::tempdir().expect("tmp");
        let backing = tmp.path().join("backing");
        fs::create_dir_all(&backing).expect("backing");
        fs::write(backing.join("f.bin"), &old).expect("seed");

        // staging 孤儿 + 空 WAL（append 未发生 = 「暂存中未落盘」相位）
        let log = WriteBackLog::open(&backing).expect("open");
        fs::write(log.staging_path("orphan.staged"), &orphan).expect("stage orphan");
        drop(log);

        let log = WriteBackLog::open(&backing).expect("reopen");
        log.recover().expect("recover");
        assert_eq!(fs::read(backing.join("f.bin")).expect("read"), old,
            "孤儿暂存不得影响 backing（丢弃写无半提交）");
        // 孤儿被清扫
        let staging = backing.join(".partisync-writeback").join("staging");
        let n = fs::read_dir(&staging).expect("staging").count();
        assert_eq!(n, 0, "recover 应清扫 staging 孤儿");
    }

    /// 混合序列 crash：随机 unlink/rename/replace 追加未应用 → recover
    /// 重放两次，终态一致（扩展 P16 至 Replace 操作）。
    #[test]
    fn t03_crash_mixed_sequence_replay_converges(
        ops in proptest::collection::vec((any::<u8>(), any::<u8>(), proptest::collection::vec(any::<u8>(), 0..256)), 0..16),
    ) {
        let tmp = tempfile::tempdir().expect("tmp");
        let backing = tmp.path().join("backing");
        fs::create_dir_all(&backing).expect("backing");
        for i in 0..3u8 {
            fs::write(backing.join(format!("f{i}")), b"old").expect("seed");
        }

        // 解释为日志操作（kind: 0=Replace 1=Rename 2=skip）；
        // 引用不存在文件的项由 recover 的 NotFound=已生效 收敛。
        let log = WriteBackLog::open(&backing).expect("open");
        for (idx, (kind, i, payload)) in ops.iter().enumerate() {
            let i = i % 3;
            match kind % 3 {
                0 => {
                    let sname = format!("s{idx}.staged");
                    fs::write(log.staging_path(&sname), payload).expect("stage");
                    log.append(&WriteBackOp::Replace {
                        path: format!("f{i}"),
                        staging: sname,
                    })
                    .expect("append");
                }
                1 => {
                    log.append(&WriteBackOp::Rename {
                        from: format!("f{i}"),
                        to: format!("g{idx}"),
                    })
                    .expect("append");
                }
                _ => {}
            }
        }
        drop(log);
        let log = WriteBackLog::open(&backing).expect("reopen");
        log.recover().expect("recover 1");
        let snap1 = snapshot(&backing);
        let log = WriteBackLog::open(&backing).expect("reopen 2");
        log.recover().expect("recover 2");
        let snap2 = snapshot(&backing);
        prop_assert_eq!(snap1, snap2, "两次 recover 终态必须一致（重放幂等）");
    }
}
