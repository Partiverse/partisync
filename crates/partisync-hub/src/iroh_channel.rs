//! Hub 侧 iroh/ihor-blobs 设备通道（ADR-0015 / M4-WP04-T02）。
//!
//! Hub 作为 ALWAYS reachable relay endpoint，监听设备连接，通过 iroh-blobs
//! 接收设备上行数据（写 CAS）和下发数据（从 CAS 读）。

use std::sync::Arc;

use iroh::endpoint::presets;
use iroh_blobs::protocol::ALPN;
use iroh_blobs::provider::{handle_stream, StreamPair};
use iroh_blobs::{protocol::GetRequest, Hash};
use tracing::{debug, info, warn};

// `EventSender::DEFAULT` is in a private module; re-export for internal use
use iroh_blobs::provider::events as iroh_events;

use partisync_core::error::{PartisyError, Severity};
use partisync_transfer::iroh_blobs::{ChunkSink, ChunkSource, UploadSummary};

/// Hub 侧 iroh 节点门面（ADR-0015 裁定 2 / M4-WP04-T02 接线，T05 完成 CAS 打通）。
///
/// 包装 `iroh::Endpoint`，持有 CAS `ChunkSink`/`ChunkSource` 与
/// `HubIrohKeyspace`，提供：
/// - 从 `HubIrohKeyspace` 加载/初始化节点密钥（[`IrohHubNode::new`]）；
/// - 设备连接 accept 循环：设备 push blob → MemStore 暂存 → blake3 校验 →
///   CAS `ChunkSink`（[`IrohHubNode::run`] / [`handle_incoming`]）；
/// - 下行执行器：CAS `ChunkSource` → 直连设备 → iroh-blobs get::fsm
///   状态机（[`IrohHubNode::executor`] / `IrohBlobsExecutor`）。
#[allow(dead_code)]
pub struct IrohHubNode<S, C> {
    endpoint: iroh::Endpoint,
    sink: S,
    source: C,
    keyspace: Arc<dyn HubIrohKeyspace>,
}

impl<S: ChunkSink + Clone + Send + 'static, C: ChunkSource + Clone + Send + 'static>
    IrohHubNode<S, C>
{
    /// 从 CAS sink/source + HubIrohKeyspace 构造 iroh 节点（ADR-0015 裁定 2）。
    ///
    /// # Errors
    /// iroh Endpoint 初始化失败或 keyspace 读写失败。
    pub async fn new(
        sink: S,
        source: C,
        keyspace: Arc<dyn HubIrohKeyspace>,
    ) -> Result<Self, PartisyError> {
        let secret_key = match keyspace.get_node_sk()? {
            Some(sk_b64) => {
                let bytes = base64_decode(&sk_b64).map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("keyspace decode: {e}").into()),
                })?;
                if bytes.len() != 32 {
                    return Err(PartisyError {
                        severity: Severity::Fatal,
                        source: Some("invalid secret key length".into()),
                    });
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                iroh::SecretKey::from_bytes(&arr)
            }
            None => {
                let sk = iroh::SecretKey::generate();
                let sk_b64 = base64_encode(&sk.to_bytes());
                keyspace.put_node_sk(&sk_b64).map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("keyspace write: {e}").into()),
                })?;
                info!("Generated new iroh node ID: {}", sk.public());
                sk
            }
        };

        let endpoint = iroh::Endpoint::builder(presets::N0)
            .secret_key(secret_key)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await
            .map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("endpoint bind: {e}").into()),
            })?;

        endpoint.online().await;
        info!("IrohHubNode started: id={}", endpoint.id());

        Ok(Self {
            endpoint,
            sink,
            source,
            keyspace,
        })
    }

    /// 启动节点：accept 循环处理设备上传（push → MemStore → CAS）。
    ///
    /// # Errors
    /// iroh 运行错误。
    pub async fn run(self) -> Result<(), PartisyError> {
        info!("IrohHubNode running: id={}", self.endpoint.id());

        loop {
            match self.endpoint.accept().await {
                Some(incoming) => {
                    let sink = self.sink.clone();
                    tokio::spawn(async move {
                        match handle_incoming(incoming, sink).await {
                            Ok(summary) => debug!(
                                chunks = summary.chunks_received,
                                bytes = summary.bytes_received,
                                "upload session synced to CAS"
                            ),
                            Err(e) => warn!("incoming connection error: {}", e),
                        }
                    });
                }
                None => {
                    info!("endpoint closed");
                    break;
                }
            }
        }

        Ok(())
    }

    /// 构造下行执行器（hub → 设备），复用节点 endpoint 与 CAS source
    /// （M4-WP04-T05）。
    pub fn executor(&self) -> IrohBlobsExecutor<C> {
        IrohBlobsExecutor::new(self.source.clone(), self.endpoint.clone())
    }

    /// 返回本 hub 的 iroh 节点地址（debug string）。
    ///
    /// # Errors
    /// iroh 节点未初始化。
    pub fn node_addr(&self) -> String {
        let addr = self.endpoint.addr();
        // EndpointAddr has no Display; build string from components
        let id = format!("{:?}", addr.id);
        let relays: Vec<String> = addr.relay_urls().map(|r| r.to_string()).collect();
        if relays.is_empty() {
            id
        } else {
            format!("{} relay={}", id, relays.join(","))
        }
    }

    /// 返回本 hub 的 iroh 节点公钥。
    pub fn node_id(&self) -> iroh::PublicKey {
        self.endpoint.id()
    }

    /// 返回 iroh Endpoint（供 download 执行器使用）。
    pub fn endpoint(&self) -> &iroh::Endpoint {
        &self.endpoint
    }
}

