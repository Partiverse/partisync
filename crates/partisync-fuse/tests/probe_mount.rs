//! 产品真挂载探针（P15，SPEC M8-WP01 §3）：环境门控——有 `/dev/fuse` 且
//! 可挂载才执行（Linux 容器 `--device /dev/fuse --cap-add SYS_ADMIN` 实测
//! 路径）；无环境（GitHub runner / 本地 mac 无 FUSE）则跳过并 stderr 留痕
//! （M7 判例）。移植自 spike 探针 + overnight-notes §2 补齐项
//! （rmdir/symlink/mknod 逐项 errno）。
//!
//! errno 常量为 Linux 值（探针只在 Linux 容器执行）。

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::Duration;

use partisync_fuse::PartiFuse;

const EPERM: i32 = 1;
const EACCES: i32 = 13;
const EEXIST: i32 = 17;

/// 挂载一个 PartiFuse 实例，返回 (挂载点, BackgroundSession)。
fn mount_once(tag: &str) -> Option<(PathBuf, fuser::BackgroundSession)> {
    let dir = std::env::temp_dir().join(format!("partifuse-{tag}-{}", std::process::id()));
    let backing = dir.join("backing");
    let mp = dir.join("mnt");
    fs::create_dir_all(&backing).expect("backing dir");
    fs::create_dir_all(&mp).expect("mountpoint dir");
    match partisync_fuse::spawn_mount(PartiFuse::new(backing.clone()), &mp) {
        Ok(session) => {
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

    // ── ① readdir 可见性 ──
    fs::write(backing.join("a.txt"), b"hello partifuse").expect("seed");
    let listing: Vec<String> = fs::read_dir(&mp)
        .expect("readdir")
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(listing.contains(&"a.txt".to_string()), "readdir 应见 a.txt");

    // ── ② 随机读（含块边界 offset）──
    let mut f = fs::File::open(mp.join("a.txt")).expect("open 只读");
    let mut buf = [0u8; 5];
    f.seek(SeekFrom::Start(6)).unwrap();
    f.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"parti", "随机读 offset=6");
    f.seek(SeekFrom::Start(0)).unwrap();
    f.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"hello", "随机读 offset=0");
    // "hello partifuse" = 15 字节；End(-5) = 10
    let end = f.seek(SeekFrom::End(-5)).unwrap();
    assert_eq!(end, 10, "seek End 定位");
    f.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"ifuse", "随机读 offset=End-5");

    // ── ③ 仍拒绝面逐项 errno（P15 延续；写回面开放项见 ⑦）──
    // T02 起 unlink/rmdir/rename 走写回日志路径（SPEC M8-WP07 §2.1），
    // 旧 EPERM 断言移交 ⑦ 的「生效」断言；本节保持「仍拒绝」操作。
    expect_errno(fs::create_dir(mp.join("d")), EPERM, "mkdir");
    expect_errno(
        std::os::unix::fs::symlink("/etc/hostname", mp.join("l")),
        EPERM,
        "symlink",
    );
    expect_errno(
        fs::OpenOptions::new().write(true).open(mp.join("a.txt")),
        EACCES,
        "已存在文件写打开",
    );
    expect_errno(
        fs::OpenOptions::new().append(true).open(mp.join("a.txt")),
        EACCES,
        "已存在文件 append 打开",
    );

    // ── ④ 拒绝后完整性（P15 核心）──
    assert_eq!(
        fs::read(mp.join("a.txt")).unwrap(),
        b"hello partifuse",
        "拒绝面探针后文件必须原样"
    );

    // ── ⑤ 新文件顺序写 ──
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
    expect_errno(
        fs::OpenOptions::new().append(true).open(mp.join("new.bin")),
        EACCES,
        "release 后写打开",
    );

    // ── ⑥ O_EXCL 语义 ──
    expect_errno(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(mp.join("new.bin")),
        EEXIST,
        "同名 create_new",
    );

    // 跳写（offset != cur_len → EINVAL）说明：页缓存下 VFS 不可构造到达
    // 该检查的调用序列（SEMANTICS.md 登记「防线前置」）——探针以 ③⑤⑥
    // 覆盖同语义链的可达面。

    // ── ⑦ 写回面（M8-WP07-T02；SPEC §2.1 + P16）──
    // unlink 经日志路径生效（先日志后应用）：
    fs::remove_file(mp.join("a.txt")).expect("unlink 经写回日志应生效");
    assert!(
        !backing.join("a.txt").exists(),
        "unlink 后 backing 应无 a.txt"
    );
    // rmdir 空目录生效；非空目录 ENOTEMPTY（拒绝先于日志写入）
    fs::create_dir(backing.join("subdir")).expect("seed subdir");
    fs::remove_dir(mp.join("subdir")).expect("rmdir 空目录经写回日志应生效");
    fs::create_dir(backing.join("subfull")).expect("seed");
    fs::write(backing.join("subfull/x"), b"1").expect("seed");
    match fs::remove_dir(mp.join("subfull")) {
        Ok(_) => panic!("非空 rmdir 应被拒"),
        Err(e) => assert_eq!(e.raw_os_error(), Some(39), "非空 rmdir = ENOTEMPTY"),
    }
    // rename 生效（同挂载点内，跨目录允许）
    fs::rename(mp.join("new.bin"), mp.join("renamed.bin")).expect("rename 经写回日志应生效");
    assert!(!backing.join("new.bin").exists() && backing.join("renamed.bin").exists());
    // 写回日志目录在挂载面不可见 + 不可写回（落锤 Q2）
    let listing: Vec<String> = fs::read_dir(&mp)
        .expect("readdir")
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !listing
            .iter()
            .any(|n| n.starts_with(".partisync-writeback")),
        "readdir 不应暴露写回日志目录"
    );
    expect_errno(
        fs::rename(mp.join(".partisync-writeback"), mp.join("stolen")),
        EACCES,
        "写回日志目录 rename 应 EACCES（探针自纠错判例：unlink 对目录被 VFS 前置拦，须用可达 handler）",
    );
}
