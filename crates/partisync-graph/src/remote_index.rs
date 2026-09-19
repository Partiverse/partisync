//! 远端索引器（SPEC M1-WP02）：Provider（S3/WebDAV/fs）目录树 → PartiGraph。
//!
//! 与本地 [`crate::indexer`] 并列，共享 entry/闭包语义：
//! - 处理序 = 字典序（确定性；远端 v1 无 checkpoint，作业化归后续任务）；
//! - 内容身份 = 下载后 BLAKE3（PartiSync 统一口径——远端 ETag 可能是
//!   MPU composite，不可作内容身份，调研方案 §3.1）；
//! - v1 不落本地 CAS 块（远端缓存/分层归 M1-WP03），仅记内容身份与 size；
//! - root 条目 = `/`（与本地一致，UI 导航锚点）。

use partisync_core::error::{PartisyError, Severity};
use partisync_provider::Provider;

use crate::store::{EntryKind, Store};

/// 远端索引结果摘要。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RemoteIndexReport {
    pub files: u64,
    pub dirs: u64,
    pub bytes_hashed: u64,
}

/// 递归索引远端 `prefix` 下全部条目。幂等：重复索引 stats 不变。
///
/// # Errors
/// 根不可列 → Fatal；单文件读取失败按分类透传（重试型错误整体中止，重跑幂等续传）。
pub async fn index_provider(
    store: &Store,
    provider: &Provider,
    prefix: &str,
) -> Result<RemoteIndexReport, PartisyError> {
    let mut report = RemoteIndexReport::default();
    let root_id = store
        .add_entry(None, "remote", "/", EntryKind::Dir, 0, 0, None, None)
        .await?;
    let mut dir_ids: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    dir_ids.insert(String::new(), root_id);

    // 逐层 BFS：目录字典序展开（远端 list 天然按层；无 checkpoint 需求故不强求全局序）
    let mut queue: std::collections::VecDeque<(String, String)> = std::collections::VecDeque::new(); // (provider dir, graph parent path)
    queue.push_back((prefix.trim_matches('/').to_string(), String::new()));
    while let Some((dir, graph_parent)) = queue.pop_front() {
        let children = provider.list_children(&dir).await?;
        // 字典序：先目录后文件、名称序（与本地索引器口径一致）
        let mut sorted = children;
        sorted.sort_by(|a, b| (!a.is_dir).cmp(&(!b.is_dir)).then(a.path.cmp(&b.path)));
        for entry in sorted {
            let name = entry.path.rsplit('/').next().unwrap_or("").to_string();
            let vpath = format!("{graph_parent}/{name}");
            if entry.is_dir {
                let parent_id = dir_ids.get(&graph_parent).cloned();
                let id = store
                    .add_entry(
                        parent_id.as_deref(),
                        &name,
                        &vpath,
                        EntryKind::Dir,
                        0,
                        entry.mtime_ns,
                        None,
                        None,
                    )
                    .await?;
                dir_ids.insert(vpath.clone(), id);
                report.dirs += 1;
                queue.push_back((entry.path.clone(), vpath));
            } else {
                // 内容身份：下载后 BLAKE3（远端 ETag 不可信为内容身份，SPEC 注记）
                let data = provider.read_file(&entry.path).await?;
                let hash = partisync_cas::content_hash(&data);
                let parent_id = dir_ids.get(&graph_parent).cloned();
                store
                    .add_entry(
                        parent_id.as_deref(),
                        &name,
                        &vpath,
                        EntryKind::File,
                        entry.size,
                        entry.mtime_ns,
                        Some((&hash, entry.size)),
                        None, // 远端分块（本地缓存）归 M1-WP03
                    )
                    .await?;
                report.files += 1;
                report.bytes_hashed += entry.size;
            }
        }
    }
    Ok(report)
}

/// 构造错误帮助（未用 Severity 变体时保留分类学一致性）。
#[allow(dead_code)]
fn fatal(msg: impl Into<String>) -> PartisyError {
    PartisyError {
        severity: Severity::Fatal,
        source: Some(msg.into().into()),
    }
}