/// 处理单个设备连接：逐流接收 push blob，每条流完成后增量同步进 CAS。
///
/// 同步语义（M4-WP04-T05）：对每条流处理完（`handle_stream` 返回 + store
/// idle）后新出现的 blob 做 blake3 完整性校验（与 iroh-blobs `Hash` 同口径），
/// 再经 [`ChunkSink::put_chunk`] 原子入 CAS（新块写对象 + refcount=1，已存在
/// refcount+1）；校验失败的 blob 跳过并告警，不允许半块/坏块入库。返回
/// [`UploadSummary`] 供上层审计。
///
/// 连接契约（SPEC M4-WP04 §2 UploadAck）：iroh-blobs 0.103 的 push 是
/// fire-and-forget（设备 `send.finish()` 即返回，不读 hub 应答），若设备
/// push 后立即关连接，hub 可能还没 accept 该流，数据随连接丢弃。因此设备侧
/// 必须保持连接直到 hub 同步完成（协议层的 UploadAck，M5 设备侧执行器落地）。
pub async fn handle_incoming<S: ChunkSink + Clone + Send + 'static>(
    incoming: iroh::endpoint::Incoming,
    sink: S,
) -> Result<UploadSummary, PartisyError> {
    let accepting = incoming.accept().map_err(|e| PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("accept: {e}").into()),
    })?;

    let connection = accepting.await.map_err(|e| PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("handshake: {e}").into()),
    })?;

    debug!(
        "new iroh connection from: endpoint_id={}",
        connection.remote_id()
    );

    let mem_store = iroh_blobs::store::mem::MemStore::default();
    let store: iroh_blobs::api::Store = mem_store.into();
    let events = iroh_events::EventSender::DEFAULT;
    let mut summary = UploadSummary {
        chunks_received: 0,
        bytes_received: 0,
    };
    let mut seen = std::collections::HashSet::new();

    // 逐流**内联**处理（不等价于 `handle_connection`：其 spawn-and-forget
    // 在连接立即关闭的竞态下无法保证流已收敛）：每条流完成后等 store Actor
    // 合并 import 结果，再增量同步新 blob 进 CAS。
    //
    // **M5-WP01 UploadAck**：每轮 sync 后向设备发 ack 帧序列，让 device 端能关闭
    // 连接前知道哪些 blob 已落库（消除 fire-and-forget 数据丢失风险）。
    while let Ok(pair) = StreamPair::accept(&connection, events.clone()).await {
        handle_stream(pair, store.clone())
            .await
            .map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("handle stream: {e}").into()),
            })?;
        store.wait_idle().await.map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("wait idle: {e}").into()),
        })?;
        let (part, new_hashes) = sync_new_blobs(&store, &sink, &mut seen).await?;
        add_summary(&mut summary, part);
        send_upload_acks(&connection, &new_hashes).await;
    }

    // 连接关闭后的兜底同步（流水线化尾部流）
    store.wait_idle().await.map_err(|e| PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("wait idle: {e}").into()),
    })?;
    let (part, new_hashes) = sync_new_blobs(&store, &sink, &mut seen).await?;
    add_summary(&mut summary, part);
    // 此处连接大概率已关，send_upload_acks 会 warn 但不致命
    send_upload_acks(&connection, &new_hashes).await;

    info!(
        chunks = summary.chunks_received,
        bytes = summary.bytes_received,
        "connection blobs synced to CAS"
    );
    Ok(summary)
}

