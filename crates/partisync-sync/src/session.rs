//! 同步会话（SPEC M2-WP01 契约 §3）：push/pull 两个 Store 之间的 oplog 交换。
//!
//! v1：对端为同进程 Store 句柄（网络传输归 M2-WP04 iroh）。语义：
//! - 应用 = 设备自有域全量行 upsert（属主跟随 payload）/ remove；
//! - 回环防护：origin_device == 对端本机 的行跳过（A→B→A 不再产生变更）；
//! - 中继：应用成功的行进对端 oplog（origin 保留原值）——多节点传播靠它；
//! - ACK 裁剪：对端确认的 hlc 从本端 oplog 删除。

use partisync_core::error::{PartisyError, Severity};
use partisync_graph::store::{EntryKind, Store};
use serde_json::Value;

/// push 统计。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncStats {
    pub applied: u64,
    pub skipped_self_origin: u64,
    pub conflicts: u64,
}

/// 把 `src` 的 oplog 推送到 `dst` 并 ACK 裁剪。
///
/// # Errors
/// DB 错误 → Fatal（单行应用错误亦按 Fatal 上抛——oplog 保留待重试）。
pub async fn push(src: &Store, dst: &Store) -> Result<SyncStats, PartisyError> {
    let rows = src.pending_oplog().await?;
    let dst_device = dst.device_id().await?;
    let mut stats = SyncStats::default();
    let mut acked: Vec<String> = Vec::new();
    for row in &rows {
        if row.origin_device == dst_device {
            stats.skipped_self_origin += 1;
            acked.push(row.hlc.clone()); // 对端自己的行原样回投：直接 ACK（幂等无效果）
            continue;
        }
        apply_row(dst, row, &mut stats).await?;
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

async fn apply_row(
    dst: &Store,
    row: &partisync_graph::store::OplogRow,
    stats: &mut SyncStats,
) -> Result<(), PartisyError> {
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
            let before = dst.entry_by_path(&path).await?;
            dst.apply_remote_entry(
                &path, &name, kind, size, mtime_ns, content, chunk_root, &owner,
            )
            .await?;
            // 冲突检测：目标路径已有不同属主行 → apply 内部改挂 .conflict-* 路径
            if before.is_some() {
                let after = dst.entry_by_path(&path).await?;
                if after.is_some_and(|a| a.owner_device.as_deref() != Some(owner.as_str())) {
                    stats.conflicts += 1;
                }
            }
            stats.applied += 1;
            // 中继：进对端 oplog（origin 保留），供多节点传播
            dst.record_oplog(
                &row.space_id,
                row.domain,
                &row.entity,
                &row.entity_id,
                &row.op,
                &row.origin_device,
                &row.payload,
            )
            .await?;
        }
        ("entry", "remove") => {
            let path = payload["path"].as_str().unwrap_or_default().to_string();
            dst.remove_entry(&path).await?;
            stats.applied += 1;
            dst.record_oplog(
                &row.space_id,
                row.domain,
                &row.entity,
                &row.entity_id,
                &row.op,
                &row.origin_device,
                &row.payload,
            )
            .await?;
        }
        _ => {} // 未知实体：v1 忽略（向后兼容）
    }
    Ok(())
}
