//! 最小索引器（SPEC M0-WP02 契约 §3）：walkdir 递归 → 图谱 + 内容身份。
//!
//! 完整扫描器（watcher/journal/断点续扫）归 M0-WP04/WP05。
//! 符号链接 v1 跳过（SPEC 非目标：防止循环与指断裂图）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use partisync_cas::chunker::CdcConfig;
use partisync_cas::{content_hash_file, put_chunks, ChunkStore};
use partisync_core::error::{classify_io, PartisyError, Severity};
use walkdir::WalkDir;

use crate::store::{EntryKind, Store};

/// 分块阈值：文件 ≥ min 才进块库（小文件整文件去重更划算）。
fn chunk_threshold() -> usize {
    CdcConfig::PRODUCTION.min
}

/// 递归索引 `root` 下的全部目录与文件。幂等：重复索引 stats 不变。
///
/// # Errors
/// 根不可读 → 按表驱动分类；单文件哈希失败按 [`classify_io`] 分类
/// （NotFound 等 Retryable → 整体中止返回，重跑幂等续传）。
pub async fn index_path(
    store: &Store,
    cas: Option<&ChunkStore>,
    root: &Path,
) -> Result<IndexReport, PartisyError> {
    let root = root.canonicalize().map_err(|e| io_err("定位索引根", e))?;
    let root_name = root
        .file_name()
        .map_or_else(|| "/".to_string(), |n| n.to_string_lossy().into_owned());
    // 根条目路径固定为 "/"：UI 导航的统一锚点
    let mut dir_ids: HashMap<PathBuf, String> = HashMap::new();
    let root_id = store
        .add_entry(None, &root_name, "/", EntryKind::Dir, 0, 0, None, None)
        .await?;
    dir_ids.insert(root.clone(), root_id);

    let mut report = IndexReport::default();
    for item in WalkDir::new(&root).min_depth(1).follow_links(false) {
        let entry = item.map_err(|e| {
            // walkdir 把底层 io 包在 Error 里；按常见瞬态（路径消失等）归类
            match e.io_error() {
                Some(io) => io_err("遍历", std::io::Error::new(io.kind(), io.to_string())),
                None => PartisyError::new(Severity::Fatal),
            }
        })?;
        let rel = entry
            .path()
            .strip_prefix(&root)
            .map_err(|_| PartisyError::new(Severity::Fatal))?;
        let vpath = to_vpath(rel);
        let meta = entry.metadata().map_err(|e| match e.io_error() {
            Some(io) => io_err("读取元数据", std::io::Error::new(io.kind(), io.to_string())),
            None => PartisyError::new(Severity::Fatal),
        })?;
        if meta.is_symlink() {
            report.skipped_symlinks += 1;
            continue;
        }
        let parent_id = entry.path().parent().and_then(|p| dir_ids.get(p).cloned());
        let mtime_ns = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos() as u64);
        if meta.is_dir() {
            let id = store
                .add_entry(
                    parent_id.as_deref(),
                    &entry.file_name().to_string_lossy(),
                    &vpath,
                    EntryKind::Dir,
                    0,
                    mtime_ns,
                    None,
                    None,
                )
                .await?;
            dir_ids.insert(entry.path().to_owned(), id);
            report.dirs += 1;
        } else if meta.is_file() {
            let hash = content_hash_file(entry.path()).await?;
            // 大文件分块入库（块级去重/delta 的载体，SPEC M0-WP03 §3）
            let mut chunk_root = None;
            if let Some(cas) = cas.filter(|_| meta.len() as usize >= chunk_threshold()) {
                let data = tokio::fs::read(entry.path())
                    .await
                    .map_err(|e| io_err("读文件分块", e))?;
                let (_, root_hash) = put_chunks(cas, &data, CdcConfig::PRODUCTION).await?;
                chunk_root = Some(root_hash);
                report.chunked_files += 1;
            }
            store
                .add_entry(
                    parent_id.as_deref(),
                    &entry.file_name().to_string_lossy(),
                    &vpath,
                    EntryKind::File,
                    meta.len(),
                    mtime_ns,
                    Some((&hash, meta.len())),
                    chunk_root.as_deref(),
                )
                .await?;
            report.files += 1;
        }
    }
    Ok(report)
}

/// 索引结果摘要。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IndexReport {
    pub files: u64,
    pub dirs: u64,
    pub skipped_symlinks: u64,
    pub chunked_files: u64,
}

/// 相对路径 → 虚拟路径（"/" + 分隔符统一为 '/'）。
fn to_vpath(rel: &Path) -> String {
    let s = rel.to_string_lossy().replace('\\', "/");
    format!("/{s}")
}

fn io_err(what: &'static str, e: std::io::Error) -> PartisyError {
    let severity = classify_io(e.kind());
    let source: Box<dyn std::error::Error + Send + Sync> = format!("{what}: {e}").into();
    PartisyError {
        severity,
        source: Some(source),
    }
}
