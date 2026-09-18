//! 最小索引器（SPEC M0-WP02 §3；M0-WP05 作业上下文；M0-WP07 批处理 KPI 优化）。
//!
//! 处理序 = 全局字典序（先收集后排序）：checkpoint 的 `vpath ≤ cp` 划界要求
//! 「处理序 == 字典序」；walkdir 的 DFS 序在「同级 目录 vs 点号文件」上与字典序
//! 不一致（`/a.txt < /a/b` 但 DFS 先访问 `/a/`），会破坏 L5 恢复等价性。
//! 批处理（200/批）：批内文件并发哈希（JoinSet 隐藏 I/O）+ 单事务批量插入。
//! 符号链接 v1 跳过（SPEC 非目标：防止循环与指断裂图）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use partisync_cas::chunker::CdcConfig;
use partisync_cas::{content_hash_file, put_chunks, ChunkStore};
use partisync_core::error::{classify_io, PartisyError, Severity};
use walkdir::WalkDir;

use crate::jobs::JobCtx;
use crate::store::{EntryKind, FileInsert, Store};

/// 批大小（与 jobs checkpoint 粒度一致，SPEC M0-WP05 v1.1）。
const BATCH_SIZE: usize = 200;

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
    for batch in all.chunks(BATCH_SIZE) {
        // —— 目录（顺序插入，量少；字典序中目录先于其子项 ⇒ 父 id 恒可用）——
        let mut file_paths: Vec<(PathBuf, String)> = Vec::new();
        for entry_path in batch {
            let Ok(meta) = std::fs::symlink_metadata(entry_path) else {
                continue; // 扫描间隙消失：journal/watch 兜底
            };
            if meta.is_symlink() {
                report.skipped_symlinks += 1;
                continue;
            }
            let vpath = to_vpath(
                entry_path
                    .strip_prefix(&root)
                    .map_err(|_| PartisyError::new(Severity::Fatal))?,
            );
            if meta.is_dir() {
                let parent_id = entry_path.parent().and_then(|p| dir_ids.get(p).cloned());
                let name = entry_path
                    .file_name()
                    .map_or_else(|| "/".to_string(), |n| n.to_string_lossy().into_owned());
                let id = store
                    .add_entry(
                        parent_id.as_deref(),
                        &name,
                        &vpath,
                        EntryKind::Dir,
                        0,
                        mtime_of(&meta),
                        None,
                        None,
                    )
                    .await?;
                dir_ids.insert(entry_path.clone(), id);
                report.dirs += 1;
            } else if meta.is_file() {
                // 作业语义：checkpoint 划界内的文件跳过（L5：处理序 == 字典序）
                if let Some(ctx) = job.as_deref_mut() {
                    if ctx
                        .skip_up_to
                        .as_deref()
                        .is_some_and(|cp| vpath.as_str() <= cp)
                    {
                        continue;
                    }
                }
                file_paths.push((entry_path.clone(), vpath));
            }
        }
        if file_paths.is_empty() {
            continue;
        }

        // —— 文件哈希（并发，隐藏小文件 I/O 延迟）——
        let mut js = tokio::task::JoinSet::new();
        for (path, vpath) in file_paths {
            let big = std::fs::metadata(&path)
                .map(|m| m.len() as usize >= chunk_threshold())
                .unwrap_or(false)
                && cas.is_some();
            js.spawn(async move {
                let hash = if big {
                    content_hash_file(&path).await?
                } else {
                    let data = tokio::fs::read(&path)
                        .await
                        .map_err(|e| io_err("读文件", e))?;
                    partisync_cas::content_hash(&data)
                };
                Ok::<_, PartisyError>((path, vpath, hash))
            });
        }
        let mut hashed: Vec<(PathBuf, String, String)> = Vec::new();
        while let Some(joined) = js.join_next().await {
            let (path, vpath, hash) =
                joined.map_err(|e| PartisyError::with_source(Severity::Fatal, Box::new(e)))??;
            hashed.push((path, vpath, hash));
        }
        hashed.sort_by(|a, b| a.1.cmp(&b.1)); // 恢复字典序（checkpoint 划界依赖）

        // —— 组装 + 单事务批量插入 ——
        let mut records: Vec<FileInsert> = Vec::with_capacity(hashed.len());
        let mut last_vpath = String::new();
        for (path, vpath, hash) in hashed {
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            let parent_id = path.parent().and_then(|p| dir_ids.get(p).cloned());
            let name = path
                .file_name()
                .map_or_else(|| "/".to_string(), |n| n.to_string_lossy().into_owned());
            let mut chunk_root = None;
            if let Some(cas) = cas.filter(|_| meta.len() as usize >= chunk_threshold()) {
                // 内容未变的既有条目跳过分块重入库（防引用计数膨胀，实测踩坑 43→86）
                let unchanged = match store.entry_by_path(&vpath).await? {
                    Some(prev) => prev.content_id.as_deref() == Some(hash.as_str()),
                    None => false,
                };
                if !unchanged {
                    let data = tokio::fs::read(&path)
                        .await
                        .map_err(|e| io_err("读文件分块", e))?;
                    let (_, r) = put_chunks(cas, &data, CdcConfig::PRODUCTION).await?;
                    chunk_root = Some(r);
                    report.chunked_files += 1;
                }
            }
            records.push(FileInsert {
                id: partisync_core::Ulid::now().to_string(),
                parent_id,
                name,
                path: vpath.clone(),
                size: meta.len(),
                mtime_ns: mtime_of(&meta),
                content: Some((hash, meta.len())),
                chunk_root,
            });
            last_vpath = vpath;
        }
        store.add_file_batch(&records).await?;
        report.files += records.len() as u64;

        // —— 作业进度（随批提交，SPEC M0-WP05 v1.1：粒度=批）——
        if let Some(ctx) = job.as_deref_mut() {
            ctx.done += records.len() as u64;
            if let Some(limit) = ctx.stop_after {
                if ctx.done >= limit {
                    ctx.flush_checkpoint(store, &last_vpath).await?;
                    return Err(interrupted());
                }
            }
            ctx.flush_checkpoint(store, &last_vpath).await?;
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

fn mtime_of(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos() as u64)
}

fn io_err(what: &'static str, e: std::io::Error) -> PartisyError {
    let severity = classify_io(e.kind());
    let source: Box<dyn std::error::Error + Send + Sync> = format!("{what}: {e}").into();
    PartisyError {
        severity,
        source: Some(source),
    }
}
