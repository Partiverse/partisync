//! 检索索引重建（M9-WP03-T06，SPEC M9-WP03 v0.3 §2.6）。
//!
//! 背景：`IndexWriter::upsert_content` / `rebuild_index` 此前零生产调用方
//! ——桌面/CLI 从未有任何命令填充 tantivy 索引，产品搜索从未工作过
//! （demo 查询走 Bm25Index 直调 example 绕开）。本模块补上组合层写入方：
//! graph DB（entry×content）→ 文本筛选 → CAS 原文 → BM25 全文索引。
//!
//! 文本判定（方案 2，批准拍板）：mime `text/*` 或结构化文本 mime
//! （json/xml/yaml/toml）；mime 缺失时按扩展名白名单嗅探。二进制/超限
//! （256 KiB 截断）/非 UTF-8 一律跳过或截断，不静默索引垃圾。

use std::path::{Path, PathBuf};

use partisync_cas::ChunkStore;
use partisync_core::error::PartisyError;
use partisync_graph::Store;
use partisync_index::search::bm25::IndexedDoc;
use partisync_index::IndexEngine;

/// 单文档索引文本上限（字节；截断到 char 边界——防超大文本拖垮索引段）。
const MAX_DOC_BYTES: usize = 256 * 1024;

/// 无 mime 时按扩展名嗅探的白名单。
const TEXT_EXTS: &[&str] = &[
    "md", "markdown", "txt", "json", "yaml", "yml", "toml", "csv", "rs", "py", "js", "ts", "html",
    "htm", "xml", "sh",
];

/// 重建统计（打印用；skip_binary = 非文本判定，skip_invalid = 截断后仍非 UTF-8）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReindexStats {
    pub indexed: u64,
    pub skip_binary: u64,
    pub skip_invalid: u64,
    pub read_errors: u64,
}

fn ext_indexable(name: &str) -> bool {
    name.rsplit('.').next().is_some_and(|ext| {
        name.contains('.') && TEXT_EXTS.contains(&ext.to_ascii_lowercase().as_str())
    })
}

/// 文本判定：mime `text/*` / 结构化文本 mime；mime 缺失或空 → 扩展名嗅探。
fn is_indexable(mime: Option<&str>, name: &str) -> bool {
    match mime.map(str::trim) {
        Some(m) if !m.is_empty() => {
            m.starts_with("text/")
                || matches!(
                    m,
                    "application/json"
                        | "application/xml"
                        | "application/yaml"
                        | "application/x-yaml"
                        | "application/toml"
                )
        }
        _ => ext_indexable(name),
    }
}

/// 截断到 ≤ max 字节且落在 char 边界。
fn char_safe_truncate(bytes: &[u8], max: usize) -> &[u8] {
    let mut end = bytes.len().min(max);
    while end > 0 && std::str::from_utf8(&bytes[end - 1..end]).is_err() {
        end -= 1;
    }
    &bytes[..end]
}

/// 解析 entry 虚拟路径 → 磁盘候选。vpath **恒为索引根相对**（indexer
/// 写入形态：前导 `/` 是虚拟层级，不是磁盘绝对路径——实测踩坑）：
/// 去 `/` 后 join `source_root`；无根 → None（CAS 回退）。
fn resolve_source(source_root: Option<&Path>, vpath: &str) -> Option<PathBuf> {
    let rel = vpath.strip_prefix('/').unwrap_or(vpath);
    if rel.is_empty() {
        return None;
    }
    source_root.map(|root| root.join(rel))
}

