//! iroh-blobs 通道执行器（ADR-0015）。
//!
//! Hub 侧 iroh/iroh-blobs 设备通道的跨层 trait 定义与执行器桩。
//!
//! ## 架构位置（ADR-0015 决策 1）
//!
//! ```text
//! device
//!   │  iroh QUIC 连接（打洞/中继）
//!   ▼
//! partisync-hub  (iroh 节点监听 + iroh-blobs 接收/发送)
//!   │  ChunkSink / ChunkSource trait（iroh 侧不感知具体类型）
//!   ▼
//! partisync-cas  (散块存储 / 聚合器 / pack v2)
//! ```
//!
//! ## 设计原则
//!
//! - `ChunkSink` / `ChunkSource` 放在 `partisync-transfer`（被 hub 依赖），
//!   而非 `partisync-cas`——避免 cas 引人 iroh 传递依赖（ADR-0015 裁定 1）；
//! - trait 不携带 iroh 特有类型（公钥/连接句柄），只传递 `Bytes` / `Hash`；
//! - `IrohBlobsExecutor` / `IrohBlobsListener` 执行器桩对调用方透明，
//!   真实 iroh 接线由 hub 层直接使用 `iroh =1.2.0` API 实现（hub 无 iroh-blobs 传递依赖）。
//!
//! ## 接线说明（M4 进场地填实）
//!
//! Hub 层（`iroh_channel.rs`）直接使用 `iroh::Endpoint` + `iroh_blobs::BlobsProtocol`
//! 实现真实的 send/accept 逻辑，绕过了本桩。接线时：
//! 1. hub 加 `iroh =1.2.0` + `iroh-blobs =0.103.0`（主 crate 独立，无 feature 冲突）
//! 2. `iroh_channel.rs` 中用 `Endpoint::bind(presets::N0)` 建节点
//! 3. `Router::builder(ep).accept(ALPN, BlobsProtocol::new(&store)).spawn()`
//! 4. `Endpoint::connect(peer, ALPN)` + `get::fsm` 状态机实现 download
//! 5. push 路径走 `handle_connection` → `store.import_bao_reader`

use std::fmt::Debug;

use bytes::Bytes;
use partisync_core::error::PartisyError;

/// Hub → CAS 的块接收 trait（hub 侧 iroh 接收会话调用，iroh 类型不穿透）。
///
/// 实现方为 `partisync-cas` 的 `ChunkStore` 写路径（散块写入 + 引用计数）。
///
/// # ADR-0015 裁定 3
/// 设备上行时 hub 的 iroh-blobs 接收会话每收到一个完整 chunk，
/// 调用一次 `put_chunk`。引用计数 incr 在 chunk 落 CAS 散块区时原子执行。
pub trait ChunkSink: Debug + Send + Sync {
    /// 接收一个 chunk（内容为 blake3 分块数据）。
    ///
    /// 实现方：写散块 + 原子 incr refcount。
    ///
    /// # Errors
    /// IO 错误或 refcount 操作失败 → `Severity::Fatal`（不允许半推）。
    fn put_chunk(
        &self,
        chunk: &[u8],
    ) -> impl std::future::Future<Output = Result<(), PartisyError>> + Send;

    /// 批量接收（可选优化路径——等价于逐条调用 `put_chunk`）。
    ///
    /// 默认实现逐条调用 `put_chunk`；覆盖此方法可批量刷盘减少 IO 调用。
    fn put_chunks(
        &self,
        chunks: &[&[u8]],
    ) -> impl std::future::Future<Output = Result<(), PartisyError>> + Send {
        async move {
            for chunk in chunks {
                self.put_chunk(chunk).await?;
            }
            Ok(())
        }
    }
}

