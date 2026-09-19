//! 同步会话（SPEC M2-WP01 契约 §3 / M2-WP02 契约 §2–§5）：push/pull/bisync。
//!
//! 语义：
//! - 应用 = 设备自有域全量行 upsert（属主跟随 payload）/ remove；共享域 HLC LWW；
//! - 回环防护：origin_device == 对端本机 的行跳过（A→B→A 不再产生变更）；
//! - 中继：应用成功的行进对端 oplog——`record_oplog_raw` 保持原 HLC 键
//!   （一个写入全网同一个键，同写多径 `INSERT OR IGNORE` 去重；M2-WP02 契约 §2）；
//!   共享域 LWW 落选行不转发（陈旧写无需再广播）；
//! - 冲突：保留两者（apply 内改挂 `.conflict-*` 后缀）+ 血缘落档 `sync_conflict`（P11）；
//! - ACK 裁剪：对端确认的 hlc 从本端 oplog 删除（落选行也已「见过」——对端状态
//!   编码了 LWW 裁决，无需重投）；
//! - max-delete：批内待应用 entry 删除数超阈 → 整批拒绝（先拒后用）。

use partisync_core::error::{PartisyError, Severity};
use partisync_graph::store::{EntryKind, OplogRow, Store};
use serde_json::Value;

/// push 统计。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncStats {
    pub applied: u64,
    pub skipped_self_origin: u64,
    /// 共享域 LWW 落选/已见行（M2-WP02）。
    pub skipped_lww: u64,
    pub conflicts: u64,
}

/// push 选项（M2-WP02 契约 §5）。
#[derive(Debug, Clone, Copy, Default)]
pub struct PushOpts {
    /// 单批删除安全阈（rclone `--max-delete` 同语义；None = 不限）。
    pub max_delete: Option<u64>,
}

/// bisync 选项（M2-WP02 契约 §5）。
#[derive(Debug, Clone, Copy)]
pub struct BisyncOpts {
    pub max_delete: Option<u64>,
    pub max_rounds: u32,
}

impl Default for BisyncOpts {
    fn default() -> Self {
        Self {
            max_delete: None,
            max_rounds: 8,
        }
    }
}

/// bisync 统计。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BisyncStats {
    pub rounds: u32,
    pub pushed: u64,
    pub pulled: u64,
    pub conflicts: u64,
}

/// 把 `src` 的 oplog 推送到 `dst` 并 ACK 裁剪。
///
/// # Errors
/// DB 错误 → Fatal（单行应用错误亦按 Fatal 上抛——oplog 保留待重试）。
pub async fn push(src: &Store, dst: &Store) -> Result<SyncStats, PartisyError> {
    push_opts(src, dst, PushOpts::default()).await
}

/// [`push`] 带安全阈：批内待应用 entry 删除数 > `max_delete` → 整批拒绝
/// （先于任何应用——宁可不动，不可误删）。
///
/// # Errors
/// 同 [`push`]；超阈值 → Fatal。
pub async fn push_opts(
    src: &Store,
    dst: &Store,
    opts: PushOpts,
) -> Result<SyncStats, PartisyError> {
    let rows = src.pending_oplog().await?;
    let dst_device = dst.device_id().await?;
    if let Some(max) = opts.max_delete {
        // 只计 entry 删除（数据安全）；tag 墓碑是元数据，不在此列
        let removes = rows
            .iter()
            .filter(|r| r.entity == "entry" && r.op == "remove" && r.origin_device != dst_device)
            .count() as u64;
        if removes > max {
            return Err(PartisyError {
                severity: Severity::Fatal,
                source: Some(
                    format!("max-delete 安全阈触发：批内 {removes} 个删除 > 阈值 {max}，整批拒绝")
                        .into(),
                ),
            });
        }
    }
    let mut stats = SyncStats::default();
    let mut acked: Vec<String> = Vec::new();
    for row in &rows {
        if row.origin_device == dst_device {
            stats.skipped_self_origin += 1;
            acked.push(row.hlc.clone()); // 对端自己的行原样回投：直接 ACK（幂等无效果）
            continue;
        }
        apply_row(dst, row, &mut stats).await?;
        // 水位记账（M2-WP03）：每行推进 origin 的 last_hlc（单调）
        dst.note_applied(&row.origin_device, &row.hlc).await?;
        acked.push(row.hlc.clone());
    }
    src.trim_oplog(&acked).await?;
    Ok(stats)
}

/// pull = 反向 push。
///
/// # Errors
/// 同 [`push`]。
pub async fn pull(src: &Store, dst: &Store) -> Result<SyncStats, PartisyError> {
    push(src, dst).await
}

