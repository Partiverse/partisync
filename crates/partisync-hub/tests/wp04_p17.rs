//! M8-WP04-T03b 验收：P17 Hub 线性一致读（读单调）——注册行实现列
//! 回填（docs/tests/properties.md，SPEC M8-WP04 §2.3/§3）。
//!
//! 两个互补面（与注册行测试形态对应）：
//! 1. **proptest ≥1000 例（模型对照）**：随机写/读交错序列跑直连
//!    `Hub`——串行语义下读必须与模型**逐字一致**（单调性的上界强化），
//!    覆盖读时修复与投影路径的观察一致性；
//! 2. **raft 门面并发单调探针**：真双线程（写者经 raft 顺序推进版本、
//!    读者 `ensure_linearizable` 后观察）——已观察值不得回退
//!    （P17 本体：读单调）。

use partisync_hub::service::HubService;
use partisync_hub::{EntryRow, Hub, KIND_DIR, KIND_FILE};
use proptest::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn p17_root(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("wp04-p17-{tag}-{}", partisync_core::Ulid::now()))
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

// ---------------------------------------------------------------------
// 面 1：proptest 模型对照（≥1000 例）
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Op {
    /// 写：seq 号文件推进到 size=version。
    Write(u16, u64),
    /// 读：观察 seq 号文件。
    Read(u16),
}

fn op_strategy() -> impl proptest::strategy::Strategy<Value = Vec<Op>> {
    proptest::collection::vec(
        proptest::prop_oneof![
            (0u16..4, 1u64..64).prop_map(|(s, v)| Op::Write(s, v)),
            (0u16..4).prop_map(Op::Read),
        ],
        1..24,
    )
}

/// 每例一个 id 命名空间（entry_id[1..3] = case 序号）——单一 Hub 复用，
/// 1000 例避免逐例建库（356s → 秒级）。
fn case_ids(case: u16, seq: u16) -> [u8; 16] {
    let mut id = [0u8; 16];
    id[0] = 7;
    id[1..3].copy_from_slice(&case.to_be_bytes());
    id[3] = 0xBB; // file 标记（dir 用 0xAA，防 id 碰撞）
    id[4..6].copy_from_slice(&seq.to_be_bytes());
    id
}

/// 共享 Hub（OnceLock 单次建库；proptest 体串行复用）。
fn p17_hub(root: &std::path::Path) -> &'static Hub {
    static HUB: std::sync::OnceLock<Hub> = std::sync::OnceLock::new();
    HUB.get_or_init(|| Hub::open(root).expect("open shared model hub"))
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(1000))]

    #[test]
    fn p17_read_monotonicity_model(ops in op_strategy()) {
        static CASE: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);
        let case = CASE.fetch_add(1, Ordering::Relaxed);
        let hub = p17_hub(&p17_root("model"));
        let dir = {
            let mut e = [0u8; 16];
            e[0] = 7;
            e[1..3].copy_from_slice(&case.to_be_bytes());
            e[3] = 0xAA; // dir 标记
            EntryRow {
                entry_id: e,
                parent_id: None,
                kind: KIND_DIR,
                name: format!("dir-{case}"),
                content_id: None,
                size: 0,
                mtime_ns: 0,
                flags: 0,
            }
        };
        hub.put_entry(&dir).expect("put dir");
        // 模型：seq → 已应用版本（写只增不减）
        let mut model: std::collections::BTreeMap<u16, u64> = std::collections::BTreeMap::new();

        for op in &ops {
            match op {
                Op::Write(seq, version) => {
                    // 版本只增（策略域内允许重复版本 = 幂等重放形态）
                    let next = (*version).max(*model.get(seq).unwrap_or(&0));
                    let row = EntryRow {
                        entry_id: case_ids(case, *seq),
                        parent_id: Some(dir.entry_id),
                        kind: KIND_FILE,
                        name: format!("f{seq}"),
                        content_id: None,
                        size: next,
                        mtime_ns: 0,
                        flags: 0,
                    };
                    hub.put_entry(&row).expect("put");
                    model.insert(*seq, next);
                }
                Op::Read(seq) => {
                    let row = hub.get_entry(&case_ids(case, *seq)).expect("get");
                    match (&model.get(seq), row) {
                        (None, None) => {}
                        (Some(&v), Some(got)) => {
                            prop_assert_eq!(got.size, v, "read must equal model (no stale, no skip)");
                        }
                        (m, g) => panic!("model/read diverged: model={m:?} read={g:?}"),
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------
// 面 2：raft 门面并发单调探针（真双线程 + ensure_linearizable）
// ---------------------------------------------------------------------

#[test]
fn p17_raft_read_monotonicity_concurrent_probe() {
    let svc = Arc::new(HubService::open(&p17_root("raft-probe")).expect("open"));
    let dir = dir_row(8, "dir");
    svc.put_entry(&dir).expect("put dir");
    let file = file_row(&dir, "hot.bin", 1, 0);
    svc.put_entry(&file).expect("put initial");

    let done = Arc::new(AtomicBool::new(false));
    let versions = 150u64;

    // 写者：经 raft 顺序推进版本（写序 = 提交序）
    {
        let svc = Arc::clone(&svc);
        let done = Arc::clone(&done);
        std::thread::spawn(move || {
            for v in 1..=versions {
                let row = file_row(&dir_row(8, "dir"), "hot.bin", 1, v);
                svc.put_entry(&row).expect("writer put");
            }
            done.store(true, Ordering::Release);
        });
    }

    // 读者：ensure_linearizable 后观察——已见值不得回退（P17 本体）
    let mut observed = 0u64;
    let mut observed_some = false;
    loop {
        if done.load(Ordering::Acquire) {
            let final_row = svc
                .get_entry(&file.entry_id)
                .expect("final get")
                .expect("row");
            assert!(
                final_row.size >= observed,
                "final read regressed: seen {observed}, got {}",
                final_row.size
            );
            break;
        }
        let row = svc.get_entry(&file.entry_id).expect("reader get");
        if let Some(row) = row {
            assert!(
                !observed_some || row.size >= observed,
                "P17 violated: observed {} then {}",
                observed,
                row.size
            );
            observed = row.size;
            observed_some = true;
        }
        std::thread::sleep(std::time::Duration::from_micros(200));
    }
    assert_eq!(observed, versions, "reader must converge to final version");
}
