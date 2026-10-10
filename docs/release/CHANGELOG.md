# Changelog

本文件由 AI 从 PR 集（Task-ID 追溯）起草、人终审后随 Release 发布
（SPEC docs/specs/M9-WP06.md §2.2-5；判例 M8-WP02 §2.4）。格式沿
Keep a Changelog 约定；semver pre-release 口径见各版本节。

## [0.2.0-rc.1] — 草稿（待用户终审后随 Release publish；tag 前另有一道终审，cadence §3.2-3/4）

> 口径（已确认 2026-10-10，M11-WP01-T01）：tag `v0.2.0-rc.1`、产物
> 文件名带 `-rc.1` 后缀、**workspace 版本真 bump 0.1.0→0.2.0**（整数
> 版，pre-release 只在 tag 与产物名）、五 bin 入包（新增
> `partisync-mcp-http`）、strip 实施、桌面 dmg 随发。本节范围 =
> `v0.1.0-beta..main`（M9 后半 + M10 全程 + M11-WP02/WP05，2026-10-05
> → 2026-10-10）。

### Added

- **MCP 2.0 正式远程化**：Streamable HTTP + rustls TLS + 真实 OAuth
  2.1 RS（jsonwebtoken RS 验签 / aud RFC 8707 受众绑定 / scope 步进 /
  P23 fail-closed 授权矩阵）+ PRM 真实化 + 11 工具透传（stdio/远程
  同面一致性探针）；mock AS 全链 e2e（ADR-0031）。真实 IdP 端到端 =
  NB-WP05-1 条件触发。（M10-WP05）
- **可验证记忆层二期**：tombstone 生命周期（软删不动根 / update 换身
  份 / 复活 / `memory-gc` CLI）+ MCP `memory_update`/`memory_delete`
  工具（工具面增至 11）+ schema v18 content_chunk 落表 + reindex 重
  组回退（NB5/P22）；语义向量检索条件评估件（拍板暂缓，T1–T4 触发纪
  律；T2 查询实证 2026-10-10 已归档、T3 维度冒烟实测 BGEM3=1024d）。
  （M10-WP04）
- **桌面可用性三期**：检索「形同虚设」修复（真实文件名透出 + 命中词
  高亮 SnippetGenerator + 结果-详情联动 + 扩展名 chips + 空态引导 +
  索引徽标）；记忆面板深化（表头排序 / tag chips / 相对时间四档 +
  >30 天回落 / 详情联动）；浏览表头三态排序 + mtime 相对时间。（M10-
  WP01/WP02/WP03）
- **partisd 底座常驻守护进程**（M11-WP02，ADR-0032）：loopback MCP
  服务面（与 stdio sidecar 同工具面）+ pidfile 防双实例 + SIGTERM
  优雅退出；索引只读打开路径（新不变量 P24：多读者只读 + 按需写者，
  D5 全消费者抢写锁根因消除）；新装首启自动引导空索引。
- **FUSE 写事件面 + 装配层**（M9-WP01）：`partifuse --graph` 写事件 →
  graph/oplog 装配 + bisync tick。
- **扩展签名工具链**：EXT-SIGNING 文档 + 生产钥签名窗口登记（示例扩
  展签名 = 持有人动作）。（M10-WP06）

### Changed

- **workspace 版本 0.1.0 → 0.2.0**（整数版；51 处/15 文件 + tauri +
  2 处 clientInfo；wit 扩展接口版本域不变）；产物名 `-beta` →
  `-rc.1`；linux/macOS 包新增 `partisync-mcp-http`；打包前 strip。
- **桌面 GUI 进入维护冻结**（D19 定位裁决：partisync = 底座
  （CLI+库+MCP），Partiverse = 唯一消费者 GUI；本 release 为冻结前
  最后功能面，后续由 Partiverse 接替）。

### Known limitations（如实登记，不假绿）

- 远程 MCP 授权为 mock AS 边界（真实 IdP 端到端 = NB-WP05-1 条件触
  发）；loopback partisd 面无鉴权（127.0.0.1 信任域，ADR-0032 决策 5）；
- 桌面记忆编辑/删除 UI 未做（能力经 MCP memory_update/delete 提供；
  GUI 入口归 Partiverse，D19）；
