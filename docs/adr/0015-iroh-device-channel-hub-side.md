# ADR-0015: Hub 侧 iroh/iroh-blobs 设备通道接线

状态: 已接受 · 日期: 2026-09-21 · 决策人: @lead（AI 代理起草，人工已签核）
关联: SPEC M3-WP04（裁定 6 / D3）、ADR-0008（iroh 设备网络层）、
ADR-0009（iroh-blobs 块传输）、M3-WP04-T06

## 背景

D3（iroh + iroh-blobs 设备↔hub 数据传输进场）是 M3-WP04 的关键依赖项。
ADRs 0008/0009 已分别在设备网络层与块传输层做出决策；本 ADR 解决 **hub
侧接线** 的具体技术决策：

1. hub 作为 ALWAYS 可达节点时，iroh/ihor-blobs 放在哪个 crate；
2. 设备上行（upload）与 hub 下发（download）的具体协议流；
3. hub 侧的 iroh 节点如何监听中继连接；
4. 与 CAS store 的集成点（不走 iroh 直接写 CAS，hub 侧转换一次）。

## 进场时核实的事实（2026-09-21）

crates.io 稳定版（iroh 1.x 线）：

| Crate | 版本 | 许可证 | MSRV | 备注 |
|-------|------|--------|------|------|
| `iroh` | `1.x`（minor 锁 Cargo.lock） | MIT OR Apache-2.0 | 1.75 | 含 iroh-net（中继/打洞）+ iroh-docs |
| `iroh-blobs` | `1.x`（与 iroh 同 minor） | MIT OR Apache-2.0 | 1.75 | BLAKE3 验证流；`baomars` 擦除编码自验 |

- 两 crate 均无传递 unsafe 边界（`iroh-blobs` 内部 `baomars` 含 unsafe，
  但对外 API 无 unsafe——与 reed-solomon-erasure 同模式，deny 无额外豁免）；
- 许可证均在 deny.toml 白名单内，进场 cargo deny 零新增白名单条目；
- `iroh-blobs` 的 `iroh_blobs::protocol::ALPN` 协议字节已固定，可与
  其他 iroh 1.x 节点互通（无需变更 ALPN 即可跨实现互操作）；
- hub 侧仅用 `iroh`（中继/节点发现）+ `iroh-blobs`（块流），不需要
  `iroh-docs`（directional sync 功能本 repo 自己实现）。

## 决策

### 1. Crate 分布：hub 听、transfer 送、cas 不直接碰 iroh

```
设备
  │  iroh QUIC 连接（打洞/中继）
  ▼
partisync-hub  (iroh 节点监听 + iroh-blobs 接收/发送)
  │  ChunkStore trait（不传递 iroh 依赖）
  ▼
partisync-cas  (散块存储 / 聚合器 / pack v2)
  ▲
  │
partisync-transfer (iroh-blobs sender 执行器，execute_plan 调用方)
```

- **`partisync-hub`**：新增 `iroh` + `iroh-blobs` 依赖；实现 `iroh`
  节点监听（hub 永远是 reachable relay endpoint），以及
  `iroh-blobs` 接收（设备上传，写 CAS）与发送（设备下载，从 CAS 读）
  两个会话角色；
- **`partisync-transfer`**：`chunk_plan::execute_plan` 的 sender 闭包绑定到
  `iroh-blobs` sender——不直接依赖 iroh crate，通过 trait object 间接调用；
  与 S3/OpenDAL 执行器共调度器 + 限速器（ADR-0009 裁定 5）；
- **`partisync-cas`**：`store` / `pack` 不引入任何 iroh 依赖——通过 hub
  暴露的 `ChunkSink` / `ChunkSource` trait 接受/提供块数据（trait 本身
  在 `partisync-core` 或独立 `partisync-transfer`）。