/// 重建核心：全量扫描 graph 内容 → 文本筛选 → 读原文 → BM25 upsert + commit。
///
/// 数据源优先级：源文件（entry.path 经 [`resolve_source`]；indexer 对小
/// 文件不写 CAS——content_id 仅是哈希身份，字节只在源盘）→ 回退按文件
/// 形态分流：大文件（chunk_root 非空）走 v18 `content_chunk` 清单重组
/// （`Store::reassemble_content`，P22；M9-WP03 债已清偿）；小文件走 CAS
/// 单块（显式 put 面）。存量无清单（v18 落表前入库，**不可回填**）重组
/// 显式 Err → 维持 read_errors 计数（诚实边界，不静默截断）。
///
/// # Errors
/// DB / 索引写入错误 → Fatal 上抛（单文件读失败仅计数不中止）。
pub async fn reindex_core(
    store: &Store,
    cas: &ChunkStore,
    engine: &IndexEngine,
    source_root: Option<&Path>,
) -> Result<ReindexStats, PartisyError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        cid: String,
        name: String,
        path: String,
        mime: Option<String>,
        mtime: i64,
        chunk_root: Option<String>,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT e.content_id AS cid, MIN(e.name) AS name, MIN(e.path) AS path, \
                c.mime AS mime, MAX(e.mtime_ns) AS mtime, MIN(e.chunk_root) AS chunk_root \
         FROM entry e JOIN content c ON c.id = e.content_id \
         WHERE e.kind = 0 GROUP BY e.content_id",
    )
    .fetch_all(store.pool_ref())
    .await
    .map_err(|e| PartisyError {
        severity: partisync_core::error::Severity::Fatal,
        source: Some(format!("reindex 扫描: {e}").into()),
    })?;

    let mut stats = ReindexStats::default();
    let mut docs = Vec::new();
    for r in rows {
        if !is_indexable(r.mime.as_deref(), &r.name) {
            stats.skip_binary += 1;
            continue;
        }
        let bytes = match (
            r.chunk_root.as_deref(),
            resolve_source(source_root, &r.path).and_then(|p| std::fs::read(p).ok()),
        ) {
            // 源文件主路径（大小文件同优先级，与债前一致）
            (_, Some(b)) => b,
            // 大文件回退（chunk_root 非空）：重组读出（P22-b/c——缺块/清单
            // 缺失显式 Err，计 read_errors 不中止不静默截断）
            (Some(root), None) => match store.reassemble_content(cas, &r.cid, Some(root)).await {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("warn: 重组 {} ({}): {e}", r.cid, r.name);
                    stats.read_errors += 1;
                    continue;
                }
            },
            // 小文件回退：CAS 单块（显式 put 面）
            (None, None) => match cas.get(&r.cid).await {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("warn: 读 {} ({}): 源缺失 + CAS: {e}", r.cid, r.name);
                    stats.read_errors += 1;
                    continue;
                }
            },
        };
        let text = match std::str::from_utf8(char_safe_truncate(&bytes, MAX_DOC_BYTES)) {
            Ok(t) => t,
            Err(_) => {
                stats.skip_invalid += 1;
                continue;
            }
        };
        docs.push(IndexedDoc {
            content_id: r.cid,
            filename: r.name,
            tags: Vec::new(),
            ocr_text: Some(text.to_owned()),
            transcript_text: None,
            updated_ns: r.mtime,
        });
    }
    stats.indexed = docs.len() as u64;
    engine.bm25_index().upsert_batch(docs)?;
    engine.commit()?;
    Ok(stats)
}

/// `partisync reindex` 子命令入口。
///
/// # Errors
/// 见 [`reindex_core`]；参数缺省沿 search_cmd 口径
/// （db/cas = 工作目录，index = data_local/.partisync/index）；
/// --source-root 缺省取 jobs 表最近 index 作业 root。
pub async fn reindex_cmd(args: &[String]) -> i32 {
    let db = flag_of(args, "--db").unwrap_or_else(|| "./partisync.db".into());
    let cas_dir = flag_of(args, "--cas").unwrap_or_else(|| "./partisync.cas".into());
    let index_root: String = flag_of(args, "--index-root").unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".partisync")
            .join("index")
            .to_string_lossy()
            .into_owned()
    });
    let store = match partisync_graph::store::Store::open(std::path::Path::new(&db)).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: 打开 db: {e}");
            return 1;
        }
    };
    let cas = match ChunkStore::open(std::path::Path::new(&cas_dir)).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: 打开块库: {e}");
            return 1;
        }
    };
    let engine =
        match partisync_index::IndexEngine::open_or_create(partisync_index::IndexEngineConfig {
            index_root: std::path::PathBuf::from(&index_root),
            enable_reranker: false,
            reranker_model_dir: None,
        }) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("error: 打开索引: {e}");
                return 1;
            }
        };
    // 源根解析：--source-root 显式 > jobs 表最近 index 作业的 root > 无
    // （绝对路径 entry 不受影响；相对路径 entry 无根则计入 read_errors）。
    let source_root: Option<PathBuf> = match flag_of(args, "--source-root") {
        Some(s) => Some(PathBuf::from(s)),
        None => sqlx::query_scalar::<_, String>(
            "SELECT root FROM jobs WHERE kind = 'index' ORDER BY created_ns DESC LIMIT 1",
        )
        .fetch_optional(store.pool_ref())
        .await
        .map_err(|e| eprintln!("warn: jobs 根查询: {e}"))
        .ok()
        .flatten()
        .map(PathBuf::from),
    };
    if let Some(root) = &source_root {
        println!("源根: {}", root.display());
    }
    match reindex_core(&store, &cas, &engine, source_root.as_deref()).await {
        Ok(s) => {
            println!(
                "reindex 完成: indexed={} skip_binary={} skip_invalid={} read_errors={} (index: {})",
                s.indexed, s.skip_binary, s.skip_invalid, s.read_errors,
                index_root
            );
            0
        }
        Err(e) => {
            eprintln!("error: reindex: {e}");
            1
        }
    }
}

