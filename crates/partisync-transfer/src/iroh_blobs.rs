//! iroh-blobs 通道执行器（ADR-0015）。
//!
//! Hub 侧 iroh/iroh-blobs 设备通道的跨层 trait 定义与执行器实现。
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
//! ## trait 设计原则
//!
//! - `ChunkSink` / `ChunkSource` 放在 `partisync-transfer`（被 hub 依赖），
//!   而非 `partisync-cas`——避免 cas 引人 iroh 传递依赖（ADR-0015 裁定 1）；
//! - trait 不携带 iroh 特有类型（公钥/连接句柄），只传递 `Bytes` / `Hash`；
//! - 具体 iroh-blobs 绑定在执行器内部（`IrohBlobsExecutor`），对调用方透明。

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

/// iroh-blobs sender 执行器：将 `chunk_plan::execute_plan` 的 sender 闭包
/// 绑定到 iroh-blobs 流式发送（ADR-0015 裁定 5）。
///
/// ```ignore
/// let plan = plan_chunks(("src_root", &src_hashes), ("dst_root", &dst_hashes));
/// let executor = IrohBlobsExecutor::new(cas_source, iroh_connection);
/// let stats = execute_plan(&plan, |hash| executor.send_chunk(hash)).await?;
/// ```
///
/// ## 执行流程（ADR-0015 裁定 4）
///
/// 1. 对 plan 中每个 `ChunkRole::Need` 的 hash，调用 `get_chunk(hash)` 取内容；
/// 2. 通过 iroh-blobs `write_blob` 流式发送（BLAKE3 窗口级验证）；
/// 3. 设备验签失败则返回 `PartisyError`（应用层重试，不走 iroh 重传）。
///
/// ## 限速
///
/// 发送速率受 `IrohBlobsExecutor` 内部限速器约束（与 S3/MPU 共调度器配额，
/// ADR-0009 裁定 5）。
///
/// # ADR-0015 裁定 5
/// `IrohBlobsExecutor` 位于 `partisync-transfer`，通过 trait object
/// 间接调用 iroh（不在此 trait 中可见）。
pub struct IrohBlobsExecutor<C: ChunkSource> {
    source: C,
    // iroh-blobs sender 句柄（内部类型，iroh crate 特有）
    // 字段类型在 iroh 版本对齐后填实（见 ADR-0015 待确认项）
    _p: std::marker::PhantomData<fn()>,
}

impl<C: ChunkSource> IrohBlobsExecutor<C> {
    /// 从 CAS ChunkSource 构造执行器。
    ///
    /// # Errors
    /// iroh 连接建立失败。
    #[allow(dead_code)]
    pub async fn new(source: C, _iroh_node: &str) -> Result<Self, PartisyError>
    where
        C: ChunkSource,
    {
        // TODO: ADR-0015 进场的版本对齐阻塞——iroh 1.2.0 与 iroh-blobs 0.103.0
        //   版本族不对齐（ADR 说"同 minor"，但两 crate 版本体系独立）。
        //   进场后填实：选择兼容的 iroh + iroh-blobs 版本组合，
        //   建立 iroh::Node 连接，初始化 iroh_blobs::protocol::write_blob session。
        eprintln!("IrohBlobsExecutor::new (iroh 版本待对齐)");
        Ok(Self {
            source,
            _p: std::marker::PhantomData,
        })
    }

    /// 通过 iroh-blobs 发送一个 chunk。
    ///
    /// # Errors
    /// iroh 发送失败或 chunk 不存在（Severity::Fatal，应用层重试）。
    #[allow(dead_code)]
    pub async fn send_chunk(&self, hash: &str) -> Result<(), PartisyError> {
        let data = self.source.get_chunk(hash).await?;
        // TODO: iroh_blobs::protocol::write_blob(&self.writer, data).await
        eprintln!(
            "send_chunk (iroh-blobs 待接线): hash={hash} size={}",
            data.len()
        );
        Ok(())
    }
}

/// iroh-blobs 接收会话：hub 侧接受设备上传并写入 CAS。
///
/// ```ignore
/// let listener = IrohBlobsListener::new(hub_addr, cas_sink).await?;
/// listener.accept().await?; // 阻塞直到设备连接并完成上传
/// ```
///
/// # ADR-0015 裁定 3
/// Hub 侧的 iroh-blobs 接收会话把每接收到的 chunk 调用 `ChunkSink::put_chunk`。
/// BLAKE3 root 验签通过后发送应用层 ACK（UploadAck）。
#[allow(dead_code)]
pub struct IrohBlobsListener<S: ChunkSink> {
    sink: S,
    // TODO: iroh 监听句柄（版本对齐后填实）
    _p: std::marker::PhantomData<fn()>,
}

impl<S: ChunkSink> IrohBlobsListener<S> {
    /// 构造监听器。
    ///
    /// # Errors
    /// iroh 节点初始化失败。
    #[allow(dead_code)]
    pub async fn new(_hub_addr: &str, sink: S) -> Result<Self, PartisyError> {
        // TODO: iroh::Node::spawn + iroh_blobs::protocol::accept_and_read_blob
        //   绑定 addr，开始监听设备连接（hub 作为 ALWAYS reachable relay endpoint，
        //   ADR-0015 裁定 2）
        eprintln!("IrohBlobsListener::new (iroh 版本待对齐)");
        Ok(Self {
            sink,
            _p: std::marker::PhantomData,
        })
    }

    /// 接受一个设备上传会话（阻塞直到会话完成或出错）。
    ///
    /// # Errors
    /// iroh 连接错误或 BLAKE3 验签失败。
    #[allow(dead_code)]
    pub async fn accept(&self) -> Result<UploadSummary, PartisyError> {
        // TODO: iroh_blobs::protocol::accept_and_read_blob(session, |chunk_bytes| {
        //     self.sink.put_chunk(chunk_bytes)
        // }).await
        // 验签通过后构造 UploadSummary 返回
        eprintln!("IrohBlobsListener::accept (iroh 版本待接线)");
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
    use bytes::Bytes;
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
        let executor = IrohBlobsExecutor::new(source, "mock://node").await.unwrap();
        executor.send_chunk("hash0").await.unwrap();
        // 验证 send_chunk 调用了 source.get_chunk（内部打印 trace 即验证路径）
    }
}
