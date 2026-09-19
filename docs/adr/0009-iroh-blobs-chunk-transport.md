# ADR-0009: 块级传输以 iroh-blobs 验证流 + 泛化 delta 协议为底座

状态: 提议 · 日期: 2026-09-19 · 决策人: @lead（AI 代理起草，人工签核待补）
关联: SPEC M2-WP05、调研方案 §5.8（泛化 delta）

## 背景
M2-WP05 把"谁有什么"的对账产物落为块级差异，跨大文件/海量小文件都得
高效传输。调研方案 §5.8 既定 iroh-blobs（验证流）+ 泛化 delta（rsync 思想搬到
对象存储时代）。

## 决策
1. `iroh-blobs 1.x`（与 ADR-0008 同 minor；blocks 传输 + BLAKE3 验证流）。
2. 泛化 delta 协议 v1：接收方先送「我有的块哈希清单」（BEP 块交换 +
   casync 种子思想合体）；发送方按缺失块清单流式推送。
3. 块定义：复用 M0-WP03 fastcdc 分块（ADR-0004，min=256K avg=1M max=4M）——
   内容定义分块天然支持 delta；对小文件（<256K）退化为整文件传送。
4. 调度：分块流水线并发（BDP 补偿）+ 令牌桶限速（rclone --bwlimit 同语义）。
5. 隔离层：`partisync-transfer::chunk_plan::ChunkPlan`（现有 transfer crate 内
   已有雏形）→ 新增 `iroh_executor::IrohBlobsExecutor`，与现有 S3/OpenDAL
   执行器同接口（同一调度器+限速器消费）。

## 备选
- 自写 rsync 风格算法：能控但工作量大（分块+校验+流控），否决；
- BEP v1 块交换协议原生实现：与 iroh 集成不深，需自建 TCP 层，否决；
- 仅依赖 content_hash 整文件传送：50GB 文件每次同步全量，不可行。

## 后果
- iroh-blobs 与 iroh 同锁版——一处升级两者同步迁移；
- BLAKE3 验证流零额外内存压力（块窗口自适应 BDP），符合 §5.8；
- 与 §5.8 "目标侧 PartiSync 代理退化路径"保留：通过 caps 声明，对方选择
  是否走 iroh-blobs（不支持时走大文件整文件传）；本 WP 仅实现 iroh 路径。

## 重新评估条件
- iroh-blobs 1.x 性能/内存曲线与 casync 差距 >2× → 评估 casync 替代；
- 移动端 M6 实测端到端 5min 收敛不可达 → 评估流控参数 + 块大小调整。