fn flag_of(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use partisync_core::Ulid;
    use partisync_graph::store::EntryKind;
    use partisync_index::search::bm25::Bm25Query;
    use partisync_index::IndexEngineConfig;

    #[tokio::test]
    async fn reindex_indexes_text_and_skips_binary() {
        let dir = std::env::temp_dir().join(format!("t06-reindex-{}", Ulid::now()));
        std::fs::create_dir_all(&dir).unwrap();

        let store = Store::open_in_memory().await.unwrap();
        store
            .seed_device_volume("dev-a", "Device A", "fp-a")
            .await
            .unwrap();
        let cas = ChunkStore::open(&dir.join("cas")).await.unwrap();

        // ① markdown 文本（源文件主路径）：真实盘上文件 + 入库 + content(mime=NULL，
        //    沿 indexer 真实形态——小文件不写 CAS，mime 不嗅探) → 扩展名嗅探命中
        let src_dir = dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let md = b"# argon2 migration notes\nweekly sync marker xyzzy";
        std::fs::write(src_dir.join("notes.md"), md).unwrap();
        let cid_md = partisync_cas::content_hash(md);
        sqlx::query("INSERT INTO content (id, size, mime) VALUES (?, ?, NULL)")
            .bind(&cid_md)
            .bind(md.len() as i64)
            .execute(store.pool_ref())
            .await
            .unwrap();
        store
            .add_entry(
                None,
                "notes.md",
                "src/notes.md",
                EntryKind::File,
                md.len() as u64,
                1_700_000_000_000_000_000,
                Some((cid_md.as_str(), md.len() as u64)),
                None,
            )
            .await
            .unwrap();

        // ② 二进制：mime NULL + .bin 扩展 → 必须跳过
        let bin = vec![0u8, 159, 146, 150];
        let cid_bin = cas.put(&bin).await.unwrap();
        sqlx::query("INSERT INTO content (id, size, mime) VALUES (?, ?, NULL)")
            .bind(&cid_bin)
            .bind(bin.len() as i64)
            .execute(store.pool_ref())
            .await
            .unwrap();
        store
            .add_entry(
                None,
                "blob.bin",
                "/blob.bin",
                EntryKind::File,
                bin.len() as u64,
                1_700_000_000_000_000_000,
                Some((cid_bin.as_str(), bin.len() as u64)),
                None,
            )
            .await
            .unwrap();

        // ③ 文本无源文件（源缺失）→ CAS 单块回退路径（显式 put 面）
        let fb = b"fallback marker quux only in cas";
        let cid_fb = cas.put(fb).await.unwrap();
        sqlx::query("INSERT INTO content (id, size, mime) VALUES (?, ?, NULL)")
            .bind(&cid_fb)
            .bind(fb.len() as i64)
            .execute(store.pool_ref())
            .await
            .unwrap();
        store
            .add_entry(
                None,
                "dropped.txt",
                "/missing/dropped.txt",
                EntryKind::File,
                fb.len() as u64,
                1_700_000_000_000_000_000,
                Some((cid_fb.as_str(), fb.len() as u64)),
                None,
            )
            .await
            .unwrap();

        let engine = IndexEngine::open_or_create(IndexEngineConfig {
            index_root: dir.join("index"),
            enable_reranker: false,
            reranker_model_dir: None,
        })
        .unwrap();

        let stats = reindex_core(&store, &cas, &engine, Some(dir.as_path()))
            .await
            .unwrap();
        engine.bm25_index().reload().unwrap(); // OnCommitWithDelay：同步语义须显式 reload（bm25.rs 判例）
        assert_eq!(
            stats.indexed, 2,
            "源文件文本（相对路径解析）+ CAS 回退文本都必须被索引（stats={stats:?}）"
        );
        assert_eq!(stats.skip_binary, 1, "二进制必须被跳过");
        assert_eq!(stats.skip_invalid, 0);
        assert_eq!(stats.read_errors, 0);

        // 全文可检索（方案 2：CAS 原文进 ocr_text 通道）
        let result = engine
            .bm25_only(Bm25Query {
                query: "xyzzy".into(),
                limit: 10,
                include_transcript: true,
            })
            .await
            .unwrap();
        assert_eq!(result.hits.len(), 1, "源文件原文标记词必须命中");
        assert_eq!(result.hits[0].content_id, cid_md);

        // CAS 回退路径文本可检索
        let result = engine
            .bm25_only(Bm25Query {
                query: "quux".into(),
                limit: 10,
                include_transcript: true,
            })
            .await
            .unwrap();
        assert_eq!(result.hits.len(), 1, "CAS 回退文本必须命中");
        assert_eq!(result.hits[0].content_id, cid_fb);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_indexable_covers_mime_and_ext_fallback() {
        assert!(is_indexable(Some("text/plain"), "a.txt"));
        assert!(is_indexable(Some("application/json"), "a.json"));
        assert!(!is_indexable(Some("image/png"), "a.png"));
        assert!(
            is_indexable(None, "notes.MD"),
            "mime 缺失 → 扩展名嗅探（大小写不敏感）"
        );
        assert!(!is_indexable(None, "no-ext"));
        assert!(is_indexable(Some(""), "readme.md"), "空 mime → 嗅探");
    }

    // ===== M10-WP04-T04（P22，SPEC M10-WP04 §2.2/§3）=====

    /// 确定性生成 ≥ target 字节的 UTF-8 文本（行号破周期性给 fastcdc 切点；
    /// 第 2 行嵌 marker 供 bm25 探针）。
    fn big_text(target: usize, marker: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(target + 512);
        let mut i: u64 = 0;
        while out.len() < target {
            let line = if i == 1 {
                format!("marker {marker} embedded for bm25 probe\n")
            } else {
                format!("partisync lorem line {i:08} argon2 migration weekly sync notes\n")
            };
            out.extend_from_slice(line.as_bytes());
            i += 1;
        }
        out
    }

    async fn t04_env(dir: &Path) -> (Store, ChunkStore, IndexEngine) {
        let store = Store::open_in_memory().await.unwrap();
        store
            .seed_device_volume("dev-a", "Device A", "fp-a")
            .await
            .unwrap();
        let cas = ChunkStore::open(&dir.join("cas")).await.unwrap();
        let engine = IndexEngine::open_or_create(IndexEngineConfig {
            index_root: dir.join("index"),
            enable_reranker: false,
            reranker_model_dir: None,
        })
        .unwrap();
        (store, cas, engine)
    }

    /// P22-a/b + reindex 重组路径：大文件索引后清单与 chunk_root 对账相等、
    /// 重组 roundtrip blake3==content_id；源盘移除后 reindex 经 content_chunk
    /// 重组读出 → marker 词命中、零 read_errors。
    #[tokio::test]
    async fn t04_big_file_manifest_roundtrip_and_reindex_reassembly() {
        let dir = std::env::temp_dir().join(format!("t04-roundtrip-{}", Ulid::now()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let (store, cas, engine) = t04_env(&dir).await;

        // 2.5 MiB > 256 KiB 阈值 ⇒ 分块入库（indexer.rs 落表站点真实路径）
        let big = big_text(2500 * 1024, "xyzzyquux");
        std::fs::write(dir.join("src/big-notes.md"), &big).unwrap();
        let report = partisync_graph::indexer::index_path(&store, Some(&cas), &dir.join("src"))
            .await
            .unwrap();
        assert_eq!(report.chunked_files, 1, "大文件必须走分块入库");

        let cid = partisync_cas::content_hash(&big);
        let hashes = store.content_chunk_hashes(&cid).await.unwrap();
        assert!(
            hashes.len() >= 2,
            "2.5MiB 文本必须切成多块（got {}）",
            hashes.len()
        );
        let refs: Vec<&str> = hashes.iter().map(String::as_str).collect();
        let entry_root = store
            .entry_by_path("/big-notes.md")
            .await
            .unwrap()
            .unwrap()
            .chunk_root
            .expect("大文件 entry 必有 chunk_root");
        assert_eq!(
            partisync_cas::chunk_root(&refs),
            entry_root,
            "P22-a：content_chunk 清单 seq 升序复算必须与 entry.chunk_root 对账相等"
        );

        let bytes = store
            .reassemble_content(&cas, &cid, Some(&entry_root))
            .await
            .unwrap();
        assert_eq!(bytes, big, "P22-b：重组 roundtrip 必须逐位还原原文件");

        // reindex 重组路径：源盘移除（source_root=None → 源缺失）→ 大文件经
        // content_chunk 清单重组读出 → marker 命中
        let stats = reindex_core(&store, &cas, &engine, None).await.unwrap();
        assert_eq!(
            stats.read_errors, 0,
            "有清单大文件重组必须成功（stats={stats:?}）"
        );
        engine.bm25_index().reload().unwrap(); // OnCommitWithDelay：显式 reload（bm25.rs 判例）
        let result = engine
            .bm25_only(Bm25Query {
                query: "xyzzyquux".into(),
                limit: 10,
                include_transcript: true,
            })
            .await
            .unwrap();
        assert_eq!(result.hits.len(), 1, "重组路径原文标记词必须命中");
        assert_eq!(result.hits[0].content_id, cid);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// P22-c 两类注入必显式 Err：人为删 CAS 块对象 → 缺块 Err；删清单行 →
    /// 拼接哈希失配 Err。绝不返回部分字节。
    #[tokio::test]
    async fn t04_missing_chunk_or_manifest_row_fails_explicitly() {
        let dir = std::env::temp_dir().join(format!("t04-inject-{}", Ulid::now()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let (store, cas, _engine) = t04_env(&dir).await;

        // 同 env 双文件（内容不同 ⇒ cid 与块集互不干扰）
        let big1 = big_text(2500 * 1024, "waldo");
        let big2 = big_text(2500 * 1024, "quux-waldo");
        std::fs::write(dir.join("src/inject1.md"), &big1).unwrap();
        std::fs::write(dir.join("src/inject2.md"), &big2).unwrap();
        partisync_graph::indexer::index_path(&store, Some(&cas), &dir.join("src"))
            .await
            .unwrap();
        let cid1 = partisync_cas::content_hash(&big1);
        let cid2 = partisync_cas::content_hash(&big2);
        let root1 = store
            .entry_by_path("/inject1.md")
            .await
            .unwrap()
            .unwrap()
            .chunk_root
            .unwrap();
        let root2 = store
            .entry_by_path("/inject2.md")
            .await
            .unwrap()
            .unwrap()
            .chunk_root
            .unwrap();

        // 注入①：人为删一块 CAS 对象（objects/<h[..2]>/<hash>）→ 缺块显式 Err
        let hashes = store.content_chunk_hashes(&cid1).await.unwrap();
        let victim = &hashes[hashes.len() / 2];
        std::fs::remove_file(dir.join("cas/objects").join(&victim[..2]).join(victim)).unwrap();
        let err = store
            .reassemble_content(&cas, &cid1, Some(&root1))
            .await
            .expect_err("P22-c：任一块 CAS 缺失必 Err，绝不返回部分字节");
        assert!(
            format!("{err}").contains("缺块"),
            "错误必须显式指向缺块（err={err}）"
        );

        // 注入②：删清单首行（清单扰动）→ 拼接 blake3 失配显式 Err
        sqlx::query("DELETE FROM content_chunk WHERE content_id = ? AND seq = 0")
            .bind(&cid2)
            .execute(store.pool_ref())
            .await
            .unwrap();
        let err = store
            .reassemble_content(&cas, &cid2, Some(&root2))
            .await
            .expect_err("P22-c：清单扰动必显式 Err");
        assert!(
            format!("{err}").contains("校验败") || format!("{err}").contains("对账败"),
            "错误必须显式指向校验/对账失败（err={err}）"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 存量无清单（v18 落表前入库，不可回填）→ read_errors 计数不静默。
    #[tokio::test]
    async fn t04_legacy_content_without_manifest_counts_read_errors() {
        let dir = std::env::temp_dir().join(format!("t04-legacy-{}", Ulid::now()));
        std::fs::create_dir_all(&dir).unwrap();
        let (store, cas, engine) = t04_env(&dir).await;

        // 存量形态：entry 带 chunk_root（大文件），但清单从未持久化且源盘缺失
        let legacy = b"legacy big content bytes marker waldo";
        let cid = partisync_cas::content_hash(legacy);
        sqlx::query("INSERT INTO content (id, size, mime) VALUES (?, ?, NULL)")
            .bind(&cid)
            .bind(legacy.len() as i64)
            .execute(store.pool_ref())
            .await
            .unwrap();
        store
            .add_entry(
                None,
                "legacy.md",
                "/legacy.md",
                EntryKind::File,
                legacy.len() as u64,
                1_700_000_000_000_000_000,
                Some((cid.as_str(), legacy.len() as u64)),
                Some(&"ab".repeat(32)),
            )
            .await
            .unwrap();

        let stats = reindex_core(&store, &cas, &engine, None).await.unwrap();
        assert_eq!(
            stats.read_errors, 1,
            "存量无清单必须显式计 read_errors（不可回填，不静默截断）"
        );
        assert_eq!(stats.indexed, 0, "清单缺失的内容不得进索引");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