/// Hub ← CAS 的块发送 trait（hub 侧 iroh-blobs 下发会话调用，iroh 类型不穿透）。
///
/// 实现方为 `partisync-cas` 的 `ChunkStore` 读路径（按 hash 读取块内容）。
///
/// # ADR-0015 裁定 4
/// 设备下行时 hub 从 CAS store 按 hash 读 chunk，然后通过 iroh-blobs 流式发送。
/// ChunkPlan 差分（`chunk_plan::plan_chunks`）决定需要下发哪些 hash，
/// sender 闭包内部调用本 trait 的 `get_chunk`。
pub trait ChunkSource: Debug + Send + Sync {
    /// 按 blake3 hash 取块内容。
    ///
    /// # Errors
    /// hash 未命中或 IO 错误 → `Severity::Fatal`（发送方应重试）。
    fn get_chunk(
        &self,
        hash: &str,
    ) -> impl std::future::Future<Output = Result<Bytes, PartisyError>> + Send;
}

/// iroh-blobs sender 桩执行器（ADR-0015 裁定 5）。
///
/// 真实实现在 hub 层 `iroh_channel.rs`：直接使用 `iroh::Endpoint` + `get::fsm` 状态机。
/// 本桩为编译占位，保持 API 签名兼容。
///
/// # 接线说明（M4 进场地填实）
///
/// ```ignore
/// // hub/iroh_channel.rs 中直接用 iroh 1.2.0 API：
/// use iroh::{Endpoint, endpoint::presets};
/// use iroh_blobs::{protocol::ALPN, get::fsm::*, protocol::GetRequest};
///
/// let endpoint = Endpoint::bind(presets::N0).await?;
/// let conn = endpoint.connect(peer_addr, ALPN).await?;
/// let request = GetRequest::blob(hash);
/// let state = fsm::start(conn, request, RequestCounters::default());
/// // ... 驱动状态机
/// ```
pub struct IrohBlobsExecutor<C: ChunkSource> {
    source: C,
    // TODO (M4): 替换为 Box<dyn IrohSendSession> — hub 层直接用 iroh::Endpoint
    // 而非经由本桩，以避免 iroh-blobs tokio feature 冲突（workspace tokio 缺 blocking）
    _p: std::marker::PhantomData<fn()>,
}

impl<C: ChunkSource> IrohBlobsExecutor<C> {
    /// 从 CAS ChunkSource 构造执行器。
    #[allow(dead_code)]
    pub async fn new(source: C, _endpoint: &str, _peer_addr: &str) -> Result<Self, PartisyError> {
        // M4 填实：hub 层直接用 iroh::Endpoint，绕过本桩
        Ok(Self {
            source,
            _p: std::marker::PhantomData,
        })
    }

    /// 通过 iroh-blobs 发送一个 chunk（桩）。
    #[allow(dead_code)]
    pub async fn send_chunk(&self, hash: &str) -> Result<(), PartisyError> {
        let _data = self.source.get_chunk(hash).await?;
        // M4: hub/iroh_channel.rs 直接用 endpoint.connect + fsm 状态机
        Ok(())
    }
}

/// iroh-blobs 接收会话桩：hub 侧接受设备上传并写入 CAS（ADR-0015 裁定 3）。
///
/// 真实实现在 hub 层 `iroh_channel.rs`：通过 `Router::accept(ALPN, BlobsProtocol)`
/// 分发连接，`handle_connection` 处理 push 写 CAS。
///
/// # 接线说明（M4 进场地填实）
///
/// ```ignore
/// // hub/iroh_channel.rs 中：
/// use iroh::{Endpoint, protocol::Router};
/// use iroh_blobs::{protocol::ALPN, BlobsProtocol};
///
/// let blobs = BlobsProtocol::new(&store, None);
/// let router = Router::builder(endpoint)
///     .accept(ALPN, blobs)
///     .spawn();
/// // Router 在后台 accept iroh-blobs 连接，写入 ChunkSink
/// ```
#[allow(dead_code)]
pub struct IrohBlobsListener<S: ChunkSink> {
    sink: S,
    // TODO (M4): 替换为 hub 层直接持有的 iroh::Endpoint 监听句柄
    _p: std::marker::PhantomData<fn()>,
}

