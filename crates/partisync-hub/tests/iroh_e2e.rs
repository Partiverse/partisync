//! iroh 设备通道端到端集成测试（M4-WP04-T06）。
//!
//! 全程 loopback 直连（`presets::Minimal`：无 relay / 无 DNS 地址发现），
//! 离线可跑、不依赖外网；关键 await 均带 60s 超时防挂起：
//!
//! 1. **upload**：设备 push blob → hub `handle_incoming`（MemStore 暂存）→
//!    blake3 校验 → CAS `ChunkSink`（ChunkStore 落库 + refcount=1）。
//! 2. **download**：hub `IrohBlobsExecutor`（CAS `ChunkSource`）→ 直连设备
//!    provider → iroh-blobs `get::fsm` 状态机走通全路径。

use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;
use iroh::endpoint::presets::Minimal;
use iroh_base::{EndpointAddr, TransportAddr};
use iroh_blobs::protocol::{GetRequest, ALPN};
use iroh_blobs::provider::{events, handle_connection};
use iroh_blobs::store::mem::MemStore;

use partisync_cas::ChunkStore;
use partisync_hub::iroh_channel::{handle_incoming, IrohBlobsExecutor};

/// 单步超时包装：防 iroh 网络意外挂起拖死 CI。
async fn t<F: std::future::Future>(name: &'static str, fut: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(60), fut)
        .await
        .unwrap_or_else(|_| panic!("e2e step timeout: {name}"))
}

/// 绑定一个离线 endpoint；`with_alpn` 决定是否注册 iroh-blobs ALPN（可 accept）。
async fn bind(with_alpn: bool) -> iroh::Endpoint {
    let mut builder = iroh::Endpoint::builder(Minimal);
    if with_alpn {
        builder = builder.alpns(vec![ALPN.to_vec()]);
    }
    builder
        .bind()
        .await
        .expect("endpoint bind (presets::Minimal)")
}

/// 以 loopback 直连地址构造 EndpointAddr（Minimal 无 relay，只能走 IP 直连）。
fn loopback(ep: &iroh::Endpoint) -> EndpointAddr {
    let port = ep
        .bound_sockets()
        .first()
        .map(|s| s.port())
        .expect("endpoint has bound socket");
    EndpointAddr::from_parts(
        ep.id(),
        [TransportAddr::Ip(SocketAddr::from(([127, 0, 0, 1], port)))],
    )
}

fn tempdir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "hub-iroh-e2e-{tag}-{}",
        partisync_core::Ulid::now()
    ))
}

/// upload：设备 push → hub MemStore → CAS 落库（blake3 同口径 + refcount=1）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upload_push_lands_in_cas() {
    let cas = ChunkStore::open_in_memory(&tempdir("up"))
        .await
        .expect("cas open");

    // hub 侧：accept 一个连接并跑完整 handle_incoming（含 MemStore→CAS 同步）
    let hub_ep = bind(true).await;
    let hub_addr = loopback(&hub_ep);
    let hub_task = tokio::spawn({
        let cas = cas.clone();
        async move {
            let incoming = t("hub accept", hub_ep.accept())
                .await
                .expect("hub incoming");
            t("hub handle_incoming", handle_incoming(incoming, cas)).await
        }
    });

    // 设备侧：本地加 blob，push 到 hub
    let dev_ep = bind(false).await;
    let dev_store: iroh_blobs::api::Store = MemStore::default().into();
    let payload = Bytes::from_static(b"e2e upload payload \xe4\xbd\xa0\xe5\xa5\xbd");
    let tag = t(
        "dev add_bytes",
        dev_store.blobs().add_bytes(payload.clone()).with_tag(),
    )
    .await
    .expect("add_bytes with_tag");
    let hash = tag.hash;

    let conn = t("dev connect", dev_ep.connect(hub_addr, ALPN))
        .await
        .expect("device connect to hub");
    t("dev push", async {
        dev_store
            .remote()
            .execute_push(conn.clone(), GetRequest::blob(hash).into())
            .complete()
            .await
    })
    .await
    .expect("execute_push complete");

    // SPEC M4-WP04 §2 UploadAck 占位：iroh-blobs 0.103 push 是 fire-and-forget
    // （`send.finish()` 即返回、不读 hub 应答），设备若立即关连接，hub 可能还没
    // accept 该流，数据随连接丢弃。真实设备侧（M5 执行器）将等 hub 同步确认；
    // 此处以保持连接 50ms 代之，再正常关闭触发 hub 侧收尾。
    tokio::time::sleep(Duration::from_millis(50)).await;
    drop(conn);

    // hub 侧摘要：1 块、字节数一致
    let summary = t("hub join", hub_task)
        .await
        .expect("hub task join")
        .expect("handle_incoming ok");
    assert_eq!(summary.chunks_received, 1);
    assert_eq!(
        summary.bytes_received,
        u64::try_from(payload.len()).unwrap()
    );

    // CAS 落库断言：hash 口径一致 + 内容一致 + refcount=1
    let hex = partisync_cas::content_hash(&payload);
    assert_eq!(
        hex,
        hash.to_string(),
        "iroh Hash 与 CAS content_hash 同为 blake3 hex"
    );
    assert_eq!(cas.get(&hex).await.expect("cas get"), payload.to_vec());
    let stats = cas.stats().await.expect("cas stats");
    assert_eq!(stats.chunks, 1);
    assert_eq!(stats.refs, 1);
}

/// download：hub executor（CAS source）→ 直连设备 provider → fsm 状态机走通。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn download_executor_fetches_via_fsm() {
    // 设备作为 iroh-blobs provider 持有 blob
    let dev_ep = bind(true).await;
    let dev_store: iroh_blobs::api::Store = MemStore::default().into();
    let payload = Bytes::from_static(b"e2e download payload \xf0\x9f\x8c\x90");
    let tag = t(
        "dev add_bytes",
        dev_store.blobs().add_bytes(payload.clone()).with_tag(),
    )
    .await
    .expect("add_bytes with_tag");
    let hash = tag.hash;
    let dev_addr = loopback(&dev_ep);

    let provider = tokio::spawn(async move {
        let incoming = t("dev accept", dev_ep.accept())
            .await
            .expect("dev incoming");
        let accepting = incoming.accept().expect("dev incoming accept");
        let conn = t("dev handshake", accepting).await.expect("dev handshake");
        handle_connection(conn, dev_store, events::EventSender::DEFAULT).await;
    });

    // hub 侧：CAS source 已有该块，executor 直连设备驱动 fsm
    let cas = ChunkStore::open_in_memory(&tempdir("down"))
        .await
        .expect("cas open");
    cas.put(&payload).await.expect("cas put");
    let hub_ep = bind(false).await;
    let executor = IrohBlobsExecutor::new(cas, hub_ep);
    t(
        "hub send_chunk",
        executor.send_chunk(&dev_addr, &hash.to_string()),
    )
    .await
    .expect("send_chunk fsm complete");

    t("provider join", provider)
        .await
        .expect("provider task join");
}
