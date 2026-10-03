//! 索引接线 e2e（M8-WP07-T05；SPEC §3「索引接线 e2e」/ R1 处置）：
//! 挂载面写 → 写回日志应用 → EventRecord → GraphApplier（Store 后端）
//! → graph 行；**同根校验** = 同一操作序列走「CLI 直写」（graph::journal
//! 同款 add_entry 路径）的 Merkle 根必须与「挂载写」路径一致。
//!
//! 架构（SPEC §5 收敛：接线入口在 sync 侧——落锤 Q3）：fuse 写回应用
//! 成功后由装配层（gateway/`partifuse --graph`）构造 EventRecord 推给
//! EventApplier；本测试在 `partisync-fuse` 内直用
//! `partisync_sync::event::GraphApplier` 验证该链路（sync 为 path 依赖）。

use std::path::PathBuf;

use partisync_fuse::writeback::{WriteBackLog, WriteBackOp};
use partisync_graph::merkle::{entry_leaf, merkle_root, Leaf};
use partisync_graph::store::{EntryKind, Store};
use partisync_sync::event::{EventApplier, EventKind, EventRecord, GraphApplier};

/// 挂载写侧事件流：op 序列 → EventRecord（装配层同款构造）。
///
/// **size 台账**：Rename 的 Created 事件必须携带源条目 size（replace
/// 时登记）——装配层从既有 graph 行/写回会话取，本测试用内存台账同构
/// 该职责（T05 判例：Rename 丢 size = 叶哈希漂移，e2e 抓出）。
fn ops_to_records(ops: &[WriteBackOp]) -> Vec<EventRecord> {
    fn rec(path: &str, kind: EventKind, size: u64, seq: usize) -> EventRecord {
        EventRecord {
            provider: "partifuse".into(),
            space: "default".into(),
            // graph 口径：path 带前导 '/'（CLI/journal 同款；无前导路径
            // 会造成叶 key 漂移——Merkle 不同根，e2e 抓出）
            path: format!("/{}", path.trim_start_matches('/')),
            kind,
            cursor: format!("pf-{seq}"),
            payload: serde_json::json!({ "size": size, "mtime_ns": 1_700_000_000_000_000_000u64 }),
        }
    }
    let mut sizes: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut out = Vec::new();
    for (seq, op) in ops.iter().enumerate() {
        match op {
            WriteBackOp::Unlink { path } => {
                sizes.remove(path.as_str());
                out.push(rec(path, EventKind::Removed, 0, seq));
            }
            WriteBackOp::Rmdir { path } => {
                out.push(rec(path, EventKind::Removed, 0, seq));
            }
            WriteBackOp::Rename { from, to } => {
                let size = sizes.remove(from.as_str()).unwrap_or(0);
                sizes.insert(to.clone(), size);
                out.push(rec(from, EventKind::Removed, 0, seq));
                out.push(rec(to, EventKind::Created, size, seq));
            }
            WriteBackOp::Replace { path, .. } => {
                sizes.insert(path.clone(), 42);
                out.push(rec(path, EventKind::Modified, 42, seq));
            }
        }
    }
    out
}

/// 从 graph store 计算 Merkle 根（entry 状态叶，CLI 直写同款口径）。
async fn store_root(store: &Store) -> String {
    let rows = store.entry_state_leaves().await.expect("leaves");
    let leaves: Vec<Leaf> = rows
        .iter()
        .map(|(path, kind, content, owner, size, mtime)| {
            entry_leaf(
                path,
                *kind,
                content.as_deref(),
                owner.as_deref(),
                u64::try_from(*size).unwrap_or(0),
                u64::try_from(*mtime).unwrap_or(0),
            )
        })
        .collect();
    merkle_root(&leaves)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn t05_mount_write_and_cli_write_share_merkle_root() {
    let tmp = tempfile::tempdir().expect("tmp");

    // ── 路径 A：挂载写 → 写回日志 → 应用 → 事件流 → GraphApplier ──
    let db_a: PathBuf = tmp.path().join("a.db");
    let backing_a: PathBuf = tmp.path().join("a-backing");
    std::fs::create_dir_all(&backing_a).expect("backing");
    let store_a = Store::open(&db_a).await.expect("store A");
    store_a
        .seed_device_volume("device-self", "本机", "vol-a")
        .await
        .expect("seed");

    let log = WriteBackLog::open(&backing_a).expect("wal");
    let ops = vec![
        WriteBackOp::Replace {
            path: "docs/report.docx".into(),
            staging: "s1.staged".into(),
        },
        WriteBackOp::Unlink {
            path: "docs/old.txt".into(),
        },
        WriteBackOp::Rename {
            from: "docs/report.docx".into(),
            to: "docs/final/report.docx".into(),
        },
    ];
    // staging blob + 日志（挂载写产物）
    std::fs::create_dir_all(backing_a.join("docs")).expect("dir");
    std::fs::write(log.staging_path("s1.staged"), b"v2-content").expect("stage");
    for op in &ops {
        log.append(op).expect("append");
    }
    log.recover().expect("apply");

    // 接线：应用成功 → 事件流 → GraphApplier（Store A）
    let applier_a = GraphApplier::new(store_a.clone());
    for record in ops_to_records(&ops) {
        applier_a.apply_event(&record).await.expect("apply event");
    }

    // ── 路径 B：CLI 直写（同终态：docs/final/report.docx size=42）──
    let db_b: PathBuf = tmp.path().join("b.db");
    let store_b = Store::open(&db_b).await.expect("store B");
    store_b
        .seed_device_volume("device-self", "本机", "vol-b")
        .await
        .expect("seed");
    let parent = store_b
        .add_entry(None, "docs", "/docs", EntryKind::Dir, 0, 0, None, None)
        .await
        .expect("mkdir docs");
    let parent2 = store_b
        .add_entry(
            Some(&parent),
            "final",
            "/docs/final",
            EntryKind::Dir,
            0,
            0,
            None,
            None,
        )
        .await
        .expect("mkdir final");
    store_b
        .add_entry(
            Some(&parent2),
            "report.docx",
            "/docs/final/report.docx",
            EntryKind::File,
            42,
            1_700_000_000_000_000_000,
            None,
            None,
        )
        .await
        .expect("add file");

    // ── 同根校验 ──
    let root_a = store_root(&store_a).await;
    let root_b = store_root(&store_b).await;

    assert_eq!(
        root_a, root_b,
        "挂载写与 CLI 直写的 Merkle 根必须一致（R1 同根校验）"
    );

    // 挂载写产物在 graph 可见（find 面：path 精确查询——children 需
    // 根行，两侧 ensure_dir_chain 口径均为 parent_id=None，不适用）
    let row = store_a
        .entry_by_path("/docs/final/report.docx")
        .await
        .expect("entry_by_path");
    assert!(row.is_some(), "挂载写产物应在 graph 可见（索引接线生效）");
}