impl<S: ChunkSink> IrohBlobsListener<S> {
    /// 构造监听器。
    #[allow(dead_code)]
    pub async fn new(_hub_addr: &str, sink: S) -> Result<Self, PartisyError> {
        // M4 填实：hub 层直接用 Router::builder(ep).accept(ALPN, BlobsProtocol).spawn()
        Ok(Self {
            sink,
            _p: std::marker::PhantomData,
        })
    }

    /// 接受一个设备上传会话（桩）。
    #[allow(dead_code)]
    pub async fn accept(&self) -> Result<UploadSummary, PartisyError> {
        // M4: hub/iroh_channel.rs 用 Router 在后台 accept，由 BlobsProtocol
        // 调用 ChunkSink::put_chunk，无需本桩参与 accept 循环
        Ok(UploadSummary {
            chunks_received: 0,
            bytes_received: 0,
        })
    }
}

/// 上传会话摘要。
#[derive(Debug, Clone)]
pub struct UploadSummary {
    pub chunks_received: u64,
    pub bytes_received: u64,
}

// ----------------------------------------------------------------------------
// 测试与桩实现（无 iroh 依赖的纯逻辑验证）
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use partisync_core::error::Severity;

    /// 内存内 ChunkSource 桩（测试用）。
    #[derive(Debug, Default)]
    pub struct InMemoryChunkSource {
        chunks: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Bytes>>>,
    }

    impl InMemoryChunkSource {
        pub fn insert(&self, hash: &str, data: impl Into<Bytes>) {
            self.chunks
                .lock()
                .unwrap()
                .insert(hash.to_string(), data.into());
        }
    }

    impl ChunkSource for InMemoryChunkSource {
        async fn get_chunk(&self, hash: &str) -> Result<Bytes, PartisyError> {
            self.chunks
                .lock()
                .unwrap()
                .get(hash)
                .cloned()
                .ok_or_else(|| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("chunk not found: {hash}").into()),
                })
        }
    }

    /// 内存内 ChunkSink 桩（测试用）。
    #[derive(Debug, Default)]
    pub struct InMemoryChunkSink {
        received: std::sync::Arc<std::sync::Mutex<Vec<Bytes>>>,
    }

    impl ChunkSink for InMemoryChunkSink {
        async fn put_chunk(&self, chunk: &[u8]) -> Result<(), PartisyError> {
            self.received
                .lock()
                .unwrap()
                .push(Bytes::copy_from_slice(chunk));
            Ok(())
        }
    }

    impl InMemoryChunkSink {
        pub fn chunks(&self) -> Vec<Bytes> {
            self.received.lock().unwrap().clone()
        }
    }

    #[tokio::test]
    async fn in_memory_source_provides_chunk() {
        let source = InMemoryChunkSource::default();
        source.insert("abc123", &b"hello world"[..]);
        let got = source.get_chunk("abc123").await.unwrap();
        assert_eq!(got.as_ref(), b"hello world");
    }

    #[tokio::test]
    async fn in_memory_source_missing_hash_returns_fatal() {
        let source = InMemoryChunkSource::default();
        let err = source.get_chunk("notexist").await.unwrap_err();
        assert_eq!(err.severity, Severity::Fatal);
    }

    #[tokio::test]
    async fn in_memory_sink_records_chunks() {
        let sink = InMemoryChunkSink::default();
        sink.put_chunk(b"chunk1").await.unwrap();
        sink.put_chunk(b"chunk2").await.unwrap();
        let chunks = sink.chunks();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].as_ref(), b"chunk1");
        assert_eq!(chunks[1].as_ref(), b"chunk2");
    }

    #[tokio::test]
    async fn executor_send_chunk_via_source() {
        let source = InMemoryChunkSource::default();
        source.insert("hash0", &b"data0"[..]);
        let executor = IrohBlobsExecutor::new(source, "mock://node", "mock://peer")
            .await
            .unwrap();
        executor.send_chunk("hash0").await.unwrap();
    }
}
