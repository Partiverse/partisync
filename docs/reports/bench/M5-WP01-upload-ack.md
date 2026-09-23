# 基准评测报告：M5-WP01 UploadAck 协议时延与吞吐评估

> **任务编号**: `M5-WP01-T06`
> **关联规格**: [docs/specs/M5-WP01.md](../../specs/M5-WP01.md)
> **评测日期**: 2026-09-24
> **测试入口**: `cargo test -p partisync-hub --test m5_wp01 -- --nocapture upload_ack_latency_and_throughput_benchmark`

---

## 1. 评测背景与目的

在 M4-WP04 中，`iroh-blobs` 的 `execute_push` 为 fire-and-forget 语义，设备端无法感知 Hub 端是否已完成 CAS 落盘。M5-WP01 引入了基于独立 QUIC 单向流（Unidirectional Stream）的 35 字节定长 `UploadAck (v0x01)` 确认帧。

本评测旨在量化引入逐块 ACK 确认与 `ChunkStore` 落盘校验后的端到端往返时延（RTT）及吞吐开销，验证其是否满足 DoD 契约指标（Loopback 下 P99 < 15ms）。

---

## 2. 测试环境与工作负载

- **硬件/OS**: Apple Silicon (arm64, Darwin 25.6.0)
- **传输栈**: `iroh = 1.2.0` (`presets::Minimal`, Loopback QUIC 直连) + `iroh-blobs = 0.103`
- **存储后端**: `ChunkStore::open_in_memory`（SQLite 内存索引 + 本地临时文件系统原子 rename 落盘 + BLAKE3 完整性校验）
- **负载规模**:
  - 连续推送块数 (`COUNT`): 20 个独立块
  - 单块大小 (`CHUNK_SIZE`): 16 KB (16,384 Bytes)
  - 每块流程：Device `execute_push` → Hub `handle_stream` → Hub `blake3` 校验 → Hub `CAS put_chunk` → Hub `open_uni + write 35B UploadAck` → Device `UploadAcker::expect_ack` 接收解码。

---

## 3. 实测结果数据

```text
BENCH_RESULT: count=20, chunk_bytes=16384, total_ms=63.16, p50_ms=2.48, p99_ms=8.35, max_ms=8.35
```

| 指标项 | 实测值 | DoD 门槛要求 | 结论 |
|---|---|---|---|
| **20×16KB 总耗时** | `63.16 ms` | - | ✅ |
| **单块 Push+CAS+ACK P50 时延** | **`2.48 ms`** | `< 5.0 ms` | ✅ 远优于预期 |
| **单块 Push+CAS+ACK P99 时延** | **`8.35 ms`** | `< 15.0 ms` | ✅ 达标（余量 44%） |
| **单块最大时延 (Max)** | `8.35 ms` | `< 50.0 ms` | ✅ |
| **平均吞吐速率 (含同步等待)** | `~316 chunks/sec` | `> 100 chunks/sec` | ✅ |

---

## 4. 关键结论与架构分析

1. **QUIC Uni-Stream 零阻塞优势**：
   Hub 端通过 `connection.open_uni()` 发送 35 字节定长帧，与 `iroh-blobs` 主数据流完全解耦，协议头开销仅占 16KB 数据块的 `0.21%`。
2. **数据安全与性能兼得**：
   在完整包含 BLAKE3 哈希校验、文件系统临时文件写入 + 原子 `rename`、SQLite 引用计数更新以及 QUIC ACK 帧往返的全链条下，中位耗时仅 **2.48ms**，P99 仅 **8.35ms**，彻底消除了 fire-and-forget 断连丢块隐患。
