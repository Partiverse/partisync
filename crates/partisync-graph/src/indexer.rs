//! 最小索引器（SPEC M0-WP02 契约 §3；M0-WP05 增加作业上下文）。
//!
//! 处理序 = 全局字典序（先收集后排序）：checkpoint 的 `vpath ≤ cp` 划界要求
//! 「处理序 == 字典序」；walkdir 的 DFS 序在「同级 目录 vs 点号文件」上与字典序
//! 不一致（`/a.txt < /a/b` 但 DFS 先访问 `/a/`），会破坏 L5 恢复等价性。
//! v1 内存可承受（10⁶ 路径 ~50MB 量级）；流式归 WP07。
//! 符号链接 v1 跳过（SPEC 非目标：防止循环与指断裂图）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use partisync_cas::chunker::CdcConfig;
use partisync_cas::{content_hash_file, put_chunks, ChunkStore};
use partisync_core::error::{classify_io, PartisyError, Severity};
use walkdir::WalkDir;

use crate::jobs::JobCtx;
use crate::store::{EntryKind, Store};

/// 分块阈值：文件 ≥ min 才进块库（小文件整文件去重更划算）。
fn chunk_threshold() -> usize {
    CdcConfig::PRODUCTION.min
}

/// 中断信号（SPEC M0-WP05）：stop_after 达到时返回，severity = Interrupted。
fn interrupted() -> PartisyError {
    PartisyError {
        severity: Severity::Interrupted,
        source: Some("作业被中断（checkpoint 已持久化）".into()),
    }
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
    index_path_job(store, cas, root, None).await
}

/// 带作业上下文的索引（checkpoint 跳过 / 提交 / stop_after 注入）。
///
/// # Errors
/// 同 [`index_path`]；`stop_after` 达到 → Interrupted。
pub async fn index_path_job(
    store: &Store,
    cas: Option<&ChunkStore>,
    root: &Path,
    mut job: Option<&mut JobCtx>,
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

    // 收集 + 全局字典序（见模块注释）
    let mut all: Vec<PathBuf> = WalkDir::new(&root)
        .min_depth(1)
        .follow_links(false)
        .into_iter()
        .filter_map(|item| match item {
            Ok(e) => Some(e.into_path()),
            Err(e) => {
                // 遍历中的瞬态错误（路径消失等）：跳过该项继续
                let msg = e
                    .io_error()
                    .map_or_else(|| format!("{e}"), ToString::to_string);
                eprintln!("index: 遍历跳过: {msg}");
                None
            }
        })
        .collect();
    all.sort_by_cached_key(|p| to_vpath(p.strip_prefix(&root).unwrap_or(p)));

    let mut report = IndexReport::default();
    for entry_path in &all {
        let rel = entry_path
            .strip_prefix(&root)
            .map_err(|_| PartisyError::new(Severity::Fatal))?;
        let vpath = to_vpath(rel);
        let meta = std::fs::symlink_metadata(entry_path).map_err(|e| io_err("读取元数据", e))?;
        if meta.is_symlink() {
            report.skipped_symlinks += 1;
            continue;
        }
        let parent_id = entry_path.parent().and_then(|p| dir_ids.get(p).cloned());
        let mtime_ns = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos() as u64);
        let name = entry_path
            .file_name()
            .map_or_else(|| "/".to_string(), |n| n.to_string_lossy().into_owned());
        if meta.is_dir() {
            let id = store
                .add_entry(
                    parent_id.as_deref(),
                    &name,
                    &vpath,
                    EntryKind::Dir,
                    0,
                    mtime_ns,
                    None,
                    None,
                )
                .await?;
            dir_ids.insert(entry_path.clone(), id);
            report.dirs += 1;
        } else if meta.is_file() {
            // 作业语义：checkpoint 划界内的文件跳过哈希/分块（目录不跳）。
            // L5 等价性：处理序 == 字典序 ⇒ 跳过段与已处理段无交叠、结构 upsert 幂等。
            if let Some(ctx) = job.as_deref_mut() {
                if ctx
                    .skip_up_to
                    .as_deref()
                    .is_some_and(|cp| vpath.as_str() <= cp)
                {
                    continue;
                }
            }
            let hash = content_hash_file(entry_path).await?;
            // 内容未变的既有条目跳过分块重入库（否则 put 的引用计数在幂等重跑中膨胀
            // ——实测踩坑：43 refs → 86）
            let unchanged = match store.entry_by_path(&vpath).await? {
                Some(prev) => prev.content_id.as_deref() == Some(hash.as_str()),
                None => false,
            };
            // 大文件分块入库（块级去重/delta 的载体，SPEC M0-WP03 §3）
            let mut chunk_root = None;
            if let Some(cas) = cas
                .filter(|_| meta.len() as usize >= chunk_threshold())
                .filter(|_| !unchanged)
            {
                let data = tokio::fs::read(entry_path)
                    .await
                    .map_err(|e| io_err("读文件分块", e))?;
                let (_, root_hash) = put_chunks(cas, &data, CdcConfig::PRODUCTION).await?;
                chunk_root = Some(root_hash);
                report.chunked_files += 1;
            }
            store
                .add_entry(
                    parent_id.as_deref(),
                    &name,
                    &vpath,
                    EntryKind::File,
                    meta.len(),
                    mtime_ns,
                    Some((&hash, meta.len())),
                    chunk_root.as_deref(),
                )
                .await?;
            report.files += 1;
            if let Some(ctx) = job.as_deref_mut() {
                ctx.done += 1;
                if let Some(limit) = ctx.stop_after {
                    if ctx.done >= limit {
                        ctx.flush_final(store, &vpath).await?;
                        return Err(interrupted());
                    }
                }
                ctx.maybe_flush(store, &vpath).await?;
            }
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
