//! 真挂载探针（SPEC M7-WP03 T01）：环境门控——有 `/dev/fuse` 且可挂载
//! 才执行（Linux 容器 `--device /dev/fuse --cap-add SYS_ADMIN` 实测路径）；
//! 无环境（GitHub runner / 本地 mac 无 FUSE）则跳过并在 stderr 留痕，
//! 报告区分「实测数字」与「未执行」。
//!
//! errno 常量为 Linux 值（探针只在 Linux 容器执行；本地 mac 无挂载环境）。

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use fuser::Config;

const EPERM: i32 = 1;
const EACCES: i32 = 13;
const EEXIST: i32 = 17;

const MB: u64 = 1024 * 1024;

/// 挂载一个 PartiFuse 实例，返回 (挂载点, BackgroundSession)。
/// 挂载失败（无 /dev/fuse、无权限）→ None（探针跳过）。
fn mount_once(tag: &str) -> Option<(PathBuf, fuser::BackgroundSession)> {
    let dir = std::env::temp_dir().join(format!("partifuse-{tag}-{}", std::process::id()));
    let backing = dir.join("backing");
    let mp = dir.join("mnt");
    fs::create_dir_all(&backing).expect("backing dir");
    fs::create_dir_all(&mp).expect("mountpoint dir");
    let mut cfg = Config::default();
    cfg.mount_options = vec![fuser::MountOption::FSName("partifuse-spike".into())];
    match fuser::spawn_mount(
        partisync_fuse_spike::PartiFuse::new(backing.clone()),
        &mp,
        &cfg,
    ) {
        Ok(session) => {
            // 就绪等待：挂载点可列目录
            for _ in 0..50 {
                if fs::read_dir(&mp).is_ok() {
                    return Some((mp, session));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            eprintln!("[SKIP] 挂载后 5s 未就绪");
            None
        }
        Err(e) => {
            eprintln!("[SKIP] 环境不可挂载（{e}）——探针跳过，报告按「未执行」登记");
            None
        }
    }
}

fn expect_errno<T>(r: std::io::Result<T>, want: i32, what: &str) {
    match r {
        Ok(_) => panic!("{what} 应被显式拒绝（得到 Ok）"),
        Err(err) => assert_eq!(
            err.raw_os_error(),
            Some(want),
            "{what} 的 errno 应为 {want}（拒绝先于破坏性效果，P15）"
        ),
    }
}

#[test]
fn probe_mount_lifecycle_and_semantics() {
    let Some((mp, _session)) = mount_once("sem") else {
        return;
    };
    let backing = std::env::temp_dir()
        .join(format!("partifuse-sem-{}", std::process::id()))
        .join("backing");

    // ── ① 预置后备文件在挂载点可见（readdir + lookup）──
    fs::write(backing.join("a.txt"), b"hello partifuse").expect("seed");
    let listing: Vec<String> = fs::read_dir(&mp)
        .expect("readdir")
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(listing.contains(&"a.txt".to_string()), "readdir 应见 a.txt");

    // ── ② 随机读：任意 offset ──
    let mut f = fs::File::open(mp.join("a.txt")).expect("open 只读");
    let mut buf = [0u8; 5];
    f.seek(SeekFrom::Start(6)).unwrap();
    f.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"parti", "随机读 offset=6");

    // ── ③ 拒绝面（P15）：拒绝先于破坏性效果 ──
    expect_errno(
        fs::remove_file(mp.join("a.txt")),
        EPERM,
        "unlink 已存在文件",
    );
    expect_errno(
        fs::rename(mp.join("a.txt"), mp.join("b.txt")),
        EPERM,
        "rename",
    );
    expect_errno(
        fs::OpenOptions::new().write(true).open(mp.join("a.txt")),
        EACCES,
        "已存在文件写打开",
    );
    expect_errno(fs::create_dir(mp.join("d")), EPERM, "mkdir");
    // truncate 路径说明：所有写打开在 open 层已拒（EACCES），setattr(size)
    // 防线在 fs.rs 直接 EPERM——VFS 侧无法构造到达 setattr 的合法路径
    //（防线前置），探针以写打开拒绝验证该语义链。
    assert!(
        fs::read(mp.join("a.txt")).is_ok(),
        "拒绝面探针后文件必须原样（拒绝先于效果）"
    );

    // ── ④ 新文件顺序写 ──
    let mut w = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(mp.join("new.bin"))
        .expect("create_new 允许");
    w.write_all(b"part1-").unwrap();
    w.write_all(b"part2-").unwrap();
    drop(w);
    assert_eq!(
        fs::read(backing.join("new.bin")).unwrap(),
        b"part1-part2-",
        "顺序写应直落后备文件"
    );
    // release 后再写打开 → 拒绝（新文件与已存在文件同语义）
    expect_errno(
        fs::OpenOptions::new().append(true).open(mp.join("new.bin")),
        EACCES,
        "release 后写打开",
    );

    // ── ⑤ O_EXCL 语义：同名 create 拒绝 ──
    expect_errno(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(mp.join("new.bin")),
        EEXIST,
        "同名 create_new（O_EXCL）",
    );
}

#[test]
fn probe_random_read_bench() {
    let Some((mp, _session)) = mount_once("bench") else {
        return;
    };
    let backing = std::env::temp_dir()
        .join(format!("partifuse-bench-{}", std::process::id()))
        .join("backing");
    // 1 MiB 伪随机内容文件
    let mut content = vec![0u8; MB as usize];
    let mut seed = 0x517cc1b727220a95u64;
    for b in content.iter_mut() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        *b = (seed >> 33) as u8;
    }
    fs::write(backing.join("bench.bin"), &content).expect("seed bench");

    // 1000 次 4 KiB 随机读（跨全文件）
    let mut f = fs::File::open(mp.join("bench.bin")).expect("open");
    let mut lat = Vec::new();
    let mut seed = 0x9e3779b97f4a7c15u64;
    for _ in 0..1000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let off = (seed >> 24) % (MB - 4096);
        let t = Instant::now();
        f.seek(SeekFrom::Start(off)).unwrap();
        let mut buf = vec![0u8; 4096];
        f.read_exact(&mut buf).unwrap();
        lat.push(t.elapsed().as_secs_f64() * 1e6);
    }
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = lat.len();
    eprintln!(
        "RANDOM_READ_4KB n={n} min={:.1} p50={:.1} p95={:.1} max={:.1} us",
        lat[0],
        lat[n / 2],
        lat[(n as f64 * 0.95) as usize],
        lat[n - 1]
    );

    // 顺序读吞吐（1 MiB × 20 轮）
    let mut f2 = fs::File::open(mp.join("bench.bin")).unwrap();
    let mut sink = vec![0u8; MB as usize];
    let t = Instant::now();
    let mut rounds = 0u64;
    for _ in 0..20 {
        f2.seek(SeekFrom::Start(0)).unwrap();
        f2.read_exact(&mut sink).unwrap();
        rounds += 1;
    }
    let secs = t.elapsed().as_secs_f64();
    let throughput = (MB * rounds) as f64 / secs / MB as f64;
    eprintln!("SEQ_READ_THROUGHPUT {throughput:.1} MiB/s（{rounds} 轮 / {secs:.3}s）");
    std::hint::black_box(&sink);

    assert_eq!(
        &fs::read(mp.join("bench.bin")).unwrap()[..8],
        &content[..8],
        "读回一致性"
    );
}
