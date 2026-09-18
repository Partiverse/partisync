//! watch 循环（SPEC M0-WP04 契约 §3）：notify 事件 → 去抖合并 → journal → 应用。
//!
//! 运行于阻塞线程（notify 为同步回调）；应用经 tokio Handle 桥接。
//! 退出语义：drop watcher ⇒ 通道断开 ⇒ 线程冲刷剩余事件后退出（CLI Ctrl-C 路径）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use partisync_core::error::{PartisyError, Severity};

use crate::journal::{self, EventKind};

/// watch 配置。
pub struct WatchConfig {
    /// 去抖窗口：静默该时长后冲刷一批（默认 1000ms）。
    pub debounce: Duration,
}

/// 运行 watch 循环直至事件源断开（watcher 被 drop）。阻塞当前线程。
///
/// # Errors
/// watcher 初始化失败 → Fatal；应用错误透传（事件保留待重放）。
pub fn run_blocking(
    store: crate::store::Store,
    cas: Option<partisync_cas::ChunkStore>,
    fs_root: PathBuf,
    cfg: WatchConfig,
) -> Result<(), PartisyError> {
    let (tx, rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = notify::recommended_watcher(tx)
        .map_err(|e| PartisyError::with_source(Severity::Fatal, Box::new(e)))?;
    watcher
        .watch(&fs_root, RecursiveMode::Recursive)
        .map_err(|e| PartisyError::with_source(Severity::Fatal, Box::new(e)))?;

    let handle = tokio::runtime::Handle::current();
    // path(虚拟) → 末态 kind（去抖窗口内按 path 合并）
    let mut pending: HashMap<String, EventKind> = HashMap::new();
    loop {
        match rx.recv_timeout(cfg.debounce) {
            Ok(Ok(event)) => {
                let kind = match event.kind {
                    notify::EventKind::Create(_) => Some(EventKind::Created),
                    notify::EventKind::Modify(_) => Some(EventKind::Modified),
                    notify::EventKind::Remove(_) => Some(EventKind::Removed),
                    _ => None,
                };
                if let Some(kind) = kind {
                    for path in event.paths {
                        if let Some(vpath) = to_vpath(&fs_root, &path) {
                            // 末态合并规则：Removed 压倒一切；Created+Modified → Modified 仍按末态
                            pending.insert(vpath, kind);
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                // 单事件源错误：记录后继续（瞬态 watched-path 竞争常见）
                eprintln!("watch: 事件源错误（忽略继续）: {e}");
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                flush(&handle, &store, cas.as_ref(), &fs_root, &mut pending);
                return Ok(());
            }
        }
        if !pending.is_empty() {
            // 窗口到期（timeout 分支或凑批）即冲刷
            flush(&handle, &store, cas.as_ref(), &fs_root, &mut pending);
        }
    }
}

fn flush(
    handle: &tokio::runtime::Handle,
    store: &crate::store::Store,
    cas: Option<&partisync_cas::ChunkStore>,
    fs_root: &Path,
    pending: &mut HashMap<String, EventKind>,
) {
    if pending.is_empty() {
        return;
    }
    let batch: Vec<(String, EventKind)> = pending.drain().collect();
    let summary = handle.block_on(async {
        for (path, kind) in &batch {
            if let Err(e) = journal::record(store, path, *kind).await {
                eprintln!("watch: 记录事件失败 {path}: {e}");
                return None;
            }
        }
        match journal::apply_pending(store, cas, fs_root).await {
            Ok(a) => Some(a),
            Err(e) => {
                eprintln!("watch: 应用失败（事件保留待重放）: {e}");
                None
            }
        }
    });
    if let Some(a) = summary {
        println!(
            "watch: {} 事件 → +{} ~{} -{}（跳过 {}）",
            batch.len(),
            a.created,
            a.modified,
            a.removed,
            a.skipped
        );
    }
}

/// fs 路径 → 虚拟路径（根下为 /xxx；根外 None）。
fn to_vpath(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    if rel.as_os_str().is_empty() {
        return Some("/".to_string());
    }
    Some(format!("/{}", rel.to_string_lossy().replace('\\', "/")))
}
