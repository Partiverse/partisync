//! 远端索引器（SPEC M1-WP02/M1-WP08）：Provider 树 → PartiGraph。
//!
//! 与本地 [`crate::indexer`] 同构的两阶段设计（M1-WP08 v1.1 作业化）：
//! ① collect——递归 list 收集全部条目；② 全局字典序处理（checkpoint 的
//! `vpath ≤ cp` 划界要求「处理序 == 字典序」，与本地同理由）。
//! 内容身份 = 下载后 BLAKE3（远端 ETag 可能是 MPU composite，不可作内容身份）；
//! v1 不落本地 CAS 块（远端缓存归 M1-WP03）。
//! `bwlimit`：可选限速（令牌桶，SPEC M1-WP08）。

use std::collections::HashMap;
use std::time::Instant;

use partisync_cas::content_hash;
use partisync_core::error::{PartisyError, Severity};
use partisync_provider::Provider;

use crate::jobs::JobCtx;
use crate::store::{EntryKind, Store};

/// 限速器：简单令牌桶（bytes/s；None = 不限速）。
pub struct RateLimiter {
    bytes_per_sec: Option<f64>,
    window_start: Instant,
    window_bytes: u64,
}

impl RateLimiter {
    #[must_use]
    pub fn new(bytes_per_sec: Option<u64>) -> Self {
        RateLimiter {
            bytes_per_sec: bytes_per_sec.map(|b| b as f64),
            window_start: Instant::now(),
            window_bytes: 0,
        }
    }

    /// 消费 n 字节；超速则阻塞补足（index-remote 下载循环调用）。
    pub async fn consume(&mut self, n: u64) {
        let Some(bps) = self.bytes_per_sec else {
            return;
        };
        self.window_bytes += n;
        let elapsed = self.window_start.elapsed().as_secs_f64();
        let allowed = elapsed * bps;
        if self.window_bytes as f64 > allowed {
            let wait = (self.window_bytes as f64 - allowed) / bps;
            tokio::time::sleep(std::time::Duration::from_secs_f64(wait)).await;
        }
        // 窗口重置（防长期漂移）
        if elapsed > 10.0 {
            self.window_start = Instant::now();
            self.window_bytes = 0;
        }
    }
}

/// 远端索引结果摘要。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RemoteIndexReport {
    pub files: u64,
    pub dirs: u64,
    pub bytes_hashed: u64,
}

fn interrupted() -> PartisyError {
    PartisyError {
        severity: Severity::Interrupted,
        source: Some("远端索引被中断（checkpoint 已持久化）".into()),
    }
}

/// 迭代收集远端条目（显式栈——async fn 递归需 boxing，此处避免）。
async fn collect(
    provider: &Provider,
    prefix: &str,
    out: &mut Vec<(String, partisync_provider::ProviderEntry)>,
) -> Result<(), PartisyError> {
    let mut stack: Vec<String> = vec![prefix.trim_matches('/').to_string()];
    while let Some(dir) = stack.pop() {
        // OpenDAL 语义：list 返回的 path 自带父前缀（相对 operator root）——
        // 不可再拼接（实测踩坑：dav-src/dav-src/ 双重前缀）
        for entry in provider.list_children(&dir).await? {
            let is_dir = entry.is_dir;
            let path = entry.path.clone();
            if is_dir {
                stack.push(path.clone());
            }
            out.push((path, entry));
        }
    }
    Ok(())
}

/// 索引远端 `prefix` 下全部条目。幂等；支持作业上下文（checkpoint/中断/限速）。
///
/// # Errors
/// 列目录/读文件失败按分类透传；`stop_after` 达到 → Interrupted。
pub async fn index_provider(
    store: &Store,
    provider: &Provider,
    prefix: &str,
    mut job: Option<&mut JobCtx>, // 作业上下文（checkpoint/中断）
    bwlimit: Option<u64>,         // 下载限速 bytes/s
) -> Result<RemoteIndexReport, PartisyError> {
    let mut report = RemoteIndexReport::default();
    let root_id = store
        .add_entry(None, "remote", "/", EntryKind::Dir, 0, 0, None, None)
        .await?;
    let mut dir_ids: HashMap<String, String> = HashMap::new();
    dir_ids.insert(String::new(), root_id);

    // ① 收集 + 全局字典序
    let prefix_trim = prefix.trim_matches('/').to_string();
    let mut all: Vec<(String, partisync_provider::ProviderEntry)> = Vec::new();
    collect(provider, &prefix_trim, &mut all).await?;
    all.sort_by(|a, b| a.0.cmp(&b.0));

    let mut limiter = RateLimiter::new(bwlimit);

    // ② 字典序处理（checkpoint 划界语义与本地一致）
    for (path, entry) in &all {
        let name = path.rsplit('/').next().unwrap_or("").to_string();
        let parent_dir = path
            .rsplit_once('/')
            .map_or(String::new(), |(p, _)| p.to_string());
        let vpath = format!("/{path}");
        if entry.is_dir {
            let parent_id = dir_ids.get(&parent_dir).cloned();
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
            dir_ids.insert(path.clone(), id);
            report.dirs += 1;
            continue;
        }
        // 文件：checkpoint 划界跳过（处理序 == 字典序 ⇒ 无交叠）
        let skipped = match job.as_deref_mut() {
            Some(ctx) => ctx
                .skip_up_to
                .as_deref()
                .is_some_and(|cp| vpath.as_str() <= cp),
            None => false,
        };
        if skipped {
            continue;
        }
        let data = provider.read_file(path).await?;
        limiter.consume(data.len() as u64).await;
        let hash = content_hash(&data);
        let parent_id = dir_ids.get(&parent_dir).cloned();
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
        if let Some(ctx) = job.as_deref_mut() {
            ctx.done += 1;
            ctx.flush_checkpoint(store, &vpath).await?;
            if let Some(limit) = ctx.stop_after {
                if ctx.done >= limit {
                    return Err(interrupted());
                }
            }
        }
    }
    Ok(report)
}