- 语义向量检索暂不实施（T2 实证已归档、T3 维度实测 1024d；实施立项
  待 T1 语料积累 ≥10⁵）；
- 索引中文摘要为 CJK fan-out 碎片（命中词高亮对中文近似失效；stored
  原文域代价评估已绿、实施随检索深化 SPEC）；
- CAS 存量 read_errors 维持；跨进程索引 reader 自动 reload 未生效
  （M11-WP02 §6-7，改进债）；
- macOS dmg 未公证（Apple Developer 外部依赖）；hub 产物仍为演示面；
  macOS 无 FUSE；
- 桌面 GUI 维护冻结注记：功能不再新增，仅必需维护（D19）。

## [0.1.0-beta] — 草稿（待用户终审后随 Release publish）

> 口径推荐案：tag `v0.1.0-beta`、产物文件名带 `-beta` 后缀、**含
> FUSE 写回**——待用户确认后执行发布（SPEC §2.1；确认前不 tag、不
> 建 Release）。本节范围 = `v0.1.0-alpha..main`（64 commits，2026-10-01
> → 2026-10-05）。

### Added

- **FUSE 挂载写回二期**：整文件替换 overlay（EBUSY 并发防护 +
  truncate/flush 同步语义）；unlink/rmdir/rename 经写回日志（JSONL
  WAL，幂等重放 P16，crash 矩阵 proptest）；`/by-hash` 只读命名空间
  （CAS 直连，EROFS，readdir 受限）；容器 musl 1.94 全套件复测 PASS。
  （M8-WP07）
- **写路径闭环**：`partisync-fuse --graph` 挂载写事件 → graph/oplog
  装配层 + `--peer` bisync tick；P19 装配探针；桌面同步水位改
  `sync_watermark` 派生（F1 清偿）；容器真挂载 e2e PASS。（M9-WP01）
- **可验证记忆层一期**：memory 资产 schema v16（ADR-0029）；
  MCP 三工具 `memory_write` / `memory_search`（FTS5 trigram + LIKE
  兜底）/ `memory_verify`（RFC 6962 式 Merkle inclusion proof，P20）；
  双端 bisync 收敛；桌面「记忆」浏览 tab + 旗舰检索「含记忆」分区
  通道。（M9-WP02/WP03）
- **桌面深耕**：语义检索旗舰 `search_hybrid` + 详情面板 + 同步 tab +
  扩展调用历史（M8-WP05，设计语言 v4.3）；`partisync reindex` 产品
  搜索索引重建命令；默认数据目录对齐 `~/.partisync`（与 CLI/gateway
  同根）；UI 转义硬化（innerHTML 插值统一 `esc()`）；转写开关诚实化
  （`include_transcript` 显式接线）。（M8-WP05 / M9-WP03）
- **扩展安全面**：扩展装载期强制验签（minisign 双钥锚定，缺签/坏签
  先于编译拒绝，P21）+ epoch/fuel 双机制强杀终止（M8-WP06）；分发者
  签名流程文档 EXT-SIGNING。（M9-WP04）
- **Hub 增量**：多租户最小面（r-tenant 归属 + 路由可见性过滤）；
  audit 链式 JSONL + 配额（软告警）；扩展来源侧账本。（M8-WP03）

### Known limitations（如实登记，不假绿）

- 挂载写为 tick 轮询（非实时）、单挂载点、max-delete 阈值防御误删
  （SEMANTICS 登记边界）；
- hub 产物仍为 `hub-demo-web` 演示面（正式服务 bin 未立）；
- Hub 远程 MCP 端点为评估骨架（mock Bearer，不承诺生产可用）；
- macOS dmg 未公证（Apple Developer 外部依赖）；产物文件名带
  `-beta` 后缀以区分 alpha。

## [0.1.0-alpha] — 2026-10-01

首个签名可分发版本（tag `v0.1.0-alpha`→`84c342d`，release run
36889384528）：CLI（linux x86_64 / macOS aarch64 tar.gz）+ hub-demo-web
（linux 包内）+ PartiSync Desktop dmg（macOS aarch64，Tauri）；全部
产物 minisign 双钥签名 + SHA256SUMS；cargo auditable 依赖嵌入；G4
检查单见 docs/reviews/M8-WP02-release-report.md。
