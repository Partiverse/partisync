//! Hub 侧 iroh/ihor-blobs 设备通道（ADR-0015 / M3-WP04-T06）。
//!
//! Hub 作为 ALWAYS reachable relay endpoint 监听设备连接，通过 iroh-blobs
//! 接收设备上行数据（写 CAS）和下发数据（从 CAS 读）。
//!
//! ## 模块边界
//!
//! - iroh crate 特有类型（公钥/连接句柄）不逃逸本模块；
//! - 对外暴露的 trait 为 `ChunkSink` / `ChunkSource`（定义在 `partisync-transfer`）；
//! - 具体执行器 `IrohBlobsExecutor` / `IrohBlobsListener` 也在 `partisync-transfer`。
//!
//! ## 依赖方向（ADR-0015 裁定 1）
//!
//! ```text
//! partisync-hub  →  partisync-transfer  →  partisync-cas
//! ```

use std::sync::Arc;

use partisync_core::error::PartisyError;
use partisync_transfer::iroh_blobs::{ChunkSink, ChunkSource};

/// Hub 侧 iroh 节点门面（ADR-0015 裁定 2）。
///
/// 包装 `iroh::Node`，提供：
/// - 从 fjall keyspace `h-iroh-node` 加载/初始化节点；
/// - 中继监听（hub 作为 ALWAYS reachable endpoint）；
/// - 设备连接分发（上行 upload / 下行 download）。
///
/// # ADR-0015 裁定 2
/// Hub 开启 `iroh::node::Config::enable_relay`，设备 NAT 穿透失败时
/// 自动走 iroh relay（后台透明重试，不阻塞上传）。
#[derive(Debug)]
#[allow(dead_code)]
pub struct IrohHubNode<S: ChunkSink, C: ChunkSource> {
    sink: Arc<S>,
    source: Arc<C>,
    // TODO: iroh::Node 句柄（iroh 版本对齐后填实）
    _p: std::marker::PhantomData<fn()>,
}

impl<S: ChunkSink, C: ChunkSource> IrohHubNode<S, C> {
    /// 从 fjall 加载或新建 iroh 节点，开始监听（ADR-0015 裁定 2）。
    ///
    /// # Errors
    /// iroh 节点初始化或fjall keyspace 读写失败。
    #[allow(dead_code)]
    pub async fn new(sink: S, source: C, _db: &dyn HubIrohKeyspace) -> Result<Self, PartisyError> {
        // TODO: ADR-0015 进场的版本对齐阻塞——
        //   1. 建立 iroh::Node（密钥从 fjall keyspace `h-iroh-node` 加载，
        //      新建则存入同一 keyspace，重启可复现节点 ID）；
        //   2. 开启 `iroh::node::Config::enable_relay`；
        //   3. 绑定 addr 开始监听设备连接；
        //   4. spawn 任务循环 accept_and_read_blob / write_blob session。
        eprintln!("IrohHubNode::new (iroh 版本待对齐)");
        Ok(Self {
            sink: Arc::new(sink),
            source: Arc::new(source),
            _p: std::marker::PhantomData,
        })
    }

    /// 启动节点（阻塞直到节点关闭）。
    ///
    /// # Errors
    /// iroh 节点运行错误。
    #[allow(dead_code)]
    pub async fn run(self) -> Result<(), PartisyError> {
        // TODO: iroh::Node::run() 或等价的 accept 循环。
        //   每收到设备连接，分发到 upload 或 download handler：
        //   - Upload handler: IrohBlobsListener::new(self.sink).accept().await
        //   - Download handler: IrohBlobsExecutor::new(self.source).run_download().await
        eprintln!("IrohHubNode::run (iroh 版本待接线)");
        Ok(())
    }

    /// 返回本 hub 的 iroh 节点地址（可分享给设备，用于 QUIC 连接）。
    ///
    /// # Errors
    /// iroh 节点未初始化。
    #[allow(dead_code)]
    pub async fn node_addr(&self) -> Result<String, PartisyError> {
        // TODO: iroh::Node::addr() → iroh::NodeAddr → serialize to string
        eprintln!("IrohHubNode::node_addr (iroh 版本待对齐)");
        Ok("iroh://mock-node-addr".to_string())
    }
}

/// Hub 侧 iroh 节点密钥存储 trait（fjall keyspace `h-iroh-node`）。
///
/// 由 `raft_store` 或 `registry` 现有模块实现（ADR-0015 裁定 5 扩展）。
pub trait HubIrohKeyspace: Send + Sync {
    /// 取节点私钥（Base64 编码）。
    ///
    /// # Errors
    /// keyspace 读取失败。
    fn get_node_sk(&self) -> Result<Option<String>, PartisyError>;

    /// 存节点私钥（Base64 编码）。
    ///
    /// # Errors
    /// keyspace 写入失败。
    fn put_node_sk(&self, sk: &str) -> Result<(), PartisyError>;
}

// ----------------------------------------------------------------------------
// 测试桩实现（无 iroh 依赖的纯逻辑验证）
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bytes::Bytes;
    use partisync_core::error::{PartisyError, Severity};
    use partisync_transfer::iroh_blobs::{ChunkSink, ChunkSource};

    use super::{HubIrohKeyspace, IrohHubNode};

    /// 内存内 ChunkSink 桩。
    #[derive(Debug, Default)]
    pub struct InMemSink {
        received: Arc<std::sync::Mutex<Vec<Bytes>>>,
    }

    #[allow(dead_code)]
    impl InMemSink {
        pub fn chunks(&self) -> Vec<Bytes> {
            self.received.lock().unwrap().clone()
        }
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

    /// 内存内 ChunkSource 桩。
    #[derive(Debug, Default)]
    pub struct InMemSource {
        chunks: Arc<std::sync::Mutex<std::collections::HashMap<String, Bytes>>>,
    }

    #[allow(dead_code)]
    impl InMemSource {
        pub fn insert(&self, hash: &str, data: impl Into<Bytes>) {
            self.chunks
                .lock()
                .unwrap()
                .insert(hash.to_string(), data.into());
        }
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

    /// 内存内 HubIrohKeyspace 桩。
    #[derive(Debug, Default)]
    pub struct InMemHubIrohKeyspace {
        sk: std::sync::Mutex<Option<String>>,
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
    async fn hub_node_new_returns_iroh_node() {
        let sink = InMemSink::default();
        let source = InMemSource::default();
        let keyspace = InMemHubIrohKeyspace::default();
        let node = IrohHubNode::new(sink, source, &keyspace).await;
        assert!(node.is_ok());
    }

    #[tokio::test]
    async fn hub_node_run_completes() {
        let sink = InMemSink::default();
        let source = InMemSource::default();
        let keyspace = InMemHubIrohKeyspace::default();
        let node = IrohHubNode::new(sink, source, &keyspace).await.unwrap();
        // 阻塞桩不真跑——验证路径不 panic 即可
        let result = node.run().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn hub_node_addr_returns_string() {
        let sink = InMemSink::default();
        let source = InMemSource::default();
        let keyspace = InMemHubIrohKeyspace::default();
        let node = IrohHubNode::new(sink, source, &keyspace).await.unwrap();
        let addr = node.node_addr().await.unwrap();
        assert!(!addr.is_empty());
    }
}
