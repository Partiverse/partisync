//! 变更捕获（SPEC M2-WP01 契约 §2 / M2-WP02 契约 §4）：图谱变更 → oplog 行。
//!
//! 捕获口径（SPEC 风险节）：**索引场景不捕获**（全量索引即全量状态，对端首次
//! 同步拉全量）；仅 watch/journal 应用路径显式调用——漏广播窗口与 P8 重放收敛一致。
//!
//! 共享域（domain=1）：tag/entry_tag 变更在本地写图谱后调用 capture——oplog 键即
//! 本笔写入的 LWW 水位，capture 随即经 `apply_remote_tag*` 把水位回填到行上
//! （本端时钟严格单调 ⇒ 恒为 LWW 胜者）。

use partisync_core::error::{PartisyError, Severity};
use partisync_graph::store::Store;

fn fatal(what: impl Into<String>) -> PartisyError {
    PartisyError {
        severity: Severity::Fatal,
        source: Some(what.into().into()),
    }
}

/// 记录一次 entry upsert 变更（设备自有域，全量行 payload）。
///
/// origin = 本机 device（oplog 语义：谁广播本行）。不得用 `row.owner_device`
/// 兜底——属主列缺失（旧索引数据）会让 origin 变 unknown，对端回环防护失效
/// （M2 KPI 基准实测暴露的乒乓放大根因之一）。
///
/// # Errors
/// 条目不存在（此为 remove 误用）→ Fatal；device 未登记 / DB 错误 → Fatal。
pub async fn record_entry_upsert(store: &Store, path: &str) -> Result<(), PartisyError> {
    let row = store
        .entry_by_path(path)
        .await?
        .ok_or_else(|| fatal(format!("捕获目标不存在: {path}")))?;
    let origin = store.device_id().await?;
    let owner = row.owner_device.clone().unwrap_or_else(|| origin.clone());
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
            &origin,
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

/// 记录一次 tag upsert（共享域；name/color 读自落笔后的行）。
///
/// # Errors
/// tag 不存在 → Fatal；DB 错误 → Fatal。
pub async fn record_tag_upsert(store: &Store, tag_id: &str) -> Result<(), PartisyError> {
    let row = store
        .tag_by_id(tag_id)
        .await?
        .ok_or_else(|| fatal(format!("捕获目标不存在: tag {tag_id}")))?;
    let owner = store.device_id().await?;
    let payload = serde_json::json!({ "id": row.id, "name": row.name, "color": row.color });
    let key = store
        .record_oplog(
            "default",
            1,
            "tag",
            tag_id,
            "upsert",
            &owner,
            &payload.to_string(),
        )
        .await?;
    // 本笔写入 = LWW 水位（本端时钟单调 ⇒ 必胜）
    store
        .apply_remote_tag(tag_id, &row.name, row.color.as_deref(), false, &key)
        .await?;
    Ok(())
}

/// 记录一次 tag 删除（墓碑）。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn record_tag_remove(store: &Store, tag_id: &str) -> Result<(), PartisyError> {
    let owner = store.device_id().await?;
    let payload = serde_json::json!({ "id": tag_id });
    let key = store
        .record_oplog(
            "default",
            1,
            "tag",
            tag_id,
            "remove",
            &owner,
            &payload.to_string(),
        )
        .await?;
    store.apply_remote_tag(tag_id, "", None, true, &key).await?;
    Ok(())
}

/// 记录一次打标签（共享域链接；entry 以 path 为跨节点身份）。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn record_tag_link(
    store: &Store,
    tag_id: &str,
    entry_path: &str,
) -> Result<(), PartisyError> {
    let owner = store.device_id().await?;
    let payload = serde_json::json!({ "tag_id": tag_id, "entry_path": entry_path });
    let entity_id = format!("{tag_id}|{entry_path}");
    let key = store
        .record_oplog(
            "default",
            1,
            "entry_tag",
            &entity_id,
            "link",
            &owner,
            &payload.to_string(),
        )
        .await?;
    store
        .apply_remote_tag_link(tag_id, entry_path, false, &key)
        .await?;
    Ok(())
}

/// 记录一次摘标签（共享域链接墓碑）。
///
/// # Errors
/// DB 错误 → Fatal。
pub async fn record_tag_unlink(
    store: &Store,
    tag_id: &str,
    entry_path: &str,
) -> Result<(), PartisyError> {
    let owner = store.device_id().await?;
    let payload = serde_json::json!({ "tag_id": tag_id, "entry_path": entry_path });
    let entity_id = format!("{tag_id}|{entry_path}");
    let key = store
        .record_oplog(
            "default",
            1,
            "entry_tag",
            &entity_id,
            "unlink",
            &owner,
            &payload.to_string(),
        )
        .await?;
    store
        .apply_remote_tag_link(tag_id, entry_path, true, &key)
        .await?;
    Ok(())
}
