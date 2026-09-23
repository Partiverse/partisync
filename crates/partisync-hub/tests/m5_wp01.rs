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
    std::env::temp_dir().join(format!("hub-m5-wp01-{tag}-{}", partisync_core::Ulid::now()))
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
            let incoming = t("hub accept", hub_ep.accept())
                .await
                .expect("hub incoming");
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
    assert_eq!(
        status,
        partisync_hub::iroh_channel::UploadAckStatus::Accepted
    );

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
            let incoming = t("hub accept", hub_ep.accept())
                .await
                .expect("hub incoming");
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upload_ack_duplicate() {
    let cas = ChunkStore::open_in_memory(&tempdir("dup"))
        .await
        .expect("cas open");

    let hub_ep = bind().await;
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

    let dev_ep = bind().await;
    let dev_store: iroh_blobs::api::Store = MemStore::default().into();
    let conn = t("dev connect", dev_ep.connect(hub_addr, ALPN))
        .await
        .expect("device connect");
    let mut acker = UploadAcker::spawn(conn.clone());

    let payload = Bytes::from_static(b"upload_ack_duplicate idempotent payload");
    let hash = t(
        "dev add_bytes",
        dev_store.blobs().add_bytes(payload.clone()).with_tag(),
    )
    .await
    .expect("add_bytes")
    .hash;

    // 第一次 push -> Accepted
    t("dev push 1", async {
        dev_store
            .remote()
            .execute_push(conn.clone(), GetRequest::blob(hash).into())
            .complete()
            .await
    })
    .await
    .expect("push 1");

    let status1 = t(
        "dev expect_ack 1",
        acker.expect_ack(hash, Duration::from_secs(15)),
    )
    .await
    .expect("expect_ack 1 ok");
    assert_eq!(
        status1,
        partisync_hub::iroh_channel::UploadAckStatus::Accepted
    );

    // 第二次在同一连接重复 push -> Duplicate
    t("dev push 2", async {
        dev_store
            .remote()
            .execute_push(conn.clone(), GetRequest::blob(hash).into())
            .complete()
            .await
    })
    .await
    .expect("push 2");

    let status2 = t(
        "dev expect_ack 2",
        acker.expect_ack(hash, Duration::from_secs(15)),
    )
    .await
    .expect("expect_ack 2 ok");
    assert_eq!(
        status2,
        partisync_hub::iroh_channel::UploadAckStatus::Duplicate
    );

    acker.close();
    let summary = t("hub join", hub_task)
        .await
        .expect("hub task join")
        .expect("handle_incoming ok");
    // 只有首次写入计入 chunks_received，Duplicate 跳过
    assert_eq!(summary.chunks_received, 1);
    let stats = cas.stats().await.expect("cas stats");
    assert_eq!(stats.chunks, 1);
    assert_eq!(stats.refs, 1);
}

#[derive(Clone, Debug)]
struct FailingSink;

