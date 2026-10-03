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
const EROFS: i32 = 30;
const EBUSY: i32 = 16;
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

    // ── ③ 仍拒绝面逐项 errno（P15 延续；写回面开放项见 ⑦⑧）──
    // T02 起 unlink/rmdir/rename 走写回日志路径；T03 起已存在文件写打开
    // 进 overlay（见 ⑧）——旧 EACCES 断言移交 ⑧。
    expect_errno(fs::create_dir(mp.join("d")), EPERM, "mkdir");
    expect_errno(
        std::os::unix::fs::symlink("/etc/hostname", mp.join("l")),
        EPERM,
        "symlink",
    );
    // perm 形式 setattr 仍拒绝（truncate 形式开放见 ⑧）
    use std::os::unix::fs::PermissionsExt;
    let mut perm = fs::symlink_metadata(mp.join("a.txt"))
        .expect("md")
        .permissions();
    perm.set_mode(0o600);
    expect_errno(
        fs::set_permissions(mp.join("a.txt"), perm),
        EPERM,
        "chmod（perm setattr）",
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
    // release 后写打开：T03 起允许（新 overlay 会话），语义见 ⑧。

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

    // ── ⑧ overlay 整文件替换（M8-WP07-T03；SPEC §2.1「写打开已存在文件」）──
    // a. 写打开已存在文件（T02 时 EACCES → 现进入 overlay 会话）：预填
    //    原内容，跳写到 cur_len 之外 EINVAL；release 整文件替换。
    // b. Q1 并发：持有期间第二写打开 → EBUSY。
    // c. truncate(size) 特例：holder 持有期在 staging 截断；独立调用走
    //    原子 Replace。
    // d. crash 相位：release 前 backing 保持旧内容（无半提交）。
    let old = fs::read(backing.join("renamed.bin")).expect("read old");
    let mut w1 = fs::OpenOptions::new()
        .write(true)
        .open(mp.join("renamed.bin"))
        .expect("写打开已存在文件应进入 overlay（T03 开放）");
    // 预填游标：追加位置 = 原长度（顺序写契约不变）
    w1.seek(SeekFrom::Start(old.len() as u64)).unwrap();
    w1.write_all(b"-appended").unwrap();
    // release 前 backing 必须仍是旧内容（无半提交）
    assert_eq!(
        fs::read(backing.join("renamed.bin")).unwrap(),
        old,
        "release 前 backing 应保持旧内容（overlay 无半提交）"
    );
    // b. 并发第二写打开 → EBUSY（w1 未 drop = 未 release）
    expect_errno(
        fs::OpenOptions::new()
            .write(true)
            .open(mp.join("renamed.bin")),
        EBUSY,
        "同路径暂存持有期间第二写打开（Q1：后写者 EBUSY）",
    );
    drop(w1); // release → 整文件替换生效
    let mut expect_new = old.clone();
    expect_new.extend_from_slice(b"-appended");
    assert_eq!(
        fs::read(backing.join("renamed.bin")).unwrap(),
        expect_new,
        "release 后 backing 应为 overlay 新内容（整文件替换）"
    );
    // 释放后重开 → EBUSY 消失（可再次进入 overlay）
    let mut w2 = fs::OpenOptions::new()
        .write(true)
        .open(mp.join("renamed.bin"))
        .expect("release 后重开应允许");
    w2.seek(SeekFrom::Start(0)).unwrap();
    // c. truncate 特例：O_TRUNC 打开 = open 后 setattr(size=0)——清空写
    drop(w2);
    let mut w3 = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(mp.join("renamed.bin"))
        .expect("O_TRUNC 写打开（open+setattr(0) 序列）");
    w3.write_all(b"fresh").unwrap();
    drop(w3);
    assert_eq!(
        fs::read(backing.join("renamed.bin")).unwrap(),
        b"fresh",
        "O_TRUNC 后写入 = 清空写整文件替换"
    );
    // c2. 独立 truncate(size)（无 holder）：原子 Replace 到 size
    fs::File::options()
        .write(true)
        .open(mp.join("renamed.bin"))
        .and_then(|f| f.set_len(2))
        .expect("独立 truncate(2)");
    assert_eq!(
        fs::read(backing.join("renamed.bin")).unwrap(),
        b"fr",
        "truncate(2) = 原内容截断（整文件替换特例）"
    );
    // staging 无残留（apply 完成即清）
    let staging = backing.join(".partisync-writeback").join("staging");
    if let Ok(rd) = fs::read_dir(&staging) {
        assert!(rd.count() == 0, "staging 不应有残留 blob");
    }
}