/// 把 MemStore 中尚未同步过的 blob 校验后写入 CAS，返回本轮同步摘要 + 新同步 hash 列表。
///
/// **M5-WP01 UploadAck**：返回 `Vec<Hash>` 让 [`handle_incoming`] 在同一连接上
/// 写 ack 帧。device 侧 [`super::upload_acker`] 消费这些帧。
async fn sync_new_blobs<S: ChunkSink>(
    store: &iroh_blobs::api::Store,
    sink: &S,
    seen: &mut std::collections::HashSet<Hash>,
) -> Result<(UploadSummary, Vec<Hash>), PartisyError> {
    let mut summary = UploadSummary {
        chunks_received: 0,
        bytes_received: 0,
    };
    let mut new_hashes = Vec::new();
    let hashes = store
        .blobs()
        .list()
        .hashes()
        .await
        .map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("list blobs: {e}").into()),
        })?;
    for hash in hashes {
        if !seen.insert(hash) {
            continue;
        }
        let data = store
            .blobs()
            .get_bytes(hash)
            .await
            .map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("export blob {hash}: {e}").into()),
            })?;
        if Hash::from(blake3::hash(&data)) != hash {
            warn!(%hash, size = data.len(), "blob blake3 校验失败，跳过同步");
            continue;
        }
        sink.put_chunk(&data).await?;
        summary.bytes_received = summary
            .bytes_received
            .saturating_add(u64::try_from(data.len()).unwrap_or(u64::MAX));
        summary.chunks_received += 1;
        new_hashes.push(hash);
        debug!(%hash, size = data.len(), "blob synced to CAS");
    }
    Ok((summary, new_hashes))
}

fn add_summary(total: &mut UploadSummary, part: UploadSummary) {
    total.chunks_received += part.chunks_received;
    total.bytes_received = total.bytes_received.saturating_add(part.bytes_received);
}

// ─── UploadAck 帧（M5-WP01-T01） ──────────────────────────────────────────────
//
// 帧格式（v0x01）：
//   [1B version=0x01]
//   [1B type=0x02 UploadAck]
//   [32B hash]      // blake3 hash of the blob that was accepted
//   [1B status]     // 0=Accepted / 1=Duplicate / 2=Rejected / 3=Retrying
// 总长度 35 B（每个 blob 一帧）。
//
// 走独立的 QUIC unidirectional stream（不与 iroh-blobs 流混用），保证不破坏
// 既有协议；版本协商在 connection open 时由 UploadHello 帧完成（M5-WP01 后续 T 扩展）。

/// UploadAck 状态码（与 SPEC M5-WP00 §3.2.1 一致）。
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadAckStatus {
    Accepted = 0,
    Duplicate = 1,
    Rejected = 2,
    Retrying = 3,
}

pub const UPLOAD_ACK_VERSION: u8 = 0x01;
pub const UPLOAD_ACK_TYPE: u8 = 0x02;
pub const UPLOAD_ACK_FRAME_LEN: usize = 1 + 1 + 32 + 1; // version + type + hash + status = 35B

