//! M11-WP02 T02 探针：索引只读打开路径（P24，SPEC M11-WP02 §2.1/§3.1）。
//!
//! P24 登记行 docs/tests/properties.md（登记先于测试代码，commit cc23fb0）：
//! (a) 只读打开不获取写锁；(b) 与活跃 IndexWriter 并存；(c) N 只读实例并发。

use partisync_index::{
    Bm25Index, Bm25Query, Bm25Result, IndexEngine, IndexEngineConfig, IndexedDoc,
};

fn doc(id: &str, marker: &str) -> IndexedDoc {
    IndexedDoc {
        content_id: id.to_string(),
        filename: format!("{id}.txt"),
        tags: vec!["m11wp02".into()],
        ocr_text: Some(format!("{marker} probe content {id}")),
        transcript_text: None,
        updated_ns: 1,
    }
}

fn hits(idx: &Bm25Index, marker: &str) -> Bm25Result {
    idx.search(Bm25Query {
        query: marker.to_string(),
        limit: 10,
        include_transcript: false,
    })
    .expect("search should not fail")
}

/// P24-a：只读打开前后 index 目录无写锁文件创建，且 is_read_only()。
#[test]
fn p24_a_readonly_open_never_takes_writer_lock() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bm25");

    // 先用写者建索引 + 写入一条（构造既存索引）
    let idx = Bm25Index::open_or_create(&path).unwrap();
    idx.upsert(doc("c-1", "quantum")).unwrap();
    idx.commit().unwrap();
    drop(idx);
    // 写者 drop 后锁文件由 writer 线程池异步清理——轮询等待（≤2s）；
    // 超时仍存在则记为陈留（tantivy index.rs:541-544 容许），主断言
    // 降级为「只读打开不改变锁文件存在性」。
    let mut pre_clean = false;
    for _ in 0..40 {
        if !path.join(".tantivy-writer.lock").exists() {
            pre_clean = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let pre = path.join(".tantivy-writer.lock").exists();

    // 只读打开：不创建写锁文件（P24-a 主断言）
    let ro = Bm25Index::open_read_only(&path).unwrap();
    let post = path.join(".tantivy-writer.lock").exists();
    assert_eq!(
        pre, post,
        "P24-a: open_read_only must not create .tantivy-writer.lock"
    );
    if pre_clean {
        assert!(!post, "strong form: no lock before, none after");
    }
    assert!(ro.is_read_only());
    let r = hits(&ro, "quantum");
    assert_eq!(r.total, 1);
    assert_eq!(r.hits[0].content_id, "c-1");
}

/// P24-b：只读实例与活跃 IndexWriter 并存（D5 回归直证）——打开不失败、
/// 写者新 commit 经 force_reload 可见；写方法在只读实例返回结构化错误。
#[test]
fn p24_b_readonly_coexists_with_active_writer() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bm25");

    // 活跃写者（进程内持锁，flock 对同进程第二 fd 同样互斥——与跨进程
    // 语义一致，见 tantivy directory_lock.rs INDEX_WRITER_LOCK）
    let writer_idx = Bm25Index::open_or_create(&path).unwrap();
    writer_idx.upsert(doc("c-1", "alpha")).unwrap();
    writer_idx.commit().unwrap();

    // 写者持锁在场时只读打开：不失败、不触碰写锁（D5 场景直证）
    let ro = Bm25Index::open_read_only(&path).unwrap();
    assert!(ro.is_read_only());
    let r = hits(&ro, "alpha");
    assert_eq!(r.total, 1);

    // 写者新 commit → 只读实例 reload 可见
    writer_idx.upsert(doc("c-2", "beta")).unwrap();
    writer_idx.commit().unwrap();
    ro.force_reload().unwrap();
    let r = hits(&ro, "beta");
    assert_eq!(r.total, 1);
    assert_eq!(r.hits[0].content_id, "c-2");

    // 只读实例写方法 = 结构化错误
    assert!(ro.upsert(doc("c-x", "x")).is_err());
    assert!(ro.commit().is_err());
    assert!(ro.clear().is_err());
}

/// P24-c：≥3 只读实例并发 search 无互斥、无 LockBusy。
#[test]
fn p24_c_three_readonly_instances_concurrent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bm25");
    let writer_idx = Bm25Index::open_or_create(&path).unwrap();
    for i in 0..8 {
        writer_idx.upsert(doc(&format!("c-{i}"), "zenith")).unwrap();
    }
    writer_idx.commit().unwrap();

    let ros: Vec<std::sync::Arc<Bm25Index>> = (0..3)
        .map(|_| std::sync::Arc::new(Bm25Index::open_read_only(&path).unwrap()))
        .collect();
    let handles: Vec<_> = ros
        .into_iter()
        .enumerate()
        .map(|(t, ro)| {
            std::thread::spawn(move || {
                for _ in 0..50 {
                    let r = hits(&ro, "zenith");
                    assert_eq!(r.total, 8, "thread {t}");
                }
            })
        })
        .collect();
    for h in handles {
        h.join().expect("concurrent reader thread");
    }
}

/// engine 级只读打开冒烟（open_read_only + bm25 查询 + 缺目录结构化错误）。
#[tokio::test]
async fn engine_open_read_only_smoke_and_missing_dir_error() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("index");
    std::fs::create_dir_all(root.join("bm25")).unwrap();

    let cfg = IndexEngineConfig {
        index_root: root.clone(),
        ..IndexEngineConfig::default()
    };
    // 写者建库（engine 既有面）
    let engine = IndexEngine::open_or_create(cfg.clone()).unwrap();
    engine.bm25_index().upsert(doc("c-e", "engine")).unwrap();
    engine.commit().unwrap();
    drop(engine);

    // 只读引擎：查询可用
    let ro = IndexEngine::open_read_only(cfg.clone()).unwrap();
    let r = ro
        .bm25_only(Bm25Query {
            query: "engine".into(),
            limit: 10,
            include_transcript: false,
        })
        .await
        .expect("bm25_only ok");
    assert!(r.total >= 1);

    // 缺目录 → 结构化错误（区别于 open_or_create 的建目录语义）
    let miss = IndexEngineConfig {
        index_root: dir.path().join("nonexistent"),
        ..IndexEngineConfig::default()
    };
    assert!(IndexEngine::open_read_only(miss).is_err());
}