> **依赖方向注解**：hub 依赖 cas 是既有架构（cas 在领域层，hub 在能力
> 层）；transfer 不依赖 hub（跨层反向依赖需 ADR——本设计不引入此问题，
> transfer 的 sender 由 hub 构造后注入）。

### 2. Hub 监听角色：iroh Relay Endpoint

Hub 启动时初始化一个 `iroh::Node`（本地密钥存储在 fjall `h-iroh-node` keyspace，
重启可复现节点 ID）：

- **公开地址**：通过 iroh-net 的 DNS 句柄公告（`iroh::Node::addr` 返回
  可分享的 `iroh::NodeAddr`——含公钥 + 中继 URL）；
- **连接模型**：设备主动连 hub（设备侧发 QUIC client，hub 侧 accept）；
  hub 不主动连接设备——打洞仅在设备↔设备场景由 iroh-net 中继承接；
- **中继语义**：hub 的 iroh 节点开启 `iroh::node::Config::enable_relay`；
  设备 NAT 穿透失败时自动走 iroh relay（后台透明重试，不阻塞上传）；
- **下行监听**：hub 还暴露一个 `iroh-blobs` 协议处理器——设备可主动
  请求（request）特定 hash 的块列表，hub 从 CAS 读出并流式响应
  （`iroh_blobs::protocol::write_blob`）。

### 3. 设备上行协议（Upload Path）

```
设备侧                          Hub 侧
  │                               │
  │  iroh::Node::connect(hub_addr) │
  ├───────────────────────────────▶│
  │                               │
  │  iroh_blobs::protocol::write_blob（流式 BLAKE3 验证）
  │  ────────────────────────────▶│  iroh_blobs::protocol::accept_and_read_blob
  │   chunk[0] + hash_partial      │   接收窗口自适应 BDP
  │   chunk[1] + hash_partial      │   写满 X MB 后
  │   ...                          │     → ChunkSink::put_chunk(chunk)
  │   last_chunk + BLAKE3 root     │     → 原子核销引用计数 +1
  │                               │
  │  ◀──────────────────────────────│  整包收完 → 触发聚合器（满/过期）
```

- Hub 侧 `iroh-blobs` 接收会话把每接收到的 chunk 调用
  `ChunkSink::put_chunk(&[u8])` —— 该 trait 由 CAS store 实现，
  不传 iroh 类型；
- 引用计数 incr 在 chunk 落 CAS 散块区时原子执行（store 的既有语义）；
- 设备上传完毕（BLAKE3 root 验签通过）后，hub 侧发送 `UploadAck`
  （自定义 iroh-blobs 协议外的应用层 ACK，通过 `iroh::connection::Method`）；
- 断线续传：设备重连后重新发送缺失 chunk 列表（ChunkPlan 差分协议，
  ADR-0009 裁定 2），hub 侧幂等处理（重复 hash 的 put_chunk 走 CAS
  的幂等语义）。

### 4. 设备下行协议（Download Path）

```
Hub 侧                          设备侧
  │                               │
  │  设备发送 WantHashes { [hash0, hash1, ...] }
  │  ─────────────────────────────▶│
  │                               │
  │  收到 WantHashes
  │   → ChunkPlan 差分（已有实现）
  │   → 对每个 Need 块：
  │     iroh_blobs::protocol::write_blob(chunk)
  │  ◀──────────────────────────────│
  │                               │
  │  设备 BLAKE3 验签（窗口级）
  │  验签失败 → NACK + 重发请求（应用层重试，不走 iroh 重传）
```

- Hub 下行时从 CAS store 按 hash 读 chunk（`ChunkStore::get`），然后
  通过 iroh-blobs 流式发送；
- 与现有 `chunk_plan::execute_plan` 集成：sender 闭包内部调用
  `iroh_blobs::protocol::write_blob`；
- 限速通过 `execute_plan` 所在的 scheduler 限速器（ADR-0009 裁定 5，
  iroh-blobs 与 S3/MPU 共限速配额）。

### 5. Hub 侧 iroh 节点初始化（代码层面的模块位置）