/// 序列化单个 UploadAck 帧到字节缓冲。
fn encode_upload_ack(hash: &Hash, status: UploadAckStatus) -> [u8; UPLOAD_ACK_FRAME_LEN] {
    let mut buf = [0u8; UPLOAD_ACK_FRAME_LEN];
    buf[0] = UPLOAD_ACK_VERSION;
    buf[1] = UPLOAD_ACK_TYPE;
    buf[2..34].copy_from_slice(hash.as_bytes());
    buf[34] = status as u8;
    buf
}

/// 在同一 QUIC 连接上向设备写 UploadAck 帧序列（每个 hash 一帧）。
///
/// **不阻塞** iroh-blobs 流处理：失败仅 warn（device 端靠 30s 超时重传兜底）。
async fn send_upload_acks(connection: &iroh::endpoint::Connection, hashes: &[Hash]) {
    if hashes.is_empty() {
        return;
    }
    let payload_len = hashes.len() * UPLOAD_ACK_FRAME_LEN;
    match connection.open_uni().await {
        Ok(mut send_stream) => {
            for hash in hashes {
                let frame = encode_upload_ack(hash, UploadAckStatus::Accepted);
                if let Err(e) = send_stream.write_all(&frame).await {
                    warn!(?e, "UploadAck 写帧失败（device 端会重传）");
                    return;
                }
            }
            // SendStream::finish 在 iroh 0.x 是同步签名（返回 Result<(), ClosedStream>）。
            if let Err(e) = send_stream.finish() {
                warn!(?e, "UploadAck stream finish 失败");
            } else {
                info!(acks = hashes.len(), bytes = payload_len, "UploadAck 已发往设备");
            }
        }
        Err(e) => {
            warn!(?e, "UploadAck 打开 uni 流失败（连接可能已关）");
        }
    }
}

// ----------------------------------------------------------------------------
// Download 执行器：hub → 设备（通过 iroh-blobs get::fsm 状态机）
// ----------------------------------------------------------------------------

/// iroh-blobs download 执行器（ADR-0015 裁定 4 / M4-WP04-T02）。
pub struct IrohBlobsExecutor<C: ChunkSource> {
    source: C,
    endpoint: iroh::Endpoint,
}

impl<C: ChunkSource> IrohBlobsExecutor<C> {
    /// 从 CAS ChunkSource + iroh Endpoint 构造执行器。
    pub fn new(source: C, endpoint: iroh::Endpoint) -> Self {
        Self { source, endpoint }
    }

