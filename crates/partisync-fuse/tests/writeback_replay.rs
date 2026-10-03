//! P16 写回一致性 proptest（SPEC M8-WP07 §3）：随机操作序列 → append 全部
//! 未应用（模拟「日志已落盘、应用前 crash」相位）→ `recover()` 重放两次
//! → 快照逐字节一致且与模型终态一致（重放幂等，backing 无半提交）。

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use partisync_fuse::writeback::{WriteBackLog, WriteBackOp};
use proptest::prelude::*;

/// 路径快照：rel 字符串 → 内容（文件）或 "dir"（目录）。仅 backing 根下
/// 业务条目（跳过 .partisync-writeback）。
fn snapshot(root: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
    for e in fs::read_dir(dir).expect("read_dir") {
        let e = e.expect("entry");
        let p = e.path();
        let rel = p
            .strip_prefix(root)
            .expect("strip")
            .to_string_lossy()
            .replace('\\', "/");
        if rel.starts_with(".partisync-writeback") {
            continue;
        }
        if p.is_dir() {
            out.insert(rel.clone(), "dir".into());
            walk(root, &p, out);
        } else {
            out.insert(rel, fs::read(&p).expect("read").len().to_string());
        }
    }
}

/// 模型动作（解释期按当前模型态过滤非法动作）。
#[derive(Debug, Clone)]
enum Action {
    Unlink(u8),
    Rmdir(u8),
    Rename(u8, u8),
}

fn actions() -> impl Strategy<Value = Vec<Action>> {
    proptest::collection::vec(
        prop_oneof![
            (0u8..6).prop_map(Action::Unlink),
            (0u8..4).prop_map(Action::Rmdir),
            (0u8..6, 0u8..6).prop_map(|(a, b)| Action::Rename(a, b)),
        ],
        0..24,
    )
}

/// 模型态：files[i] 存在性 + 名称。目录 d0..d3 固定存在（部分为空）。
/// 初始态 = 4 个文件 + 4 个目录（其中 d1、d3 为空）。
fn initial_state() -> (BTreeMap<String, String>, Vec<Option<String>>) {
    let mut model: BTreeMap<String, String> = BTreeMap::new();
    for (i, d) in ["d0", "d1", "d2", "d3"].iter().enumerate() {
        model.insert(d.to_string(), "dir".into());
        if i % 2 == 0 {
            model.insert(format!("{d}/x.txt"), "4".into()); // 非空目录
        }
    }
    let files: Vec<Option<String>> = (0..6).map(|i| Some(format!("f{i}.txt"))).collect();
    for f in files.iter().flatten() {
        model.insert(f.clone(), "7".into());
    }
    (model, files)
}

/// 解释动作 → 日志操作（非法动作返回 None 并原样跳过；模型同步更新）。
fn interpret(
    model: &mut BTreeMap<String, String>,
    files: &mut [Option<String>],
    a: &Action,
) -> Option<WriteBackOp> {
    match a {
        Action::Unlink(i) => {
            let name = files.get_mut(*i as usize)?.take()?;
            model.remove(&name);
            Some(WriteBackOp::Unlink { path: name })
        }
        Action::Rmdir(i) => {
            let d = format!("d{i}");
            // 仅空目录可删（与 rmdir 预处理同口径）
            if model
                .iter()
                .any(|(k, _)| k != &d && k.starts_with(&format!("{d}/")))
            {
                return None;
            }
            if model.remove(&d)?.as_str() != "dir" {
                return None;
            }
            Some(WriteBackOp::Rmdir { path: d })
        }
        Action::Rename(i, j) => {
            if i == j {
                return None;
            }
            let from = files.get(*i as usize).cloned().flatten()?;
            let to = format!("f{j}.txt");
            if files.get(*j as usize)?.is_some() {
                return None; // 目标槽位被占 → 跳过（rename 不覆盖文件）
            }
            *files.get_mut(*i as usize)? = None;
            *files.get_mut(*j as usize)? = Some(to.clone());
            model.remove(&from);
            model.insert(to.clone(), "7".into());
            Some(WriteBackOp::Rename { from, to })
        }
    }
}

fn seed_backing(root: &Path, model: &BTreeMap<String, String>) {
    fs::create_dir_all(root).expect("backing");
    for (k, v) in model {
        let p = root.join(k);
        if v == "dir" {
            fs::create_dir_all(&p).expect("mkdir");
        } else {
            fs::create_dir_all(p.parent().expect("parent")).expect("parent");
            fs::write(&p, b"1234567").expect("seed file");
        }
    }
}

fn assert_model_holds(root: &Path, model: &BTreeMap<String, String>) {
    let snap = snapshot(root);
    let expect: BTreeMap<String, String> = model
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                if v == "dir" { "dir".into() } else { "7".into() },
            )
        })
        .collect();
    assert_eq!(
        snap, expect,
        "backing 终态必须与模型一致（重放幂等 + 无半提交）"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn p16_replay_is_idempotent_and_converges(actions in actions()) {
        let tmp = tempfile::tempdir().expect("tmp");
        let backing = tmp.path().join("backing");
        let (mut model, mut files) = initial_state();
        seed_backing(&backing, &model);

        // 日志已落盘、应用前 crash 相位：append 全部，不 apply
        let log = WriteBackLog::open(&backing).expect("open");
        for a in &actions {
            if let Some(op) = interpret(&mut model, &mut files, a) {
                log.append(&op).expect("append");
            }
        }
        drop(log);

        // crash 后重放两次：结果必须一致且等于模型终态
        let log = WriteBackLog::open(&backing).expect("reopen");
        log.recover().expect("replay 1");
        assert_model_holds(&backing, &model);
        let log = WriteBackLog::open(&backing).expect("reopen 2");
        log.recover().expect("replay 2");
        assert_model_holds(&backing, &model);
        // 日志压实为空（全量应用后无残留；空序列时 WAL 文件未创建）
        let wal = backing.join(".partisync-writeback").join("wal.jsonl");
        let wal_len = wal.metadata().map(|m| m.len()).unwrap_or(0);
        assert!(wal_len == 0, "全量应用后 WAL 应压实为空（得到 {wal_len} 字节）");
    }
}