impl partisync_transfer::iroh_blobs::ChunkSink for FailingSink {
    async fn put_chunk(&self, _chunk: &[u8]) -> Result<(), partisync_core::error::PartisyError> {
        Err(partisync_core::error::PartisyError {
            severity: partisync_core::error::Severity::Fatal,
            source: Some("disk quota exceeded".into()),
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upload_ack_rejected() {
    let hub_ep = bind().await;
    let hub_addr = loopback(&hub_ep);
    let hub_task = tokio::spawn(async move {
        let incoming = t("hub accept", hub_ep.accept())
            .await
            .expect("hub incoming");
        t(
            "hub handle_incoming",
            handle_incoming(incoming, FailingSink),
        )
        .await
    });

    let dev_ep = bind().await;
    let dev_store: iroh_blobs::api::Store = MemStore::default().into();
    let conn = t("dev connect", dev_ep.connect(hub_addr, ALPN))
        .await
        .expect("device connect");
    let mut acker = UploadAcker::spawn(conn.clone());

    let payload = Bytes::from_static(b"upload_ack_rejected payload");
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

    let status = t(
        "dev expect_ack",
        acker.expect_ack(hash, Duration::from_secs(15)),
    )
    .await
    .expect("expect_ack ok");
    assert_eq!(
        status,
        partisync_hub::iroh_channel::UploadAckStatus::Rejected
    );

    acker.close();
    let summary = t("hub join", hub_task)
        .await
        .expect("hub task join")
        .expect("handle_incoming ok");
    assert_eq!(summary.chunks_received, 0);
}

#[derive(Clone, Debug)]
struct FlakySink<S> {
    inner: S,
    failures_remaining: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

impl<S: partisync_transfer::iroh_blobs::ChunkSink> partisync_transfer::iroh_blobs::ChunkSink
    for FlakySink<S>
{
    async fn put_chunk(&self, chunk: &[u8]) -> Result<(), partisync_core::error::PartisyError> {
        use std::sync::atomic::Ordering;
        let prev = self.failures_remaining.load(Ordering::SeqCst);
        if prev > 0 {
            self.failures_remaining.fetch_sub(1, Ordering::SeqCst);
            return Err(partisync_core::error::PartisyError {
                severity: partisync_core::error::Severity::Retryable,
                source: Some("transient hub storage busy".into()),
            });
        }
        self.inner.put_chunk(chunk).await
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upload_ack_retry_success() {
    let cas = ChunkStore::open_in_memory(&tempdir("retry"))
        .await
        .expect("cas open");
    let flaky = FlakySink {
        inner: cas.clone(),
        failures_remaining: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(2)),
    };

    let hub_ep = bind().await;
    let hub_addr = loopback(&hub_ep);
    let hub_task = tokio::spawn(async move {
        let incoming = t("hub accept", hub_ep.accept())
            .await
            .expect("hub incoming");
        t("hub handle_incoming", handle_incoming(incoming, flaky)).await
    });

    let dev_ep = bind().await;
    let dev_store: iroh_blobs::api::Store = MemStore::default().into();
    let conn = t("dev connect", dev_ep.connect(hub_addr, ALPN))
        .await
        .expect("device connect");
    let mut acker = UploadAcker::spawn(conn.clone());

    let payload = Bytes::from_static(b"upload_ack_retry_success payload");
    let hash = t(
        "dev add_bytes",
        dev_store.blobs().add_bytes(payload.clone()).with_tag(),
    )
    .await
    .expect("add_bytes")
    .hash;

    let policy = partisync_hub::upload_acker::UploadRetryPolicy {
        initial_backoff: Duration::from_millis(20),
        max_retries: 4,
        ack_timeout: Duration::from_secs(10),
    };

    let status = t(
        "dev push_with_retry",
        acker.push_with_retry(hash, policy, || {
            let dev_store = dev_store.clone();
            let conn = conn.clone();
            async move {
                dev_store
                    .remote()
                    .execute_push(conn, GetRequest::blob(hash).into())
                    .complete()
                    .await
                    .map(|_| ())
                    .map_err(|_| AckError::ConnectionClosed)
            }
        }),
    )
    .await
    .expect("push_with_retry succeeded after 2 transient Retrying frames");

    assert_eq!(
        status,
        partisync_hub::iroh_channel::UploadAckStatus::Accepted
    );

    acker.close();
    let summary = t("hub join", hub_task)
        .await
        .expect("hub task join")
        .expect("handle_incoming ok");
    assert_eq!(summary.chunks_received, 1);
    let hex = partisync_cas::content_hash(&payload);
    assert_eq!(cas.get(&hex).await.expect("cas get"), payload.to_vec());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upload_ack_latency_and_throughput_benchmark() {
    let cas = ChunkStore::open_in_memory(&tempdir("bench"))
        .await
        .expect("cas open");

    let hub_ep = bind().await;
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

    let dev_ep = bind().await;
    let dev_store: iroh_blobs::api::Store = MemStore::default().into();
    let conn = t("dev connect", dev_ep.connect(hub_addr, ALPN))
        .await
        .expect("device connect");
    let mut acker = UploadAcker::spawn(conn.clone());

    const COUNT: usize = 20;
    const CHUNK_SIZE: usize = 16 * 1024; // 16 KB per chunk
    let mut rtts_us = Vec::with_capacity(COUNT);
    let total_start = std::time::Instant::now();

    for i in 0..COUNT {
        let mut buf = vec![0u8; CHUNK_SIZE];
        buf[0..4].copy_from_slice(&(i as u32).to_le_bytes());
        let payload = Bytes::from(buf);
        let hash = dev_store
            .blobs()
            .add_bytes(payload)
            .with_tag()
            .await
            .expect("add_bytes")
            .hash;

        let step_start = std::time::Instant::now();
        dev_store
            .remote()
            .execute_push(conn.clone(), GetRequest::blob(hash).into())
            .complete()
            .await
            .expect("push");

        let status = acker
            .expect_ack(hash, Duration::from_secs(10))
            .await
            .expect("expect_ack");
        let elapsed_us = step_start.elapsed().as_micros() as u64;
        rtts_us.push(elapsed_us);
        assert_eq!(
            status,
            partisync_hub::iroh_channel::UploadAckStatus::Accepted
        );
    }

    let total_elapsed = total_start.elapsed();
    acker.close();
    let summary = t("hub join", hub_task)
        .await
        .expect("hub task join")
        .expect("handle_incoming ok");
    assert_eq!(summary.chunks_received, COUNT as u64);

    rtts_us.sort_unstable();
    let p50_us = rtts_us[COUNT / 2];
    let p99_us = rtts_us[(COUNT * 99) / 100];
    let max_us = *rtts_us.last().unwrap();
    println!(
        "BENCH_RESULT: count={COUNT}, chunk_bytes={CHUNK_SIZE}, total_ms={:.2}, p50_ms={:.2}, p99_ms={:.2}, max_ms={:.2}",
        total_elapsed.as_secs_f64() * 1000.0,
        p50_us as f64 / 1000.0,
        p99_us as f64 / 1000.0,
        max_us as f64 / 1000.0
    );
    // DoD 断言：loopback 下单次 push+ack P99 < 50ms
    assert!(
        p99_us < 50_000,
        "loopback UploadAck P99 should be < 50ms, got {}us",
        p99_us
    );
}
