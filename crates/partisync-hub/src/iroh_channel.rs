//! Hub 侧 iroh/ihor-blobs 设备通道（ADR-0015 / M4-WP04-T02）。
//!
//! Hub 作为 ALWAYS reachable relay endpoint，监听设备连接，通过 iroh-blobs
//! 接收设备上行数据（写 CAS）和下发数据（从 CAS 读）。

use std::sync::Arc;

use iroh::endpoint::presets;
use iroh_blobs::protocol::ALPN;
use iroh_blobs::{protocol::GetRequest, provider::handle_connection, Hash};
use tracing::{debug, info, warn};

// `EventSender::DEFAULT` is in a private module; re-export for internal use
use iroh_blobs::provider::events as iroh_events;

use partisync_core::error::{PartisyError, Severity};
use partisync_transfer::iroh_blobs::{ChunkSink, ChunkSource};

/// Hub 侧 iroh 节点门面（ADR-0015 裁定 2 / M4-WP04-T02）。
///
/// 包装 `iroh::Endpoint` + `iroh_blobs::MemStore`，提供：
/// - 从 `HubIrohKeyspace` 加载/初始化节点密钥；
/// - 设备连接 accept 循环（upload）；
/// - CAS `ChunkSink`/`ChunkSource` 与 iroh-blobs 协议的打通。
///
/// # 接线说明（M4-WP04-T02）
///
/// Upload：设备连接 → `Endpoint::accept()` → `Incoming::accept()` →
/// `handle_connection(conn, MemStore)` → MemStore 临时存储 →
/// 后台任务同步到 CAS `ChunkSink`（T03）。
///
/// Download：T04 实现；通过 hub 已建立的连接回传数据。
/// Hub 侧 iroh 节点门面（ADR-0015 裁定 2 / M4-WP04-T02）。
///
/// 包装 `iroh::Endpoint` + `iroh_blobs::MemStore`，提供：
/// - 从 `HubIrohKeyspace` 加载/初始化节点密钥；
/// - 设备连接 accept 循环（upload）；
/// - CAS `ChunkSink`/`ChunkSource` 与 iroh-blobs 协议的打通。
///
/// # 接线说明（M4-WP04-T02）
///
/// Upload：设备连接 → `Endpoint::accept()` → `Incoming::accept()` →
/// `handle_connection(conn, MemStore)` → MemStore 临时存储 →
/// 后台任务同步到 CAS `ChunkSink`（T03）。
///
/// Download：T04 实现；通过 hub 已建立的连接回传数据。
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

    /// 启动节点：accept 循环处理设备上传。
    ///
    /// # Errors
    /// iroh 运行错误。
    pub async fn run(self) -> Result<(), PartisyError> {
        info!("IrohHubNode running: id={}", self.endpoint.id());

        loop {
            match self.endpoint.accept().await {
                Some(incoming) => {
                    let sink = self.sink.clone();
                    let source = self.source.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_incoming(incoming, sink, source).await {
                            warn!("incoming connection error: {}", e);
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

/// 处理单个设备连接：调用 `handle_connection` 将 blob 存入 MemStore。
async fn handle_incoming<
    S: ChunkSink + Clone + Send + 'static,
    C: ChunkSource + Clone + Send + 'static,
>(
    incoming: iroh::endpoint::Incoming,
    _sink: S,
    _source: C,
) -> Result<(), PartisyError> {
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

    handle_connection(connection, store, iroh_events::EventSender::DEFAULT).await;

    debug!("connection closed, blobs buffered in mem_store");
    // TODO (M4-WP04-T03)：MemStore → CAS 同步
    Ok(())
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
