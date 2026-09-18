//! 事件队列：scan_journal 的记录与幂等应用（SPEC M0-WP04 契约 §2）。
//!
//! P8 口径：事件先落盘后应用，应用幂等（upsert/remove），崩溃重放收敛。

use std::path::Path;
use std::time::UNIX_EPOCH;

use partisync_cas::chunker::CdcConfig;
use partisync_cas::{content_hash_file, put_chunks, ChunkStore};
use partisync_core::error::{classify_io, PartisyError, Severity};

use crate::indexer::index_path;
use crate::store::{EntryKind, Store};

/// 事件类别（journal.kind）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Created = 0,
    Modified = 1,
    Removed = 2,
}

/// journal 行。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct JournalEvent {
    pub seq: i64,
    pub path: String,
    pub kind: i64,
    pub at_ns: i64,
}

/// 一轮应用的结果摘要。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Applied {
    pub created: u64,
    pub modified: u64,
    pub removed: u64,
    pub skipped: u64,
}

fn db_err(what: &str, e: sqlx::Error) -> PartisyError {
    PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("{what}: {e}").into()),
    }
}

fn io_err(what: &'static str, e: std::io::Error) -> PartisyError {
    let severity = classify_io(e.kind());
    let source: Box<dyn std::error::Error + Send + Sync> = format!("{what}: {e}").into();
    PartisyError {
        severity,
        source: Some(source),
    }
}

/// 记录事件到 journal（应用前落盘——P8）。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn record(store: &Store, path: &str, kind: EventKind) -> Result<(), PartisyError> {
    let at_ns = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as i64);
    sqlx::query("INSERT INTO scan_journal (path, kind, at_ns) VALUES (?, ?, ?)")
        .bind(path)
        .bind(kind as i64)
        .bind(at_ns)
        .execute(store.pool_ref())
        .await
        .map_err(|e| db_err("记录事件", e))?;
    Ok(())
}

/// 取全部待应用事件（seq 序）。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn pending(store: &Store) -> Result<Vec<JournalEvent>, PartisyError> {
    sqlx::query_as::<_, JournalEvent>(
        "SELECT seq, path, kind, at_ns FROM scan_journal ORDER BY seq",
    )
    .fetch_all(store.pool_ref())
    .await
    .map_err(|e| db_err("读取事件", e))
}

/// 应用全部 pending 事件；成功后删行。幂等：重放同批事件不改变图谱（P8）。
///
/// # Errors
/// 单事件 IO 错误按分类透传；DB 错误 → Fatal。
pub async fn apply_pending(
    store: &Store,
    cas: Option<&ChunkStore>,
    fs_root: &Path,
) -> Result<Applied, PartisyError> {
    let events = pending(store).await?;
    let mut applied = Applied::default();
    for ev in &events {
        let fs_path = fs_root.join(ev.path.trim_start_matches('/'));
        match ev.kind {
            2 => {
                store.remove_entry(&ev.path).await?;
                applied.removed += 1;
            }
            0 | 1 => {
                let meta = match std::fs::metadata(&fs_path) {
                    Ok(m) => m,
                    // fs 已消失（created 后又被删）：按 removed 自愈
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        store.remove_entry(&ev.path).await?;
                        applied.removed += 1;
                        continue;
                    }
                    Err(e) => return Err(io_err("读取事件目标", e)),
                };
                if meta.is_dir() {
                    // 新目录：为该子树跑一次幂等索引（覆盖批量创建）
                    index_path(store, cas, &fs_path).await?;
                    applied.created += 1;
                    continue;
                }
                let mtime_ns = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos() as u64);
                let hash = content_hash_file(&fs_path).await?;
                let mut chunk_root = None;
                if let Some(cas) = cas.filter(|_| meta.len() as usize >= CdcConfig::PRODUCTION.min)
                {
                    let data = tokio::fs::read(&fs_path)
                        .await
                        .map_err(|e| io_err("读文件分块", e))?;
                    let (_, root_hash) = put_chunks(cas, &data, CdcConfig::PRODUCTION).await?;
                    chunk_root = Some(root_hash);
                }
                // 中途目录链：确保父目录条目存在（mkdir -p 语义）
                ensure_dir_chain(store, &ev.path).await?;
                let parent_path = parent_vpath(&ev.path);
                let parent_id = store.entry_by_path(&parent_path).await?.map(|e| e.id);
                let name = ev.path.rsplit('/').next().unwrap_or("").to_string();
                let existed = store.entry_by_path(&ev.path).await?.is_some();
                store
                    .add_entry(
                        parent_id.as_deref(),
                        &name,
                        &ev.path,
                        EntryKind::File,
                        meta.len(),
                        mtime_ns,
                        Some((&hash, meta.len())),
                        chunk_root.as_deref(),
                    )
                    .await?;
                if ev.kind == 0 && !existed {
                    applied.created += 1;
                } else {
                    applied.modified += 1;
                }
            }
            _ => applied.skipped += 1,
        }
        sqlx::query("DELETE FROM scan_journal WHERE seq = ?")
            .bind(ev.seq)
            .execute(store.pool_ref())
            .await
            .map_err(|e| db_err("消费事件", e))?;
    }
    Ok(applied)
}

async fn ensure_dir_chain(store: &Store, vpath: &str) -> Result<(), PartisyError> {
    let mut prefix = String::new();
    let segs: Vec<&str> = vpath.trim_start_matches('/').split('/').collect();
    for seg in segs.iter().take(segs.len().saturating_sub(1)) {
        prefix = format!("{prefix}/{seg}");
        if store.entry_by_path(&prefix).await?.is_none() {
            let parent = parent_vpath(&prefix);
            let parent_id = store.entry_by_path(&parent).await?.map(|e| e.id);
            store
                .add_entry(
                    parent_id.as_deref(),
                    seg,
                    &prefix,
                    EntryKind::Dir,
                    0,
                    0,
                    None,
                    None,
                )
                .await?;
        }
    }
    Ok(())
}

fn parent_vpath(vpath: &str) -> String {
    match vpath.rsplit_once('/') {
        Some((p, _)) if p.is_empty() => "/".to_string(),
        Some((p, _)) => p.to_string(),
        None => "/".to_string(),
    }
}