/// 双向同步定衬（M2-WP02 契约 §5）：反复双向 push，直到整轮零应用（不动点）
/// 或 `max_rounds`（未达不动点的有界退出——全量对账归 WP03 兜底）。
///
/// # Errors
/// 同 [`push`]。
pub async fn bisync(a: &Store, b: &Store, opts: BisyncOpts) -> Result<BisyncStats, PartisyError> {
    let mut stats = BisyncStats::default();
    let po = PushOpts {
        max_delete: opts.max_delete,
    };
    loop {
        stats.rounds += 1;
        let ab = push_opts(a, b, po).await?;
        let ba = push_opts(b, a, po).await?;
        stats.pushed += ab.applied;
        stats.pulled += ba.applied;
        stats.conflicts += ab.conflicts + ba.conflicts;
        if (ab.applied == 0 && ba.applied == 0) || stats.rounds >= opts.max_rounds {
            break;
        }
    }
    Ok(stats)
}

/// 应用一行；返回是否实际生效（session 据此 ACK/转发——落选行不转发）。
async fn apply_row(
    dst: &Store,
    row: &OplogRow,
    stats: &mut SyncStats,
) -> Result<bool, PartisyError> {
    let payload: Value = serde_json::from_str(&row.payload).map_err(|e| PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("oplog payload 解析失败: {e}").into()),
    })?;
    match (row.entity.as_str(), row.op.as_str()) {
        ("entry", "upsert") => {
            let path = payload["path"].as_str().unwrap_or_default().to_string();
            let name = payload["name"].as_str().unwrap_or_default().to_string();
            let kind = if payload["kind"].as_i64() == Some(1) {
                EntryKind::Dir
            } else {
                EntryKind::File
            };
            let size = payload["size"].as_u64().unwrap_or(0);
            let mtime_ns = payload["mtime_ns"].as_u64().unwrap_or(0);
            let content = payload["content_id"]
                .as_str()
                .map(|h| (h, payload["size"].as_u64().unwrap_or(0)));
            let chunk_root = payload["chunk_root"].as_str();
            let owner = payload["owner_device"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();
            let outcome = dst
                .apply_remote_entry(
                    &path, &name, kind, size, mtime_ns, content, chunk_root, &owner,
                )
                .await?;
            // P11 血缘落档：本位版本留守 base_path，来方版本改挂 incoming_path
            if let Some(c) = &outcome.conflict {
                stats.conflicts += 1;
                dst.record_conflict(
                    &row.space_id,
                    &c.base_path,
                    &c.base_path,
                    &c.incoming_path,
                    &row.origin_device,
                    &row.hlc,
                )
                .await?;
            }
            stats.applied += 1;
            // 中继：进对端 oplog（保键 + origin 保留），供多节点传播
            relay(dst, row).await?;
            Ok(true)
        }
        ("entry", "remove") => {
            let path = payload["path"].as_str().unwrap_or_default().to_string();
            dst.remove_entry(&path).await?;
            stats.applied += 1;
            relay(dst, row).await?;
            Ok(true)
        }
        ("tag", "upsert") => {
            let id = payload["id"].as_str().unwrap_or_default();
            let name = payload["name"].as_str().unwrap_or_default();
            let color = payload["color"].as_str();
            let applied = dst
                .apply_remote_tag(id, name, color, false, &row.hlc)
                .await?;
            count_and_relay(dst, row, applied, stats).await?;
            Ok(applied)
        }
        ("tag", "remove") => {
            let id = payload["id"].as_str().unwrap_or_default();
            let applied = dst.apply_remote_tag(id, "", None, true, &row.hlc).await?;
            count_and_relay(dst, row, applied, stats).await?;
            Ok(applied)
        }
        ("entry_tag", "link") | ("entry_tag", "unlink") => {
            let tag_id = payload["tag_id"].as_str().unwrap_or_default();
            let entry_path = payload["entry_path"].as_str().unwrap_or_default();
            let applied = dst
                .apply_remote_tag_link(tag_id, entry_path, row.op == "unlink", &row.hlc)
                .await?;
            count_and_relay(dst, row, applied, stats).await?;
            Ok(applied)
        }
        _ => Ok(false), // 未知实体：v1 忽略（向后兼容）
    }
}

/// 中继：原键 + 原 origin 进对端 oplog（`INSERT OR IGNORE` 同写去重）。
async fn relay(dst: &Store, row: &OplogRow) -> Result<(), PartisyError> {
    dst.record_oplog_raw(
        &row.hlc,
        &row.space_id,
        row.domain,
        &row.entity,
        &row.entity_id,
        &row.op,
        &row.origin_device,
        &row.payload,
        row.at_ns,
    )
    .await
}

/// 共享域统计与条件转发（LWW 落选行不转发）。
async fn count_and_relay(
    dst: &Store,
    row: &OplogRow,
    applied: bool,
    stats: &mut SyncStats,
) -> Result<(), PartisyError> {
    if applied {
        stats.applied += 1;
        relay(dst, row).await?;
    } else {
        stats.skipped_lww += 1;
    }
    Ok(())
}