新增 `partisync-hub/src/iroh_channel.rs`（模块），导出类型：

```rust
// 伪代码，示意 API surface
pub struct IrohHubNode { /* iroh::Node 句柄 */ }

impl IrohHubNode {
    /// 从 fjall 加载或新建节点，绑定中继，开始监听
    pub async fn new(store: &dyn CasStore) -> Result<Self>;

    /// 在后台任务接受 iroh-blobs 连接，分发到 upload/download handler
    pub async fn run(self) -> Result<()>;
}

/// Hub → CAS 的块接收 trait（iroh 侧不感知）
pub trait ChunkSink: Send + Sync {
    fn put_chunk(&self, chunk: &[u8]) -> impl Future<Output = Result<(), Error>> + Send;
}

/// Hub ← CAS 的块发送 trait
pub trait ChunkSource: Send + Sync {
    fn get_chunk(&self, hash: &str) -> impl Future<Output = Result<Bytes, Error>> + Send;
}
```

> `ChunkSink` / `ChunkSource` trait 放在 `partisync-transfer`（被 hub 依赖）
> 而非 `partisync-cas`——避免 cas 引人 iroh 传递依赖。

### 6. 密钥与配对（沿用 ADR-0008 裁定 2）

Hub 的 iroh 节点密钥派生：
- 节点私钥 = `hub-iroh-sk`（存储在 fjall keyspace `h-iroh-node`）
- 派生自 hub 自身的 master key（ADR-0010 E2EE 栈）；
- Hub 助记词身份（ADR-0008 裁定 2 的设备侧）不用于 hub 自身——
  hub 节点以公钥标识（`iroh::PublicKey`），设备通过 QR code 或 URL
  获取 hub 的 `iroh::NodeAddr`（含公钥 + 中继 URL）。

设备配对流程（ADR-0008 裁定 2）不变：助记词 → Ed25519 → ECDH 挑战；
配对成功后 hub 把设备公钥存人 device 表（已有字段），并建立
持久化 iroh 连接会话（会话密钥材料不持久化，按需重建）。

## 备选

- **hub 不做 relay**：hub 纯客户端，设备直连——在 NAT 场景不可达，
  否决；
- **iroh 直接写 CAS 而不过 trait**：cas 被 iroh 依赖穿透，
  未来换传输方式代价大，否决；
- **hub 用 libp2p 替代 iroh**：ADR-0008 已否决 iroh → libp2p 路径；
- **iroh-blobs 以外协议传块**：BLAKE3 验证流是 ADR-0009 核心决策，
  换协议等于重做 ADRs 0008/0009，不符合执行效率。

## 后果

- `partisync-hub/Cargo.toml` 新增 `iroh` + `iroh-blobs` 依赖（minor 锁定）；
  cargo deny 复核（预期零新增白名单条目；如有传递许可问题，
  随本 ADR 修订登记）；
- `partisync-transfer` 新增 `ChunkSink` / `ChunkSource` trait——属公共
  API，变更需走 ADR；
- Hub 节点 ID 持久化于 fjall——新增 keyspace `h-iroh-node`，加
  keyspace 总量（影响 keyspace 水位评估，SPEC M3-WP04 裁定 7 评估
  项按需修订）；
- 中继带宽成本由 hub 运营方承担——iiroh-net relay 透明，不单独计费
  （iroh 1.x 定价模型待上场前确认；本 ADR 登记为开放项）。

## 待确认项（进场前填实）

| 项 | 负责 | 状态 |
|----|------|------|
| iroh 1.x relay 定价模型（hub 作为 relay 节点的费用承担方） | @lead | 待确认 |
| `iroh-blobs` 与其他 iroh 1.x 实现（如 iroh.com official）的 ALPN 互操作性 | @lead | 进场核实 |
| 设备侧 iroh-blobs 上传窗口大小（影响 BDP 吞吐） | @lead | 进场实测 |
