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
}

impl fmt::Display for AckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionClosed => write!(f, "连接已关，无法接收 ack"),
            Self::FrameFormat(s) => write!(f, "ack 帧格式错：{s}"),
            Self::Timeout { hash, timeout } => {
                write!(f, "等待 ack 超时（hash={hash:?}, timeout={timeout:?}）")
            }
        }
    }
}

impl std::error::Error for AckError {}

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
}