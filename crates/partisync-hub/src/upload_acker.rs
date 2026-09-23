//! Device 侧 UploadAck 接收器（M5-WP01-T02）
//!
//! 设备 push blob 后，**必须**在本连接上读 hub 回发的 UploadAck 帧，
//! 收到后才允许关闭连接（关闭前未收到 ack → 数据可能未落库）。
//!
//! 协议契约：`docs/specs/M5-WP00.md` §3 + `iroh_channel.rs::send_upload_acks`
//!
//! ## 使用方式（设备侧）
//! ```ignore
//! let acker = UploadAcker::spawn(connection);
//! for blob in blobs {
//!     push_via_iroh_blobs(...).await?;
//!     acker.expect_ack(blob.hash, Duration::from_secs(30)).await?;
//! }
//! acker.close();
//! ```

use std::fmt;
use std::time::Duration;

use iroh::endpoint::Connection;
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::iroh_channel::{UPLOAD_ACK_FRAME_LEN, UPLOAD_ACK_TYPE, UPLOAD_ACK_VERSION, UploadAckStatus};

/// UploadAck 接收错误。
#[derive(Debug)]
pub enum AckError {
    ConnectionClosed,
    FrameFormat(&'static str),
    Timeout {
        hash: iroh_blobs::Hash,
        timeout: Duration,
    },
    Rejected {
        hash: iroh_blobs::Hash,
    },
    ExhaustedRetries {
        hash: iroh_blobs::Hash,
        attempts: u32,
    },
}

impl fmt::Display for AckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionClosed => write!(f, "连接已关，无法接收 ack"),
            Self::FrameFormat(s) => write!(f, "ack 帧格式错：{s}"),
            Self::Timeout { hash, timeout } => {
                write!(f, "等待 ack 超时（hash={hash:?}, timeout={timeout:?}）")
            }
            Self::Rejected { hash } => {
                write!(f, "hub 拒绝接收该块（hash={hash:?}）")
            }
            Self::ExhaustedRetries { hash, attempts } => {
                write!(
                    f,
                    "超出重试上限仍未收到成功 ack（hash={hash:?}, attempts={attempts}）"
                )
            }
        }
    }
}

impl std::error::Error for AckError {}

/// 指数退避重试策略（M5-WP01-T05）。
///
/// 无外部依赖实现，支持可配置初始退避时间、最大重试次数及单次等待超时。
#[derive(Debug, Clone, Copy)]
pub struct UploadRetryPolicy {
    pub initial_backoff: Duration,
    pub max_retries: u32,
    pub ack_timeout: Duration,
}

impl Default for UploadRetryPolicy {
    fn default() -> Self {
        Self {
            initial_backoff: Duration::from_millis(100),
            max_retries: 5,
            ack_timeout: Duration::from_secs(30),
        }
    }
}

impl UploadRetryPolicy {
    /// 计算第 `attempt` 次（从 0 开始）失败后的退避时长：`initial_backoff * 2^attempt`。
    #[must_use]
    pub fn backoff_for(&self, attempt: u32) -> Duration {
        let shift = attempt.min(16);
        self.initial_backoff.saturating_mul(1u32 << shift)
    }
}

/// 单帧 UploadAck 帧解码结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedAck {
    pub hash: iroh_blobs::Hash,
    pub status: UploadAckStatus,
}

