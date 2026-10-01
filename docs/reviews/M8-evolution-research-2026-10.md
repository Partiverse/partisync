# PartiSync 演进方向深度调研（2026-10）

> 任务: M8-WP00-T02（滚动登记）· 日期: 2026-10-01 ·
> 性质: 调研工件（非 SPEC——方向采纳须走 partisync-spec-draft 批准）·
> 输入: 内部进度盘点（M8-WP00~WP04 全量交付上下文）+ 外部竞品实测调研
> 两路（同步/备份产品线、AI 记忆与扩展生态线；来源 URL 见各节）·
> 负责人: @lead

## 1. 内部进度盘点（2026-10-01 快照，main = 7c53d90）

八层能力全通且有实测锚点：

| 层 | 交付 | 锚点 |
|---|---|---|
| 本地引擎 | 10⁶ 索引 / CAS / pack v2 / EC / 分层 / GC | M0-M3 关账 |
| 同步 | E2EE / bisync / Merkle 对账 / iroh / UploadAck | M2 + M5-WP01（P99 8.35ms） |
| Hub | raft 分片 / 联邦路由 / 云事件流 / **线性一致语义兑现（P17/幂等/崩溃矩阵 M1-M6/M4-M6 探针）** / **链式审计日志** | M3-M5 + **M8-WP04 关账（PR #65-#71）** |
| 检索 | BM25+向量 RRF / LCSTS 真档 | Recall@10 = 0.95（M6-D67） |
| AI 面 | sidecar（OCR/Whisper/嵌入）/ MCP 五工具 / C2PA | M4 关账 |
| 桌面 | Tauri 2 壳 / 窗口状态 / 扩展面板 | M6-WP03 + M7-WP01 |
| 扩展 | wasmtime 47 宿主 + 注权 manifest（P13/P14） | 47/47 探针（M7-WP01） |
| 挂载 | FUSE 一期（只读+顺序写，P15） | M8-WP01 验收（PR #54-#58） |

M8 进度：WP00 ✅ / WP01 ✅ / WP04 ✅ / WP03 实施（T01 ✅ 审计）；WP02/05/06/07 待实施。
未结阻塞：WP02-T02 密钥生成仪式（需双人在场）。

## 2. 竞品实测（2026-10 时点）

### 2.1 同步/资产产品线

- **Spacedrive**：V2 重写（alpha.2，2026-02）后**资金断档停更**（种子 $2M 无续，
  [Releases 公告](https://github.com/spacedriveapp/spacedrive/releases)）；
  官网转向「local-first + AI employees」。教训：无收入先重写 = 死局。
- **Ente**：Photos 达 v1；on-device AI（Ensu 本地 LLM、Gemma 4、ML 索引 10×）；
  信任工程三件套 = Rust crypto 独立评审 + 开放经营数据 + 自托管
  （[ente.com/blog](https://ente.com/blog/)）。
- **Proton Drive**：SOC 2 Type II → SDK 统一多端 → 官方 CLI（2026-06）→
  Docs 套件化（[proton.me/blog/soc-2](https://proton.me/blog/soc-2)）。
- **Immich**：v3.0 Workflows（2026-07）+ v3.2 Search v2；FUTO 资助、无商业化
  （[v3.0.0 release](https://immich.app/blog/v3.0.0-release)）。
- **Syncthing**：v2.1.5 稳定（SQLite 迁移）；Android 官方停更、fork 治理动荡
  （维护者账号删除事件）——签名/多签分发必要性反证。
- **mountpoint-s3 / rclone**：语义边界未变（append-only 写 + 内存预算 +
  ENOMEM）；rclone 2026 一年 8+ CVE（serve/mount 面安全重灾区）
  （[releases](https://github.com/awslabs/mountpoint-s3/releases)、
  [changelog](https://rclone.org/changelog/)）。

### 2.2 AI 记忆与扩展生态线

- **AI 记忆层**：mem0（$24M）/ Letta / Zep 占云 API 段；**Merkle/CAS 可验证
  本地记忆仅学术 spec（Portable Agent Memory），零产品**；
  basic-memory 验证 local-first 需求但无同步与完整性层。
- **MCP**：官方 registry 靠 DNS TXT 验所有权、**不验 artifact 签名**；
  2026-07-28 spec（"MCP 2.0"）：Tasks/stateless core/OAuth 2.1 PRM（RFC 9728）/
  DCR 弃用（[官方公告](https://blog.modelcontextprotocol.io/posts/2026-07-28/)）。
- **WASM**：WASI 0.3 定稿（2026-06-11）+ Wasmtime 46+ 默认启用；
  **Wassette（component 即 MCP tool）先发且无 2026 竞品**、未 production-ready；
  签名分发收敛 cosign/OCI（[wasmcloud](https://wasmcloud.com/docs/v1/deployment/security/signing-webassembly-components-with-cosign-oidc/)）。
- **FUSE × AI 索引**：ragfs/BranchFS/Elastic FUSE-memory 先例刚起；
  「CAS 同步 + FUSE 挂载 + AI 索引」三合一成品未发现。
- **发布工程基线**：Tauri updater 内置 minisign + cargo auditable + cargo deny
  = 2026 单人开发者事实标准（与 ADR-0027 选型吻合）。

## 3. 战略结论

1. **格局**：Spacedrive 出局后，「跨设备文件域 + E2EE + 企业可审计 + 收入
   可持续」交叉点无人占据；照片域心智被 Ente/Immich 占据，不进入。
2. **企业化路径对标 Proton 顺序**：SOC 2 级证据（审计日志→配额→SSO）→
   CLI/SDK → 套件。PartiSync 审计（T01）与配额（T02）正沿此线。
3. **AI 检索**：on-device 小模型是 E2EE 约束下唯一路径（Ente 实证）；
   **文件域**本地语义检索是确认空位 → WP05 旗舰功能候选。
4. **扩展信任**：WASI 0.3 + MCP 2.0 双定稿窗口 6-12 个月；跟进
   cosign/OCI 对齐而非自建 registry（ADR-0025 线位天然兼容）。
5. **最大风险非竞品而是资金节奏**（Spacedrive/Immich 双反证）→
   WP02 发布实跑优先级高于一切新功能。

## 4. 演进方向建议（可行性 × 窗口排序）

| 方向 | 可行性 | 关键依据 |
|---|---|---|
| α 主线不变，WP02 发布前置 | 高 | SPEC+ADR-0027 就绪；密钥仪式为唯一阻塞 |
| WP05 桌面壳以「文件域语义检索」为旗舰 | 高 | 嵌入侧模型已验证；照片域外空位 |
| M9 重聚焦「可验证本地记忆层」 | 中高 | CAS+Merkle+MCP 组合零竞品；学术 spec 可引用 |
| WP06 后插入 cosign/OCI 扩展签名小步 | 中 | 窗口敏感；不自建 registry |
| 风险对冲：单人不做协作套件；企业线止于审计+配额+CLI | — | Proton Docs 级投入超带宽 |

## 5. 复核日志

- 外部调研两路（同步/资产线、AI 记忆与扩展线）由子代理实测产出，来源
  URL 已附；综合与可行性判断由主会话起草。
- 判断处均标注（推断）；采纳任一方向须走 SPEC 批准流程（铁律 1）。
