# Changelog

本文件由 AI 从 PR 集（Task-ID 追溯）起草、人终审后随 Release 发布
（SPEC docs/specs/M9-WP06.md §2.2-5；判例 M8-WP02 §2.4）。格式沿
Keep a Changelog 约定；semver pre-release 口径见各版本节。

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