    /// 通过 iroh-blobs get::fsm 状态机向设备发送一个 chunk。
    ///
    /// # Errors
    /// 连接失败、hash 不存在或 iroh 协议错误。
    pub async fn send_chunk(
        &self,
        peer_addr: &iroh_base::EndpointAddr,
        hash: &str,
    ) -> Result<(), PartisyError> {
        use iroh_blobs::get::fsm;

        // 1. 从 CAS 取块内容
        let data = self.source.get_chunk(hash).await?;
        let hash_bytes: [u8; 32] = blake3::hash(&data).into();

        // 2. 建立到设备的 iroh-blobs 连接
        // Endpoint::connect takes impl Into<EndpointAddr>; use .clone()
        let conn = self
            .endpoint
            .connect(peer_addr.clone(), ALPN)
            .await
            .map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("connect to {:?}: {e}", peer_addr).into()),
            })?;

        // 3. 构造 get 请求（单 blob）
        let h = Hash::from(hash_bytes);
        let request = GetRequest::blob(h);
        let counters = fsm::RequestCounters::default();

        // 4. 驱动 fsm 状态机直到 blob 发送完成
        // 状态机：
        //   AtInitial → (await) → AtConnected
        //   AtConnected → (await) → ConnectedNext (enum: StartRoot | StartChild | Closing)
        //   StartRoot → (sync next()) → AtBlobHeader
        //   AtBlobHeader → (await) → (AtBlobContent, u64)
        //   AtBlobContent → (await write_all) → AtEndBlob
        //   AtEndBlob → (sync next()) → EndBlobNext (enum: MoreChildren | Closing)
        //   StartChild → (sync next()) → AtBlobHeader
        //   AtClosing → (await) → Stats
        //
        // Step 4a: open bidirectional stream
        let connected = fsm::start(conn, request, counters)
            .next()
            .await
            .map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("fsm start: {e}").into()),
            })?;

        // Step 4b: send request and read server response header
        let connected_next = connected.next().await.map_err(|e| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("fsm connected next: {e}").into()),
        })?;

        match connected_next {
            fsm::ConnectedNext::Closing(c) => {
                let _stats = c.next().await.map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("fsm closing: {e}").into()),
                })?;
            }
            fsm::ConnectedNext::StartRoot(start) => {
                // AtStartRoot::next() is synchronous
                let header = start.next();
                // AtBlobHeader::next() is async → returns (AtBlobContent, u64)
                let (content, _size) = header.next().await.map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("blob header: {e}").into()),
                })?;
                // AtBlobContent::write_all() is async; data must implement AsyncSliceWriter
                let end_blob =
                    content
                        .write_all(data.to_vec())
                        .await
                        .map_err(|e| PartisyError {
                            severity: Severity::Fatal,
                            source: Some(format!("write_all: {e}").into()),
                        })?;
                // AtEndBlob::next() is synchronous → EndBlobNext
                let blob_next = end_blob.next();
                match blob_next {
                    fsm::EndBlobNext::MoreChildren(start_child) => {
                        // AtStartChild::next(hash) is synchronous → AtBlobHeader
                        let child_header = start_child.next(h);
                        let (child_content, _child_size) =
                            child_header.next().await.map_err(|e| PartisyError {
                                severity: Severity::Fatal,
                                source: Some(format!("child header: {e}").into()),
                            })?;
                        // AtBlobContent::drain() is async → returns Result<AtEndBlob, DecodeError>
                        let child_end_blob =
                            child_content.drain().await.map_err(|e| PartisyError {
                                severity: Severity::Fatal,
                                source: Some(format!("child drain: {e}").into()),
                            })?;
                        // child_end_blob is AtEndBlob → next() is sync
                        let child_blob_next = child_end_blob.next();
                        match child_blob_next {
                            fsm::EndBlobNext::Closing(child_closing) => {
                                let _child_stats =
                                    child_closing.next().await.map_err(|e| PartisyError {
                                        severity: Severity::Fatal,
                                        source: Some(format!("child fsm closing: {e}").into()),
                                    })?;
                            }
                            fsm::EndBlobNext::MoreChildren(_) => {
                                // Single blob; more children unexpected
                            }
                        }
                    }
                    fsm::EndBlobNext::Closing(closing) => {
                        let _stats = closing.next().await.map_err(|e| PartisyError {
                            severity: Severity::Fatal,
                            source: Some(format!("fsm closing: {e}").into()),
                        })?;
                    }
                }
            }
            fsm::ConnectedNext::StartChild(start) => {
                // AtStartChild::next(hash) is synchronous → AtBlobHeader
                let header = start.next(h);
                let (content, _size) = header.next().await.map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("child header: {e}").into()),
                })?;
                // AtBlobContent::drain() is async → returns Result<AtEndBlob, DecodeError>
                let end_blob = content.drain().await.map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("child drain: {e}").into()),
                })?;
                // end_blob is AtEndBlob → next() is sync
                let blob_next = end_blob.next();
                match blob_next {
                    fsm::EndBlobNext::Closing(closing) => {
                        let _stats = closing.next().await.map_err(|e| PartisyError {
                            severity: Severity::Fatal,
                            source: Some(format!("child fsm closing: {e}").into()),
                        })?;
                    }
                    fsm::EndBlobNext::MoreChildren(_) => {
                        // Single blob; more children unexpected
                    }
                }
            }
        }

        Ok(())
    }
}

// ----------------------------------------------------------------------------
// HubIrohKeyspace：hub 节点密钥存储（fjall keyspace `h-iroh-node`）
// ----------------------------------------------------------------------------

/// Hub 侧 iroh 节点密钥存储 trait（fjall keyspace `h-iroh-node`）。
///
/// 由 `raft_store` 或 `registry` 现有模块实现（ADR-0015 裁定 5 扩展 / M4-WP04-T03）。
pub trait HubIrohKeyspace: Send + Sync {
    /// 取节点私钥（Base64 编码）。
    fn get_node_sk(&self) -> Result<Option<String>, PartisyError>;