/// ⑨ by-hash 只读命名空间（M8-WP07-T04；SPEC §2.2）——环境门控同主探针。
#[test]
fn probe_by_hash_namespace() {
    use partisync_cas::content_hash;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let dir = std::env::temp_dir().join(format!("partifuse-bh-{}", std::process::id()));
    let backing = dir.join("backing");
    let cas = dir.join("cas");
    let mp = dir.join("mnt");
    fs::create_dir_all(&backing).expect("backing dir");
    fs::create_dir_all(&mp).expect("mountpoint dir");
    let store = rt
        .block_on(partisync_cas::ChunkStore::open(&cas))
        .expect("CAS open");
    let session = match partisync_fuse::spawn_mount(
        PartiFuse::with_cas(backing.clone(), Some(std::sync::Arc::new(store))),
        &mp,
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[SKIP] 环境不可挂载（{e}）——探针跳过，报告按「未执行」登记");
            return;
        }
    };
    let mut ready = false;
    for _ in 0..50 {
        if fs::read_dir(&mp).is_ok() {
            ready = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if !ready {
        eprintln!("[SKIP] 挂载后 5s 未就绪");
        return;
    }
    let _ = session;

    // ① 内容入 CAS + digest 命中读（read 全量 + 偏移）
    let payload = b"by-hash payload 123";
    let digest = content_hash(payload);
    rt.block_on(async {
        partisync_cas::ChunkStore::open(&cas)
            .await
            .expect("reopen cas")
            .put(payload)
            .await
            .expect("put");
    });
    let hp = mp.join("by-hash").join("blake3").join(&digest);
    assert_eq!(
        fs::read(&hp).expect("digest 命中读"),
        payload,
        "by-hash read 应直连 CAS"
    );
    let mut f = fs::File::open(&hp).expect("open by-hash 只读");
    f.seek(SeekFrom::Start(8)).unwrap();
    let mut buf = [0u8; 5];
    f.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"paylo", "by-hash 偏移读（payload[8..13]）");

    // ② 目录视图并存：by-hash 目录与真实文件同挂载
    fs::write(backing.join("real.txt"), b"real").expect("seed");
    let listing: Vec<String> = fs::read_dir(&mp)
        .expect("readdir")
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        listing.contains(&"real.txt".to_string()) && listing.contains(&"by-hash".to_string()),
        "目录视图与 by-hash 应并存"
    );
    // blake3 层 readdir 受限（R6：不枚举 digest）
    let algo_listing: Vec<String> = fs::read_dir(mp.join("by-hash").join("blake3"))
        .expect("readdir blake3")
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        algo_listing.is_empty(),
        "blake3 层 readdir 应受限（lookup-only 导航）"
    );

    // ③ digest 不存在 → ENOENT
    let bogus = "0".repeat(64);
    assert!(
        fs::File::open(mp.join("by-hash").join("blake3").join(&bogus)).is_err(),
        "不存在的 digest 应 ENOENT"
    );

    // ④ 写类操作一律 EROFS
    expect_errno(
        fs::OpenOptions::new().write(true).open(&hp),
        EROFS,
        "by-hash 写打开",
    );
    expect_errno(fs::remove_file(&hp), EROFS, "by-hash unlink");
    expect_errno(fs::rename(&hp, mp.join("stolen2")), EROFS, "by-hash rename");
    expect_errno(
        fs::File::options()
            .write(true)
            .open(&hp)
            .map(|f| f.set_len(0))
            .map(|_| ()),
        EROFS,
        "by-hash truncate",
    );

    // ⑤ 拒绝后完整性：CAS 内容原样
    assert_eq!(fs::read(&hp).expect("read"), payload, "拒绝后 CAS 内容原样");
}
