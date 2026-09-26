//! SQS 事件源（ADR-0021，feature `event-sqs`）：M5-WP04 `EventSource`
//! 契约的 AWS 实装。
//!
//! - `poll_batch`：ReceiveMessage（单次 ≤10 条，循环凑批 ≤ max / 截止
//!   deadline）→ 解析 S3 Event Notification JSON（Records[]）→
//!   [`map_s3_event_name`] 折叠到统一 EventKind（裁定 2）；
//! - `commit_cursor`：DeleteMessage（cursor = receipt handle，裁定 4
//!   不可重入语义由 SQS 承诺）；
//! - 空间归属：`space` 静态配置（S3 事件无内建空间语义——前缀映射归
//!   接线配置层）。
//!
//! 测试口径：kind 折叠与消息解析为纯函数单测；网络路径（真实 AWS）归
//! 集成阶段（凭证/区域由 `aws-config` 环境装配）。

use std::time::{Duration, Instant};

use partisync_core::error::PartisyError;

use crate::event::{EventError, EventKind, EventRecord, EventSource};

/// S3 事件名 → 统一 kind（M5-WP04 裁定 2）。
///
/// `ObjectCreated:*` → Created；`ObjectRemoved:*` → Removed；
/// `ObjectRestore/ObjectReplication` 等生命周期事件 → Modified（对象
/// 可见状态变化，幂等 upsert 安全）。None = 无法折叠（丢弃并计数——
/// 调用方记日志）。
#[must_use]
pub fn map_s3_event_name(event_name: &str) -> Option<EventKind> {
    if event_name.starts_with("ObjectCreated") {
        Some(EventKind::Created)
    } else if event_name.starts_with("ObjectRemoved") {
        Some(EventKind::Removed)
    } else if event_name.starts_with("Object") {
        Some(EventKind::Modified)
    } else {
        None
    }
}

/// S3 Event Notification 消息体 → 事件记录集（纯函数；cursor 沿用
/// SQS receipt handle——批内多条 Records 共享一个 handle，删除以
/// handle 为单位）。
#[must_use]
pub fn parse_s3_notification(space: &str, receipt_handle: &str, body: &str) -> Vec<EventRecord> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else {
        return Vec::new();
    };
    let Some(records) = v.get("Records").and_then(|r| r.as_array()) else {
        return Vec::new();
    };
    records
        .iter()
        .filter_map(|r| {
            let event_name = r.get("eventName")?.as_str()?;
            let kind = map_s3_event_name(event_name)?;
            let key = r.get("s3")?.get("object")?.get("key")?.as_str()?.to_owned();
            let size = r
                .get("s3")?
                .get("object")?
                .get("size")
                .and_then(|s| s.as_u64())
                .unwrap_or(0);
            // mtime 透传从 ISO8601 → unix ns 需 chrono（无新依赖＝不实装）；
            // 后端应用时由 graph::Store::add_entry 走 wall clock 兜底。
            Some(EventRecord {
                provider: "aws-s3".into(),
                space: space.to_owned(),
                path: key,
                kind,
                cursor: receipt_handle.to_owned(),
                payload: serde_json::json!({
                    "size": size,
                    "eventName": event_name,
                }),
            })
        })
        .collect()
}

/// SQS 事件源（ADR-0021）：持有客户端（可注入——测试/多队列）+ 队列 URL。
pub struct SqsEventSource {
    client: aws_sdk_sqs::Client,
    queue_url: String,
    /// 静态空间归属（本源全部事件记此空间）。
    space: String,
    /// 单次 ReceiveMessage 等待（长轮询秒数，0–20）。
    wait_seconds: i32,
}

impl SqsEventSource {
    /// 构造（client 由 `aws_config::load_from_env().await` 装配后注入）。
    #[must_use]
    pub fn new(client: aws_sdk_sqs::Client, queue_url: String, space: String) -> Self {
        Self {
            client,
            queue_url,
            space,
            wait_seconds: 2,
        }
    }

    /// 长轮询秒数调整（0 = 短轮询）。
    pub fn set_wait_seconds(&mut self, s: i32) {
        self.wait_seconds = s.clamp(0, 20);
    }
}

impl EventSource for SqsEventSource {
    async fn poll_batch(
        &self,
        max: usize,
        deadline: Duration,
    ) -> Result<Vec<EventRecord>, EventError> {
        let started = Instant::now();
        let mut out = Vec::new();
        let mut handles: Vec<(String, String)> = Vec::new(); // (cursor, body)
                                                             // 单次 ≤10 条（SQS 上限），循环凑批至 max / deadline
        while out.len() < max && started.elapsed() < deadline {
            let resp = self
                .client
                .receive_message()
                .queue_url(&self.queue_url)
                .max_number_of_messages(10.min((max - out.len().max(1)) as i32))
                .wait_time_seconds(self.wait_seconds)
                .send()
                .await
                .map_err(|e| EventError::Transport(format!("sqs receive: {e}")))?;
            let Some(messages) = resp.messages else {
                break; // 队列空（长轮询超时）
            };
            for m in &messages {
                let Some(handle) = m.receipt_handle() else {
                    continue;
                };
                let Some(body) = m.body() else {
                    continue;
                };
                let handle = handle.to_owned();
                let body = body.to_owned();
                out.extend(parse_s3_notification(&self.space, &handle, &body));
                handles.push((handle, body));
            }
            if messages.len() < 10 {
                break; // 队列已取空
            }
        }
        Ok(out)
    }

    async fn commit_cursor(&self, cursor: &str) -> Result<(), EventError> {
        self.client
            .delete_message()
            .queue_url(&self.queue_url)
            .receipt_handle(cursor)
            .send()
            .await
            .map_err(|e| EventError::Transport(format!("sqs delete: {e}")))?;
        Ok(())
    }

    fn name(&self) -> &str {
        "sqs"
    }
}

// PartisyError 兼容（EventError::from 已有）；占位引用避免未用告警
#[allow(dead_code)]
fn _err_shape(_: PartisyError) {}