    /// 存节点私钥（Base64 编码）。
    fn put_node_sk(&self, sk: &str) -> Result<(), PartisyError>;
}

// ----------------------------------------------------------------------------
// 辅助函数
// ----------------------------------------------------------------------------

fn base64_decode(input: &str) -> Result<Vec<u8>, PartisyError> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD.decode(input).map_err(|e| PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("base64 decode: {e}").into()),
    })
}

fn base64_encode(input: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD.encode(input)
}

// ----------------------------------------------------------------------------
// 测试与桩实现
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bytes::Bytes;
    use partisync_core::error::{PartisyError, Severity};

    use super::{HubIrohKeyspace, IrohHubNode};
    use partisync_transfer::iroh_blobs::{ChunkSink, ChunkSource};

    #[derive(Debug, Clone, Default)]
    pub struct InMemSink {
        received: Arc<std::sync::Mutex<Vec<Bytes>>>,
    }

    impl ChunkSink for InMemSink {
        async fn put_chunk(&self, chunk: &[u8]) -> Result<(), PartisyError> {
            self.received
                .lock()
                .unwrap()
                .push(Bytes::copy_from_slice(chunk));
            Ok(())
        }
    }

    #[derive(Debug, Clone, Default)]
    pub struct InMemSource {
        chunks: Arc<std::sync::Mutex<std::collections::HashMap<String, Bytes>>>,
    }

    impl ChunkSource for InMemSource {
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

    #[derive(Debug, Default)]
    pub struct InMemHubIrohKeyspace {
        sk: std::sync::Mutex<Option<String>>,
    }

    // Manual impl: Option<String> is Clone
    impl Clone for InMemHubIrohKeyspace {
        fn clone(&self) -> Self {
            Self {
                sk: std::sync::Mutex::new(self.sk.lock().unwrap().clone()),
            }
        }
    }

    impl HubIrohKeyspace for InMemHubIrohKeyspace {
        fn get_node_sk(&self) -> Result<Option<String>, PartisyError> {
            Ok(self.sk.lock().unwrap().clone())
        }

        fn put_node_sk(&self, sk: &str) -> Result<(), PartisyError> {
            *self.sk.lock().unwrap() = Some(sk.to_string());
            Ok(())
        }
    }

    #[tokio::test]
    async fn hub_node_new_generates_node_id() {
        let sink = InMemSink::default();
        let source = InMemSource::default();
        let keyspace: Arc<InMemHubIrohKeyspace> = Arc::new(InMemHubIrohKeyspace::default());
        let node = IrohHubNode::new(sink, source, keyspace)
            .await
            .expect("new should succeed");
        assert!(!node.node_id().as_bytes().is_empty());
    }

    #[tokio::test]
    async fn hub_node_node_addr_returns_string() {
        let sink = InMemSink::default();
        let source = InMemSource::default();
        let keyspace: Arc<InMemHubIrohKeyspace> = Arc::new(InMemHubIrohKeyspace::default());
        let node = IrohHubNode::new(sink, source, keyspace)
            .await
            .expect("new should succeed");
        let addr = node.node_addr();
        assert!(!addr.is_empty());
    }

    #[tokio::test]
    async fn hub_node_restores_sk_from_keyspace() {
        // Share the SAME Arc between both nodes so keyspace state is truly shared
        let keyspace: Arc<InMemHubIrohKeyspace> = Arc::new(InMemHubIrohKeyspace::default());

        let node1 = IrohHubNode::new(
            InMemSink::default(),
            InMemSource::default(),
            keyspace.clone(),
        )
        .await
        .expect("new should succeed");
        let id1 = node1.node_id();

        drop(node1);
        let node2 = IrohHubNode::new(InMemSink::default(), InMemSource::default(), keyspace)
            .await
            .expect("new should succeed");
        let id2 = node2.node_id();

        assert_eq!(id1, id2);
    }
}
