//! UploadAck 端到端集成测试（M5-WP01-T03）
//!
//! 全程 loopback 直连（`presets::Minimal`：无 relay / 无 DNS），离线可跑；
//! 关键 await 均带 30s 超时。
//!
//! 关键设计：测试**不** await hub_task 直到 device 主动 close conn。
//! handle_incoming 的 while 循环需要 conn 关闭才会退出（`iroh_channel.rs:206-221`）。

use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;
use iroh::endpoint::presets::Minimal;
use iroh_base::{EndpointAddr, TransportAddr};
use iroh_blobs::protocol::{GetRequest, ALPN};
use iroh_blobs::store::mem::MemStore;

use partisync_cas::ChunkStore;
use partisync_hub::iroh_channel::handle_incoming;
use partisync_hub::upload_acker::{AckError, UploadAcker};

async fn t<F: std::future::Future>(name: &'static str, fut: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(30), fut)
        .await
        .unwrap_or_else(|_| panic!("e2e step timeout: {name}"))
}

async fn bind() -> iroh::Endpoint {
    iroh::Endpoint::builder(Minimal)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .expect("endpoint bind")
}

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
        "hub-m5-wp01-{tag}-{}",
        partisync_core::Ulid::now()
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upload_ack_basic() {
    let cas = ChunkStore::open_in_memory(&tempdir("basic"))
        .await
        .expect("cas open");

    let hub_ep = bind().await;
    let hub_addr = loopback(&hub_ep);

    // hub 任务：accept + handle_incoming
    let hub_task = tokio::spawn({
        let cas = cas.clone();
        async move {
            let incoming = t("hub accept", hub_ep.accept()).await.expect("hub incoming");
            t("hub handle_incoming", handle_incoming(incoming, cas)).await
        }
    });

    let dev_ep = bind().await;
    let dev_store: iroh_blobs::api::Store = MemStore::default().into();
    let conn = t("dev connect", dev_ep.connect(hub_addr, ALPN))
        .await
        .expect("device connect");
    let mut acker = UploadAcker::spawn(conn.clone());

    // 准备 + push
    let payload = Bytes::from_static(b"upload_ack_basic payload \xe4\xb8\xad\xe6\x96\x87");
    let hash = t(
        "dev add_bytes",
        dev_store.blobs().add_bytes(payload.clone()).with_tag(),
    )
    .await
    .expect("add_bytes")
    .hash;
    t("dev push", async {
        dev_store
            .remote()
            .execute_push(conn.clone(), GetRequest::blob(hash).into())
            .complete()
            .await
    })
    .await
    .expect("push");

    // 等 ack
    let status = t(
        "dev expect_ack",
        acker.expect_ack(hash, Duration::from_secs(15)),
    )
    .await
    .expect("expect_ack ok");
    assert_eq!(status, partisync_hub::iroh_channel::UploadAckStatus::Accepted);

    // 关闭 conn → hub 端 while 循环退出 → hub_task 完成
    acker.close();

    let summary = t("hub join", hub_task)
        .await
        .expect("hub task join")
        .expect("handle_incoming ok");
    assert_eq!(summary.chunks_received, 1);
    assert_eq!(
        summary.bytes_received,
        u64::try_from(payload.len()).unwrap()
    );

    // CAS 落库断言
    let hex = partisync_cas::content_hash(&payload);
    assert_eq!(cas.get(&hex).await.expect("cas get"), payload.to_vec());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upload_ack_multiple() {
    let cas = ChunkStore::open_in_memory(&tempdir("multi"))
        .await
        .expect("cas open");

    let hub_ep = bind().await;
    let hub_addr = loopback(&hub_ep);
    let hub_task = tokio::spawn({
        let cas = cas.clone();
        async move {
            let incoming = t("hub accept", hub_ep.accept()).await.expect("hub incoming");
            t("hub handle_incoming", handle_incoming(incoming, cas)).await
        }
    });

    let dev_ep = bind().await;
    let dev_store: iroh_blobs::api::Store = MemStore::default().into();
    let conn = t("dev connect", dev_ep.connect(hub_addr, ALPN))
        .await
        .expect("device connect");
    let mut acker = UploadAcker::spawn(conn.clone());

    let mut hashes = Vec::new();
    for i in 0..3 {
        let payload = Bytes::from(format!("payload-{i}").into_bytes());
        let hash = t(
            "dev add_bytes",
            dev_store.blobs().add_bytes(payload.clone()).with_tag(),
        )
        .await
        .expect("add_bytes")
        .hash;
        t("dev push", async {
            dev_store
                .remote()
                .execute_push(conn.clone(), GetRequest::blob(hash).into())
                .complete()
                .await
        })
        .await
        .expect("push");
        hashes.push(hash);
    }

    for expected in &hashes {
        let status = t(
            "dev expect_ack",
            acker.expect_ack(*expected, Duration::from_secs(15)),
        )
        .await
        .expect("expect_ack ok");
        assert_eq!(
            status,
            partisync_hub::iroh_channel::UploadAckStatus::Accepted
        );
    }

    acker.close();
    let summary = t("hub join", hub_task)
        .await
        .expect("hub task join")
        .expect("handle_incoming ok");
    assert_eq!(summary.chunks_received, 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upload_ack_timeout_when_no_ack() {
    // 故意：device 连接建立后 hub 端立即关闭连接 → 设备端 expect_ack 应超时或
    // 报 ConnectionClosed。
    let hub_ep = bind().await;
    let hub_addr = loopback(&hub_ep);
    let _hub_task = tokio::spawn(async move {
        if let Some(incoming) = hub_ep.accept().await {
            // accept() 拿走 incoming ownership → 我们不能同时传给 handle_incoming
            // 这里只 accept + 立即 close，模拟设备 push 后立即断连场景
            if let Ok(accepting) = incoming.accept() {
                if let Ok(connection) = accepting.await {
                    connection.close(0u32.into(), b"timeout-test");
                }
            }
        }
    });

    let dev_ep = bind().await;
    let conn = dev_ep.connect(hub_addr, ALPN).await.expect("connect");
    let mut acker = UploadAcker::spawn(conn.clone());

    let err = acker
        .expect_ack(
            iroh_blobs::Hash::from_bytes([0u8; 32]),
            Duration::from_millis(500),
        )
        .await
        .expect_err("must timeout");
    match err {
        AckError::Timeout { .. } | AckError::ConnectionClosed => {}
        other => panic!("expected Timeout or ConnectionClosed, got {other:?}"),
    }
}