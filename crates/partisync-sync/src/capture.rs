//! 变更捕获（SPEC M2-WP01 契约 §2）：图谱变更 → oplog 行。
//!
//! 捕获口径（SPEC 风险节）：**索引场景不捕获**（全量索引即全量状态，对端首次
//! 同步拉全量）；仅 watch/journal 应用路径显式调用——漏广播窗口与 P8 重放收敛一致。

use partisync_core::error::PartisyError;
use partisync_graph::store::Store;

/// 记录一次 entry upsert 变更（设备自有域，全量行 payload）。
///
/// # Errors
/// 条目不存在（此为 remove 误用）→ Fatal；DB 错误 → Fatal。
pub async fn record_entry_upsert(store: &Store, path: &str) -> Result<(), PartisyError> {
    let row = store
        .entry_by_path(path)
        .await?
        .ok_or_else(|| PartisyError {
            severity: partisync_core::error::Severity::Fatal,
            source: Some(format!("捕获目标不存在: {path}").into()),
        })?;
    let owner = row.owner_device.clone().unwrap_or_else(|| "unknown".into());
    let payload = serde_json::json!({
        "path": row.path, "name": row.name, "kind": row.kind,
        "size": row.size, "mtime_ns": row.mtime_ns,
        "content_id": row.content_id, "chunk_root": row.chunk_root,
        "owner_device": owner,
    });
    store
        .record_oplog(
            "default",
            0, // 设备自有域
            "entry",
            &row.id,
            "upsert",
            &owner,
            &payload.to_string(),
        )
        .await
        .map(|_| ())
}

/// 记录一次 entry remove 变更。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn record_entry_remove(store: &Store, path: &str) -> Result<(), PartisyError> {
    let owner = store.device_id().await?;
    let payload = serde_json::json!({ "path": path });
    store
        .record_oplog(
            "default",
            0,
            "entry",
            path,
            "remove",
            &owner,
            &payload.to_string(),
        )
        .await
        .map(|_| ())
}