/// Device 端 UploadAck 接收器：后台 task 从 iroh 连接读 ack 帧，
/// 推到 channel，前端按 hash 查询。
pub struct UploadAcker {
    rx: mpsc::Receiver<DecodedAck>,
    conn: Connection,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl UploadAcker {
    /// 监听一条已建立 iroh 连接上的 UploadAck 帧序列。
    ///
    /// 后台 spawn 一个 task 循环 `accept_uni()` + 读 35B 帧 + 推 channel。
    /// 调用方通过 [`expect_ack`](Self::expect_ack) 同步等待指定 hash 的 ack。
    #[must_use]
    pub fn spawn(conn: Connection) -> Self {
        let (tx, rx) = mpsc::channel::<DecodedAck>(64);
        let task_conn = conn.clone();
        let task = tokio::spawn(async move {
            loop {
                let mut recv = match task_conn.accept_uni().await {
                    Ok(r) => r,
                    Err(_) => return, // connection closed
                };
                // 连续读帧直到对端 finish
                let mut buf = vec![0u8; 4096];
                loop {
                    match recv.read(&mut buf).await {
                        Ok(Some(n)) => {
                            let mut i = 0;
                            while i + UPLOAD_ACK_FRAME_LEN <= n {
                                match decode_ack_frame(&buf[i..i + UPLOAD_ACK_FRAME_LEN]) {
                                    Ok(ack) => {
                                        if tx.send(ack).await.is_err() {
                                            return;
                                        }
                                        i += UPLOAD_ACK_FRAME_LEN;
                                    }
                                    Err(_) => i += 1,
                                }
                            }
                        }
                        Ok(None) => break, // stream finished
                        Err(_) => return,
                    }
                }
            }
        });
        Self {
            rx,
            conn,
            task: Some(task),
        }
    }

    /// 同步等待指定 hash 的 ack。
    pub async fn expect_ack(
        &mut self,
        hash: iroh_blobs::Hash,
        timeout_dur: Duration,
    ) -> Result<UploadAckStatus, AckError> {
        match timeout(timeout_dur, async {
            while let Some(ack) = self.rx.recv().await {
                if ack.hash == hash {
                    return Ok(ack.status);
                }
            }
            Err(AckError::ConnectionClosed)
        })
        .await
        {
            Ok(result) => result,
            Err(_) => Err(AckError::Timeout {
                hash,
                timeout: timeout_dur,
            }),
        }
    }

    /// 带指数退避重试的可靠上传确认循环（M5-WP01-T05）。
    ///
    /// 调用方提供 `push_fn` 闭包执行单次 push 操作：
    /// - 若收到 `Accepted` 或 `Duplicate`：立即返回 `Ok(status)`；
    /// - 若收到 `Rejected`：不可重试，立即返回 `Err(AckError::Rejected)`；
    /// - 若收到 `Retrying` 或 `expect_ack` 超时：按 `policy.backoff_for(attempt)` 退避后重新调用 `push_fn`；
    /// - 超过 `policy.max_retries` 仍未成功：返回 `Err(AckError::ExhaustedRetries)`。
    pub async fn push_with_retry<F, Fut>(
        &mut self,
        hash: iroh_blobs::Hash,
        policy: UploadRetryPolicy,
        mut push_fn: F,
    ) -> Result<UploadAckStatus, AckError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<(), AckError>>,
    {
        for attempt in 0..=policy.max_retries {
            push_fn().await?;
            match self.expect_ack(hash, policy.ack_timeout).await {
                Ok(
                    status @ (UploadAckStatus::Accepted | UploadAckStatus::Duplicate),
                ) => return Ok(status),
                Ok(UploadAckStatus::Rejected) => return Err(AckError::Rejected { hash }),
                Ok(UploadAckStatus::Retrying) | Err(AckError::Timeout { .. }) => {
                    if attempt < policy.max_retries {
                        tokio::time::sleep(policy.backoff_for(attempt)).await;
                    }
                }
                Err(e) => return Err(e),
            }
        }
        Err(AckError::ExhaustedRetries {
            hash,
            attempts: policy.max_retries + 1,
        })
    }

    /// 关闭接收器（abort 后台 task + 关闭连接）。
    pub fn close(mut self) {
        if let Some(t) = self.task.take() {
            t.abort();
        }
        self.conn.close(0u32.into(), b"upload_acker closed");
    }
}

/// 解码单个 UploadAck 帧（35B）。
fn decode_ack_frame(buf: &[u8]) -> Result<DecodedAck, &'static str> {
    if buf.len() != UPLOAD_ACK_FRAME_LEN {
        return Err("长度错");
    }
    if buf[0] != UPLOAD_ACK_VERSION {
        return Err("版本不匹配");
    }
    if buf[1] != UPLOAD_ACK_TYPE {
        return Err("类型不匹配");
    }
    let hash_bytes: [u8; 32] = buf[2..34]
        .try_into()
        .map_err(|_| "hash 字段错")?;
    let hash = iroh_blobs::Hash::from_bytes(hash_bytes);
    let status = match buf[34] {
        0 => UploadAckStatus::Accepted,
        1 => UploadAckStatus::Duplicate,
        2 => UploadAckStatus::Rejected,
        3 => UploadAckStatus::Retrying,
        _ => return Err("status 未知"),
    };
    Ok(DecodedAck { hash, status })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_ack_frame_basic() {
        let hash = iroh_blobs::Hash::from_bytes([42u8; 32]);
        let mut buf = [0u8; 35];
        buf[0] = UPLOAD_ACK_VERSION;
        buf[1] = UPLOAD_ACK_TYPE;
        buf[2..34].copy_from_slice(hash.as_bytes());
        buf[34] = UploadAckStatus::Accepted as u8;
        let ack = decode_ack_frame(&buf).unwrap();
        assert_eq!(ack.hash, hash);
        assert_eq!(ack.status, UploadAckStatus::Accepted);
    }

    #[test]
    fn decode_ack_frame_wrong_version() {
        let mut buf = [0u8; 35];
        buf[0] = 0x99;
        buf[1] = UPLOAD_ACK_TYPE;
        assert!(decode_ack_frame(&buf).is_err());
    }

    #[test]
    fn decode_ack_frame_wrong_type() {
        let mut buf = [0u8; 35];
        buf[0] = UPLOAD_ACK_VERSION;
        buf[1] = 0x99;
        assert!(decode_ack_frame(&buf).is_err());
    }

    #[test]
    fn decode_ack_frame_wrong_size() {
        let buf = [0u8; 10];
        assert!(decode_ack_frame(&buf).is_err());
    }

    #[test]
    fn decode_ack_frame_unknown_status() {
        let mut buf = [0u8; 35];
        buf[0] = UPLOAD_ACK_VERSION;
        buf[1] = UPLOAD_ACK_TYPE;
        buf[34] = 99;
        assert!(decode_ack_frame(&buf).is_err());
    }

    #[test]
    fn retry_policy_exponential_backoff() {
        let policy = UploadRetryPolicy {
            initial_backoff: Duration::from_millis(100),
            max_retries: 4,
            ack_timeout: Duration::from_secs(5),
        };
        assert_eq!(policy.backoff_for(0), Duration::from_millis(100));
        assert_eq!(policy.backoff_for(1), Duration::from_millis(200));
        assert_eq!(policy.backoff_for(2), Duration::from_millis(400));
        assert_eq!(policy.backoff_for(3), Duration::from_millis(800));
    }
}