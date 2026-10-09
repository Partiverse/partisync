# ADR-0032: partisd 常驻形态（前台 + pidfile 生命周期）、loopback MCP 服务面复用、索引单写多读（只读打开 + 按需写者）

版本: 0.1 · 状态: **草稿**（随 SPEC M11-WP02 批准生效——合入 = 批准，
沿 ADR-0030/0031 判例；批准人 @lead）· 关联: SPEC docs/specs/M11-WP02.md、
M11-roadmap-proposal §3-WP02、ADR-0024（桌面运行时）/ADR-0031（MCP2
远程接线，决策 5 引用其机制）、partiverse docs/05-...md §9.3（S1 需求
源头）· 负责人: @partiverse · 批准人: @lead · 起草日期: 2026-10-09

## 背景

D19 定位裁决（partiverse 仓 Owner 已拍板）将 partisync 固化为底座
（CLI + 库 + MCP 永久），并把「partisd 守护进程实体化」列为 S1 首推
（替换 M10 GUI 容量；Partiverse V2 集成接缝 + V4 远程引擎前提）。现状
三事实：①partisd 为 8 行 println 骨架（零依赖零能力）；②D5——
partisync-index 唯一打开入口无条件建 tantivy IndexWriter，任何第二消
费者（含纯只读查询）撞写锁得 LockBusy（复现实证归档
docs/reviews/M11-WP02-d5-repro-evidence.txt）；③tantivy 0.26.2 原生
单写多读（writer 锁 index.rs:539-563 非阻塞独占；reader 跨进程安全
directory_lock.rs:52-59 META_LOCK；`Index::open_in_dir` 不取写锁
index.rs:498-504）。

硬约束：铁律 8（新依赖须 ADR + cargo deny）、crate 地图方向（壳→能
力层合法）、D19 桌面冻结令（桌面侧零改动面）、红线（deny 白名单不改，
拦截即回本 ADR 修订）。

## 决策

1. **partisd 形态 = 前台常驻进程 + pidfile/SIGTERM 生命周期**。组装
   graph/CAS/index 并暴露服务面；`--pid-file` 写 pid + 独占锁防双实
   例；SIGTERM/SIGINT 优雅退出。**否决** fork/daemonize 双化（信号与
   日志重定向复杂度，无用户诉求）与 systemd/launchd 深度集成 MVP
   （部署交 unit 文件文档示例，安装器 = 非目标）。
2. **索引并发模型 = 多读者只读打开 + 按需写者**。partisync-index 新增
   只读打开路径（不获取 `.tantivy-writer.lock`）；读消费者（sidecar/
   desktop/CLI search/partisd）一律只读；写者归真写面（CLI
   reindex/watch）按需短暂持有；写者互斥保持 tantivy 原生锁，锁冲突
   错误结构化并附指引。**否决**「daemon 独占写者」（阻塞 bulk
   reindex，需作业代理超 MVP 面）与「跨进程共享 writer」（tantivy
   writer 单进程，跨进程共享 = IPC 作业代理，同前超面）——两者均转
   后续 WP 条件候选。新不变量 **P24**（只读打开不获取写锁；N 读者 +
   ≤1 写者并发无读侧 LockBusy）随 SPEC T02 登记，先于测试代码。
3. **服务面传输 = 复用 rmcp StreamableHttp 绑定 127.0.0.1**（默认
   127.0.0.1:7650），暴露与 partisync-mcp 同一套 MCP 工具面，组装复
   用 gateway `build_server_state`（partisd 依赖 partisync-gateway，
   壳→能力层合法；**零新依赖**）。**否决** 自研 UDS JSON-RPC（新协议
   面 + rmcp 无 UDS transport，违背无聊依赖）与 gRPC（重依赖）。
   对齐收益：Partiverse V3 计划以 MCP 消费底座聚合面（其 docs/05 §5），
   partisd 即该接缝的常驻形态。
4. **partisync-mcp（stdio sidecar）保留不废**：桌面/CLI 按需拉起的
   stdio 面与 partisd 的 loopback 常驻面共用 `build_server_state` 组
   装；二者并存 = 同库多读者，无互斥（索引只读化后），语义文档化于
   SPEC §2.2。**否决**「partisd 替代并删除 sidecar」（桌面/CLI 按需
   面仍有价值，且桌面冻结期不重构其拉起机制）。
5. **安全边界 = 默认 loopback 信任域**：默认绑定 127.0.0.1、无鉴权；
   红线：默认配置不得绑定非环回地址；远程/认证沿 ADR-0031 既有机制
   另线（不在本 ADR 范围重复设计）。

## 后果

**正面**：D5 根因消除（读面永不互斥，复现路径回归即探针红）；底座获
得常驻形态（S1 兑现起步），Partiverse V2/V3 集成与 V4 远程引擎有接
缝；MCP 服务面复用使 partisd 与 sidecar 同工具面（一套面两形态）；零
新外部依赖。

**负面-风险**：partisd 引入进程生命周期管理新地（pidfile/信号/双实例
语义需探针钉死）；依赖 partisync-gateway 使 partisd 依赖树变重（壳层
可接受）；loopback 无鉴权面若被误绑非环回即暴露（红线 + 默认值防呆）。

**中性**：CLI reindex/watch 行为不变（写者按需）；桌面壳改动仅打开方
式（冻结令下的「必需维护」范畴）。

## 修订登记

| 版本 | 日期 | 说明 |
|---|---|---|
| 0.1 | 2026-10-09 | 初稿（随 SPEC M11-WP02-T01） |
| （预留） | | partisd 写者/作业面立项时修订（决策 2 的按需写者扩展） |

## 待批准项

- [ ] 决策 2 的 P24 登记行（随 SPEC T02，先于测试代码）
- [ ] 默认端口 7650 占用冲突核查（T04 实施前 `ss -tln` 实证回填）
- [ ] 决策 3 零新依赖复核（partisd Cargo.toml 随 T04 落地后 cargo deny 绿）
