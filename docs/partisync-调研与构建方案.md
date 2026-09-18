# PartiSync：AI 时代数字资产传输与多端融合管理基建底座
## —— 调研与构建方案（万亿级文件规模设计）

> 版本：v0.1（调研稿） · 日期：2026-09-18
> 参照系：rclone / rsync / Syncthing / restic / kopia / borg / casync / Spacedrive / AList(OpenList) / oCIS / Nextcloud / Seafile / DVC / lakeFS / git-annex；协议：S3 / WebDAV / FTP·FTPS / SFTP / SMB / NFS / QUIC / BitTorrent v2 / MCP。
> 所有对外部系统的描述均附来源链接；所有规模推算均给出计算过程。

---

## 0. 摘要（TL;DR）

**结论先行：没有任何一个现有软件同时具备「协议广度（rclone）+ 传输效率（rsync）+ 多端融合建模（Spacedrive）+ 万亿级元数据底座（Tectonic/S3 级）+ AI 原生（MCP/语义索引）」这五项能力。PartiSync 的机会正在这条空白带上。**

- **rsync** 有最强 delta 传输，但单线程扫描、内存随文件数线性增长、无版本/去重/云支持；2026 年 8 月的 3.5.0 一个版本修复 33 个 CVE，暴露 C 代码库的安全压力。
- **rclone** 统一了 70+ 云存储后端，是协议适配的事实标准，但对云端**无 delta 传输**（改 1 字节重传整个文件）、**无内容去重**、**无跨 remote 全局索引**（每次操作重新 LIST）。
- **Spacedrive** 的 VDFS（虚拟分布式文件系统）验证了「统一资产图谱 + 跨设备内容寻址去重 + 域分离同步」的正确性，但单机 SQLite + 整文件传输的架构天花板在 10⁵–10⁷ 文件，且项目曾因框架依赖停摆 9 个月。
- **万亿级（10¹²）元数据**在工业界有成熟规律可循（Colossus、Tectonic、S3、Pangu、DanceNN、3FS）：**元数据离开单进程内存、进入分片分布式 LSM；平面 ID 命名空间与目录树分层；基数分层让最大的层最便宜；小文件打包；Merkle 反熵对账**。
- **AI 时代的新需求**是现有工具完全没有的：模型权重/数据集作为一等资产、跨设备语义检索、AI Agent 通过 MCP 直接管理与整理资产、数据集出口（DVC/lakeFS 语义）、生成内容溯源（C2PA）。
- **Rust 生态 2026 年已完全就绪**：OpenDAL（50+ 后端）、iroh 1.x（QUIC P2P + BLAKE3 验证流式传输）、fastcdc、blake3、tantivy、usearch、rmcp（官方 MCP SDK）、fuser、dav-server、libunftp——每一层都有生产级积木。

**PartiSync 一句话定位：** 一个以 Rust 编写的「数字资产底座」——对内把所有设备与云端抽象为统一资产图谱（PartiGraph）+ 内容寻址仓库（CAS），对外既作为客户端消费 S3/WebDAV/FTP 等协议，又作为网关把自己暴露成 S3/WebDAV/FTP/FUSE/MCP 服务；单机开箱即用，可扩展到自托管 hub 集群，元数据平面按 Tectonic/S3 的规律设计，支持联邦化达到万亿级聚合条目。

---

## 1. 背景与问题定义

### 1.1 AI 时代数字资产的四个结构性变化

1. **资产体积爆炸**：大模型权重（单 checkpoints 数十至数百 GB）、多模态数据集（TB–PB 级）、AI 生成内容（图像/视频/音频）成为新的主流资产类型。传统「文件夹+网盘」模型按文件大小和路径管理，对「同一模型的 500 个 checkpoint 之间 95% 内容相同」这类结构完全无感。
2. **终端形态裂变**：手机、笔记本、NAS、工作站、边缘盒子、云函数——资产天然分散在 5–10 类终端上。Apple/Google 用封闭生态证明「多端融合 + AI 检索」是刚需（Google Photos「Ask Photos」用 Gemini 做自然语言相册检索；Apple 端上做人物/场景识别），但两者都是单一厂商的封闭孤岛。
3. **AI Agent 成为新的「用户」**：2025–2026 年 MCP（Model Context Protocol）成为 Agent 接入工具的事实标准（官方 Rust SDK rmcp 已 3.4.0，spec 迭代至 2026-07-28 版）。文件系统是 Agent 最基本的操作对象——官方参考实现就是 filesystem server。**谁掌握资产的统一索引与操作面，谁就是 Agent 时代的文件系统。**
4. **可信与溯源**：C2PA 规范已到 2.4，Adobe 工具链、Chrome 移动端、OpenAI/Google 的生成内容都开始携带溯源清单（manifest）。资产管理基建需要原生理解与保留 C2PA。

### 1.2 现状痛点：五座孤岛

| 孤岛 | 表现 | 代价 |
|---|---|---|
| **协议孤岛** | 每种存储一种客户端：S3 用 aws cli、WebDAV 用 cadaver、FTP 用 lftp、手机用厂商 App | rclone 解决了「消费端」统一，但只做同步搬运，不做索引与融合 |
| **设备孤岛** | 手机照片、笔记本文档、NAS 影音、云端备份互不知晓 | 找一个文件靠回忆；同一份内容存 5 份没人知道 |
| **元数据孤岛** | 文件只有 path+size+mtime；EXIF/OCR/转写/语义信息散落在各种工具的私有库 | AI 想用数据时先要花数月做「数据考古」 |
| **规模孤岛** | rsync 内存爆、Syncthing 数据库迁移卡死、rclone 反复 LIST | 10⁷ 文件以上，所有个人级工具开始失效 |
| **AI 孤岛** | 索引在 Google/Apple 手里，本地文件对语义检索/Agent 不可见 | 数据在自己盘上，AI 能力却在别人的云上 |

### 1.3 PartiSync 定位与非目标

**定位**：数字资产传输与多端融合管理的**基建底座**（infrastructure），不是单一功能应用。形态是：一个无界面核心守护进程 `partisd` + CLI（`partisync`）+ 桌面/移动壳（Tauri 2）+ 可选自托管 hub（`partisync-hub`）。产品矩阵与 rclone 类似（先做引擎），UI 是壳不是核。

**明确非目标（v1 阶段）**：
- 不做协作编辑/在线文档（Nextcloud 人的主场）；
- 不做 POSIX 完整语义的分布式文件系统（JuiceFS/3FS 的主场）——PartiSync 的 FUSE 挂载采用 mountpoint-s3 式的「明确非 POSIX」语义 + 本地写回日志，诚实标注能力边界；
- 不承诺单命名空间 10¹² 文件装在一台机器/一个用户库里（见 §4.1 的诚实推算），万亿级指**联邦聚合条目规模**与 hub 集群设计上限。

---

## 2. 对标软件深度调研

### 2.1 rsync：delta 传输的鼻祖及其局限

**算法**（Tridgell 博士论文 + [官方 tech report](https://www.samba.org/rsync/tech_report/node3.html)）：接收端把基准文件切成固定块（默认 128 KiB），每块算弱滚动校验（Adler-32 族）+ 强哈希（3.0 起 MD5），发送端滑动窗口匹配，只传「块引用 + 未匹配字面量」。效果：改一个字节只传增量——但**代价是两端都要完整读一遍基准文件做校验**，即「没变化也要全盘 I/O」。

**现状**：3.5.0（2026-08-13）一次性修复 33 个 CVE（[samba.org](https://www.samba.org/rsync/)、[LWN](https://lwn.net/Articles/1088759/)）。支持 zstd/lz4 压缩（3.2.0 起）、增量递归（3.0 起）。

**硬伤清单**（对万亿级设计都是致命的）：
- 原生协议无加密（靠 SSH）；单线程扫描；**内存随文件数线性增长**（数百万文件即数 GB RAM，[Resilio 分析](https://www.resilio.com/blog/rsync-large-number-of-files)）；
- 无版本、无去重、无内容寻址；delta 算法只在 rsync↔rsync 间有效，对对象存储完全无用；
- 每次运行重新 stat 整棵树，无变更日志/索引。

### 2.2 rclone：云存储协议统一的事实标准

**规模与架构**：单个 Go 二进制，官方宣称 **70+ 云存储后端**（[rclone.org](https://rclone.org/)）；最新 v1.75.1（2026-09）。架构 = 后端抽象层 + VFS 挂载层 + serve 协议服务层 + rc RPC 层。

**关键事实**：
- 子命令覆盖 copy/sync/bisync/move/check/lsf/mount/serve/ncdu/rc 等；支持服务端 copy（S3/Drive/B2/Azure 支持，Dropbox/FTP/Mega 不支持）。
- **变更判定**：默认 size+mtime，`--checksum` 需两端有共同哈希；各后端哈希五花八门（S3=MD5、B2=SHA1、OneDrive=QuickXor、Dropbox=私有、FTP=无），1.71.0 起加入 BLAKE3/XXH3（[changelog](https://rclone.org/changelog/)）。
- **虚拟 overlay remote**：crypt（客户端加密）、chunker（透明分片，默认 2 GiB 块）、union/combine（多 remote 合并命名空间）、hasher——证明「在传输工具里叠虚拟层」是社区真实需求。
- **serve 可对外暴露**：HTTP/WebDAV/FTP/SFTP/DLNA/**S3**/NFS/restic-REST——「把 A 协议存储变成 B 协议服务」是已被验证的杀手锏场景。
- `rclone mount` 需要 `--vfs-cache-mode writes|full` 才能随机写；macOS 依赖 FUSE-T/macFUSE。
- **bisync 已于 v1.71.0（2025-08）转正**：基于「上次运行的双侧 listing 快照」做双向 diff，`--conflict-resolve none` 默认保留冲突副本（`file.conflict1`）。

**硬伤清单**：
- **对云端无 delta 传输**：云端改 1 字节 = 重传整个文件（对象存储 API 没有「发我块哈希表」的动词，rsync 思想从未被泛化到云）；
- **无内容去重**（`dedupe` 命令只处理重名文件，不是存储级去重）；
- **无全局索引**：每次操作重新 LIST，`--fast-list` 只是缓解；万亿 key 的 bucket 上 LIST 本身就是灾难（见 §4.1）；
- sync 是单向快照；bisync 基于 listing 快照，重命名/空目录等边角问题文档自认不少。

### 2.3 Syncthing：P2P 持续同步

**协议**（[BEP v1 spec](https://docs.syncthing.net/specs/bep-v1.html)）：文件切 128 KiB–16 MiB 幂次块（默认取块数 <2000 的最小档），**逐块强哈希请求/响应（无滚动校验）**；每文件版本向量 + 每设备序号做一致性；**delta index 交换**（index-ID + max-sequence）让只传新增索引。NAT 穿透 = 全局发现服务器 + 中继池（中继只见密文，端到端 TLS）。身份 = TLS 证书 SHA-256。

**现状**：v2.0（2025-08）把数据库从 LevelDB 换成 SQLite（用户报告迁移数小时~数天），v2.1（2026-05）继续月度节奏。

**教训**：扫描是 I/O+DB 双瓶颈，50 万+ 文件的大文件夹普遍劣化；**本地数据库是单点瓶颈**——这正是「设备侧也要分层元数据引擎」的反面证据。

### 2.4 备份系：内容定义分块（CDC）的成熟度

| 工具 | 分块器 | 平均块 | 加密 | 关键痛点 |
|---|---|---|---|---|
| **restic** | Rabin 滚动指纹 | 1 MiB（512K–8M） | AES-256-CTR + Poly1305 | **prune 需独占锁 + 重写 pack**（S3 上又慢又贵）；仓库是「blob 汤」，无元数据查询 |
| **kopia** | BuzHash（可选 Rabin/fixed） | 10 MiB | 强制 AEAD（AES-GCM/ChaCha20） | GC/维护周期、索引 blob 膨胀 |
| **borg** | BuzHash | 2 MiB（512K–8M） | 仓库内去重，2.0 四年仍 beta | 全局锁下 prune 重写；**跨仓库不能去重** |
| **casync** | BuzHash | 64 KiB | 无 | 上游休眠多年；思路（种子文件 + 内容寻址块库）被 Proxmox PBS 继承 |

（来源：[restic design](https://restic.readthedocs.io/en/v0.5.0/Design/)、[kopia arch](https://kopia.io/docs/advanced/architecture/)、[borg notes](https://borgbackup.readthedocs.io/en/stable/usage/notes.html)、[casync 博客](https://0pointer.net/blog/casync-a-tool-for-distributing-file-system-images.html)）

**要点**：CDC + 内容寻址去重在备份仓库里已完全成熟，但**从未与「活同步拓扑」结合**——备份仓库是死的快照，同步工具（rsync/rclone/Syncthing）从不跨设备/云去重。把 CDC 去重放进同步/资产管理主路径，是空白。

### 2.5 Spacedrive：VDFS 的产品化实验（最重要的参照系）

**V2 白皮书**（[v2.spacedrive.com/overview/whitepaper](https://v2.spacedrive.com/overview/whitepaper)）把 V2 定义为「local-first、AI-native 的 VDFS」，五项创新：VDFS + 统一寻址（`SdPath`）、AI 原生架构、事务化操作（Preview→Commit→Verify）、**域分离库同步**、内容身份系统。

**值得整体继承的设计**（均有来源）：
- **数据模型**（[data-model](https://v2.spacedrive.com/core/data-model.md)）：`Entry`（自引用 parent）+ **`EntryClosure` 闭包表**（ancestor/descendant/depth，O(1) 子树查询）+ **`ContentIdentity`**（内容身份：快速采样哈希 + 完整完整性哈希；**多个 Entry 指向一个 ContentIdentity 即去重**，UUID 由 content_hash 全局确定性派生）+ `Location`（受监控目录，index_mode 分 shallow/content/deep）+ **`Volume` 作为所有权锚点**（移动硬盘换机器只改一行 device_id）+ Tag DAG（closure 表 + 关系强度）+ **Sidecar（VSS 虚拟旁车）**：缩略图/OCR/embedding 挂在 ContentIdentity 上——**一份内容只算一次 AI**。
- **域分离同步**（[library-sync](https://v2.spacedrive.com/core/library-sync.md)）：识别出文件系统数据天然有单一属主——**设备自有数据（Device/Volume/Location/Entry）单写者，属主状态永远权威，不需要 CRDT/共识**；真正共享的数据（Tag/UserMetadata/ContentIdentity）用 **HLC（混合逻辑时钟）排序的 oplog + LWW** 收敛。Oplog 存独立 `sync.db`，全对端 ACK 后裁剪（7 天保险期），保持 <1MB。
- **寻址**（[addressing](https://v2.spacedrive.com/core/addressing.md)）：`local://<设备slug>/<路径>`、`s3://<bucket>/<key>`、`content://<uuid>` 三态 SdPath；故意对齐 AWS CLI/gsutil 的 URI 习惯以获得复制粘贴级互操作。
- **网络**（[networking](https://v2.spacedrive.com/core/networking.md)）：V1 用 libp2p 被判「不可靠」，**V2 改用 [iroh](https://www.iroh.computer)（QUIC）**（当前 main 用 iroh 0.95）；Ed25519 节点身份 + BIP39 式助记词配对 + ECDH 会话密钥；传输 BLAKE3 校验 + 256KB 加密块。
- **作业系统**（[jobs](https://v2.spacedrive.com/core/jobs.md)）：持久可恢复作业（MessagePack 序列化状态 + 显式 checkpoint）作为索引/缩略图/embedding 管线的底座。

**必须避开的东西**：
- **框架依赖毁掉 V1**：prisma-client-rust 上游废弃后被迫 fork，libp2p 打补丁，整个 V1 判定不可维护（[history](https://v2.spacedrive.com/overview/history.md)）——V2 迁移到 SeaORM/sqlx。教训：**核心数据层用「无聊、稳定」的东西（sqlx + SQLite），虚拟层可以新潮，地基不行**。
- **样板代码经济学**：V1 每个文件操作 500–1000 行样板，功能速度崩塌；V2 用宏生成省 95%。教训：schema/操作层第一天就要为代码生成设计。
- **规模天花板**：一库一 SQLite、全文件传输（delta 明确列为 future work）、FTS5 搜索——架构上没有任何东西为 10⁹+ 设计，更没有 10¹²。
- **无协议网关**：数据只能通过自家 App/CLI 访问，没有 S3/WebDAV/挂载面——AList/CloudDrive2 的流行证明用户要的是「标准协议出口」。
- **项目现实**：$2M 种子轮（2022，[公告](https://spacedrive.com/blog/spacedrive-funding-announcement)），2025-03 因资金暂停（[讨论](https://github.com/spacedriveapp/spacedrive/discussions/2876)），V2 由创始人一人 AI 加速重写（v2.0.0-alpha.1 2025-12-26、alpha.2 2026-02-07，[releases](https://github.com/spacedriveapp/spacedrive/releases)）；当前 ~39k stars；许可证从 AGPL 转为 FSL-1.1-ALv2。**教训：治理与资金是架构能否活到成熟的前提。**

### 2.6 云盘聚合系与自托管同步

- **AList → OpenList**（[AList](https://github.com/alistgo/alist)、[OpenList](https://github.com/OpenListTeam/OpenList)）：Go 写的「几十种网盘聚合到一个 Web UI + WebDAV 出口」，driver 抽象（list/link/upload 三原语）。**局限：本质是 listing/代理层而非文件系统**——无离线索引（搜索依赖慢爬取）、无去重、无 delta、单节点内存缓存。2025-06 AList 转给不知名公司 + 疑似供应链投毒 PR 引发信任危机，核心开发者 fork 出 OpenList（~24.7k stars）。**教训：元数据模型（而非 driver 数量）才是这类产品的真正护城河；治理事件能瞬间改写格局。**
- **oCIS**（ownCloud Infinite Scale，现 Kiteworks 旗下）：单 Go 二进制、**space 化（每空间独立授权）+ 元数据优先（服务端不做 POSIX）**，是「空间 + 元数据优先」生产级先例；2025 年社区又 fork 出 OpenCloud。
- **Nextcloud/Seafile**：服务器中心化权威副本模型；Seafile 13.x 性能口碑最好。都不是 local-first。
- **CloudDrive2**：闭源，把网盘挂成本地磁盘（FUSE3），影视库场景流行；无统一元数据/AI。
- **Google Photos / Apple Photos**：索引 + AI 的规模参照（Ask Photos 的「先出结果、AI 总结第二」混合设计值得抄）；但封闭孤岛，不给开放索引。

### 2.7 能力矩阵与空白带

| 能力 | rsync | rclone | Syncthing | restic/kopia | Spacedrive V2 | AList | JuiceFS | **PartiSync 目标** |
|---|---|---|---|---|---|---|---|---|
| 云协议消费广度 | ✗ | **70+** | ✗ | S3 等 | S3 等主流 | 40+ | S3 | **OpenDAL 50+** |
| delta/块级传输 | **✓(最强)** | ✗ | ✓(块哈希) | ✓(CDC) | ✗ | ✗ | ✗ | ✓ CDC+泛化 delta |
| 跨设备内容去重 | ✗ | ✗ | ✗ | 仓库内 | ✓ | ✗ | ✗ | ✓ 全局 CAS |
| 双向同步/冲突 | ✗ | bisync(快照) | ✓ | ✗ | ✓(域分离) | ✗ | — | ✓ 事件+oplog |
| 全局统一索引 | ✗ | ✗ | 设备本地 | ✗ | 单机图谱 | ✗ | — | ✓ 分层索引平面 |
| 万亿级元数据 | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ~10¹⁰(TiKV) | ✓ 联邦 hub |
| 对外协议网关 | daemon | serve 全套 | ✗ | restic REST | ✗ | WebDAV | FUSE/NFS/S3* | ✓ S3/WebDAV/FTP/FUSE |
| AI 语义索引/Agent | ✗ | ✗ | ✗ | ✗ | 脚手架 | ✗ | ✗ | ✓ MCP+混合检索 |
| E2EE | SSH 隧道 | crypt | ✓ | ✓ | ✓(计划) | ✗ | ✗ | ✓ 空间级 |

*JuiceFS 有 S3 网关类组件但主形态是 POSIX FS。空白带清晰：**「rclone 的协议面 + rsync 的传输效率 + Spacedrive 的融合建模 + Tectonic 的元数据规律 + AI 原生」无一人占有。**

---

## 3. 通用协议与接口调研

### 3.1 S3：对象存储的世界语

- **硬限制**（[qfacts](https://docs.aws.amazon.com/AmazonS3/latest/userguide/qfacts.html)）：对象最大 5 TB；单 PUT ≤5 GB；MPU 分块 5 MB–5 GB、最多 10000 块；ListObjectsV2 每页 ≤1000 key，`ContinuationToken` 游标分页；预签名 URL 最长 7 天。
- **校验和体系演进（2022–2025 大变化）**：经典 ETag 对 MPU 是「各块 MD5 的 MD5 + -N 后缀」，**不能当文件哈希用**；2022-12 起支持 CRC32/CRC32C/SHA-1/SHA-256 附加校验和；**2024-12 起默认自动计算 CRC64NVMe**，MPU 区分 COMPOSITE/FULL_OBJECT 两种类型（[官方博客](https://docs.aws.amazon.com/AmazonS3/latest/userguide/checking-object-integrity.html)）。**设计含义：内部用 BLAKE3 做内容寻址，S3 边界映射 CRC64NVMe/SHA-256，永远不把 MPU ETag 当内容哈希。**
- **强一致性**：2020-12-01 起 GET/PUT/LIST 全部 read-after-write 强一致（[AWS 博客](https://aws.amazon.com/blogs/aws/amazon-s3-update-strong-read-after-write-consistency/)）——同步引擎可以建立在「写后立即可见」上。
- **每前缀限速**：每前缀至少 5500 GET/3500 PUT QPS（[性能指南](https://docs.aws.amazon.com/AmazonS3/latest/userguide/optimizing-performance.html)）——万亿对象 bucket 必须按前缀分片设计扫描。
- S3 兼容生态（MinIO/R2/B2/OSS/COS/Wasabi…）在「MPU + 预签名 + ListV2 + ETag」交集上兼容良好，附加校验和/事件通知参差不齐——**按交集编码，校验和按可选协商处理**。
- AWS 官方 Rust FUSE 客户端 **mountpoint-s3**（v1.24.0，GA）明确声明非 POSIX：只支持随机读与新建文件顺序写，不支持随机写/覆盖/truncate/改名/删除非空目录（[SEMANTICS.md](https://github.com/awslabs/mountpoint-s3/blob/main/doc/SEMANTICS.md)）——**这是「诚实标注语义边界」的最佳实践，PartiSync 的 FUSE 应该学。**

### 3.2 WebDAV：兼容网关，不是同步协议

RFC 4918 方法面（PROPFIND/MKCOL/COPY/MOVE/LOCK…，Depth 0/1/infinity）。现实坑位：ETag 是 "should" 不是 "must"（大量服务器不给）；nginx 核心 dav 模块无 PROPFIND/LOCK；无服务端校验和标准（Nextcloud 用私有 OC-Checksum）；目录 mtime 不可靠；MOVE 原子性无保证；OneDrive 的 DAV 端点已死。结论：**作为服务端兼容出口实现（Rust 有 dav-server 0.11 框架），不作为内部同步协议。**

### 3.3 FTP/FTPS/SFTP：遗留兼容层

FTP 双通道模型（PORT 主动/PASV 被动）+ NAT 原生不合；FTPS = 21 端口 AUTH TLS 显式 TLS（RFC 4217），990 隐式是事实标准非标准；MLSD（RFC 3659）修 LIST 的方言问题但嵌入式服务器支持参差。SFTP 是 SSH 子系统（OpenSSH 默认 v3），**pipelining 是性能命门**（OpenSSH sftp `-R` 64 个在途请求；无 pipelining 时吞吐 = 窗口/RTT）。定位：兼容清单项（Rust：libunftp 0.23 做 FTPS 服务端、russh 做 SFTP），不是设计目标。

### 3.4 SMB/NFS：局域网 POSIX 适配器

SMB3 Multichannel + AES-256-GCM 加密 + leases；NFSv4.1 delegations + pNFS 并行路径。**现实决策：Rust 原生 SMB/NFS 服务端栈不成熟，v1 不实现线协议**——用 FUSE（fuser）前置本地 POSIX 语义，需要 SMB 时桥接 Samba。列为远期 LAN 适配器。

### 3.5 现代传输层

- **QUIC**（RFC 9000/9001/9002）：流多路复用无队头阻塞、0-RTT 恢复（仅用于幂等请求）、**连接迁移**（WiFi↔蜂窝不断流——移动设备同步的天选特性）、内建 TLS1.3。
- **iroh**（n0-computer）：**1.0 已于 2026-06-15 发布**（"Dial Keys, not IPs"），当前 1.2.0（2026-09）；QUIC + 打洞 + 中继回退（中继只见密文），按 NodeId（Ed25519 公钥）拨号；**iroh-blobs 提供 BLAKE3 内容寻址 + 验证流式传输（bao outboard 编码，边下边验）**。与「内部哈希 = BLAKE3」形成完美闭环——**设备直连通道直接采用**。
- **BitTorrent v2**（BEP 52）：每文件 16 KiB 叶子哈希 Merkle 树——per-file 校验与跨 torrent 去重的思想可借鉴到分享链接场景。
- rsync-over-ssh 是单 TCP 流，受 BDP 限制不可并行——佐证新协议要用 QUIC 多流。

### 3.6 传输物理学三事实（决定引擎架构）

1. **带宽时延积（BDP）是吞吐的第一性限制**：100 Mbps × 200 ms RTT ≈ 2.5 MB 在途窗口；CUBIC 在高 BDP 丢包链路上崩溃。解药 = 并行流水线 + BBR/QUIC——**PartiSync 从第一天就按「分块流水线」设计，不做单流顺序传输**。
2. **syscall 开销**：每线程 ~1–2M IOPS 上限；Linux 用 io_uring（io-uring crate 0.7）摊销。
3. **零拷贝 + 哈希的分工**：sendfile/splice 省拷贝；**BLAKE3（SIMD + 多线程，多 GB/s）让哈希不在关键路径上**——内容寻址的 CPU 成本已不是瓶颈。

---

## 4. 万亿级规模：挑战、推算与行业规律

### 4.1 诚实的数量级推算

先明确「万亿级」的含义：**PartiSync 的万亿 = 联邦聚合**——一个 hub 集群或一个跨 hub 联邦所管理的内容条目（entry）与内容块（chunk）总量；单用户/单空间命名空间设计上限为 10⁹–10¹¹。理由如下：

**元数据容量**
- 每条目 ~300 字节（依据：JuiceFS/Redis 引擎经验值 ~300 B/inode；HDFS NameNode ~150 B/对象，但含块映射）：
  - 10¹² entry × 300 B = **300 GB 主索引**；LSM 写放大 ×3–5 → **~1–1.5 TB 盘上**。
  - 一个 5 节点 TiKV/fjall+Raft 集群可承载——**可行，但绝无可能放进单机内存**（对照：GFS 单 master 内存模型在 10⁷–10⁸ 文件就到顶；HDFS 单 NN 实践上限 10⁸–10⁹）。这就是「GFS→Colossus」弧线的必然性。

**全量扫描**
- 单机 stat 吞吐 ~10⁴ 文件/s：10¹² / 10⁴ = 10⁸ s ≈ **3.2 年**。
- 64 并行 worker × 5×10⁴/s = 2×10⁶/s → **5.8 天**——仍不可接受为周期性操作。
- **结论：增量事件驱动是必需品，不是优化**。本地靠 fs 事件（notify）+ 变更日志；云端靠事件通知（S3 SQS/webhook）+ 游标式增量 LIST + CheckpointCursor。全量扫描只在首次接入与定期对账发生，且必须可分片、可断点、可限速。

**云 LIST 数学**
- ListObjectsV2 每页 1000 key：10¹² key = 10⁹ 次请求；10 req/s 持续 → 3.2 年；即便打满单前缀 5500 GET/s 也要 **2 天纯 LIST**（且单前缀 QPS 封顶就在那）。
- **结论：对超大 bucket 必须按前缀分区并行 LIST + 事件流维护增量；把「LIST 全量」当异常路径而非常规路径。**

**内容块（chunk）索引**
- 假设平均文件 8 MB、CDC 平均块 1 MiB → 10¹² 文件 ≈ 10¹³ 块 × ~40 B（哈希前缀→引用）= **400 GB 块索引**——必须按空间分片 + 冷热分层 + 端侧用布谷鸟/布隆过滤器回答「见过这个块吗」。
- 端侧实际量级：个人设备 10⁵–10⁷ 文件 → 本地索引 30 MB–3 GB（redb/SQLite 完全可行）；NAS/工作站 10⁸ → 30 GB 级（fjall/rocksdb）。

**传输**
- 100 TB @ 1 Gbps ≈ 9.3 天；@10 Gbps ≈ 22 h。模型 checkpoint 保存一次改 5% 内容：CDC delta 下传 5%（≈ 4 GB/checkpoint），整文件重传下传 100%——**去重与 delta 对 AI 资产是数量级差异，不是百分比差异**。

### 4.2 工业界万亿级系统的规律（一手论文/工程博客）

| 系统 | 公开规模 | 元数据存储 | 关键技术 |
|---|---|---|---|
| GFS (2003) | 10⁷–10⁸/集群（内存上限） | 单 master RAM（<64 B/64MB chunk） | 64MB 大块、3 副本 |
| **Colossus** | 比 GFS 大 100×；几十 EB | **Bigtable 单元格**（Curators） | RS 纠删 ~1.5×、custodian 后台修复 |
| Haystack (2010) | 2600 亿照片 | 内存索引 ~10 B/needle | needle 打包、一次寻道一读 |
| **Tectonic** (FAST'21) | 样本集群 10.7 B 文件/1250 PB；全网 10¹³+ blob | **ZippyDB = 分片 RocksDB+Paxos**；Name/File/Block 三层哈希分片 | 平面 ID、基数分层、NVMe 条带化、RS(9,6)/(3,3)/(10,4) |
| Azure WAS (2011) | EB 级 | Partition 层 range 分区（Paxos） | Stream+Partition+FE、LRC(12,2,2) |
| **S3** | **400+ 万亿对象**（2024-12）、~150M req/s | 内部分片元数据服务 | 2020 强一致 LIST、前缀分片、游标 LIST |
| HDFS | 10⁸–10⁹/NN | NameNode RAM（~150 B/对象） | Federation/RBF、RS(6,3) |
| Ceph | 10⁹+ 对象/RADOS | CRUSH 算法化放置（无中心查找）；CephFS MDS 是命名空间瓶颈 | — |
| JuiceFS | Redis ~10⁸ inode；**TiKV 实测 10¹⁰** | Redis/MySQL/PG/TiKV | 64MB chunk→4MB block 对象化 |
| **Pangu 2.0**（阿里） | **~10¹³ 文件**、~100μs 延迟 | 元数据服务（range 管理 + 热点迁移） | RS 纠删条带、RDMA |
| **DanceNN**（字节） | 10¹¹ 文件（在线 NAS+离线 HDFS 共用） | range 分裂/合并的目录树 KV | 在线离线统一元数据 |
| **3FS**（DeepSeek） | 180 节点 6.6 TiB/s | **FoundationDB** | 无状态 MDS、RDMA+NVMe |

（来源：[Tectonic 论文](https://www.usenix.org/system/files/fast21-pan.pdf)、[Colossus 博客](https://cloud.google.com/blog/products/storage-data-transfer/a-peek-behind-colossus-googles-file-system)、[S3 400T 声明](https://press.aboutamazon.com/2024/12/amazon-s3-expands-capabilities-with-managed-apache-iceberg-tables-for-faster-data-lake-analytics-and-automatic-metadata-generation-to-simplify-data-discovery-and-understanding)、[Pangu FAST'23](https://www.usenix.org/conference/fast23/presentation/li-qiang-deployed)、[3FS](https://github.com/deepseek-ai/3fs)、[JuiceFS 元数据选型](https://juicefs.com/en/blog/usage-tips/juicefs-metadata-engine-selection-guide)、[HopsFS FAST'17](https://www.usenix.org/conference/fast17/technical-sessions/presentation/shvachko)、[Dynamo](https://dl.acm.org/doi/pdf/10.1145/1294261.1294281)）

### 4.3 六条规律（PartiSync 元数据平面的设计公理）

1. **元数据离开单进程内存，进入分片分布式 LSM/Raft KV**——所有突破 10⁹ 的系统无一例外（Colossus/ZippyDB/TiKV/FDB/DanceNN/NDB）。
2. **平面 ID 命名空间与目录树解耦**：平面 ID（可哈希分片、无热点）+ 单独的目录树层（可缓存、可复制）。Tectonic 的 Name/File/Block 三层是教科书。
3. **基数分层，最大层最便宜**：目录数 ≪ 文件数 ≪ 块数；块层条目最小、命中最多（Tectonic 2/3 元数据操作打在块层）。
4. **小文件打包**（Haystack needle / Tectonic 3GB block / git packfile / casync）：把 10¹³ 个小对象的元数据压力折算成 10⁹ 个 pack 的元数据压力。
5. **游标分页 + 前缀分片**是一切 LIST 的姿势；**Merkle 反熵**（Dynamo）是一切副本对账的姿势。
6. **单条目线性一致，不做全局事务**（S3 2020 强一致 + 每文件权威令牌）；跨目录原子改名等目录级语义用 range 分区内的局部事务换（ADLS Gen2 HNS / DanceNN）。

---

## 5. PartiSync 总体架构

### 5.1 设计原则

1. **Local-first，联邦化扩展**：核心跑在用户设备上，数据面默认端到端；hub 是可选加速器与聚合器，不是权威云。
2. **内容寻址优先**：BLAKE3 是贯穿始终的指纹（存储、传输、检索、去重共用）；路径只是视图。
3. **标准协议双面**：既能说所有存储方言（消费），也能把自己说成所有存储方言（服务）——「PartiSync 必须能被 rclone 当作 S3 remote 挂」是验收级互操作目标。
4. **语义诚实**：每个对外面（FUSE/S3/WebDAV）发布明确的语义声明（学 mountpoint-s3 的 SEMANTICS.md），不做含糊的「全 POSIX」承诺。
5. **无聊地基，新潮上层**：数据层 sqlx+SQLite/redb，网络层 iroh（QUIC）；AI、向量、网关放上层。把 Spacedrive V1 的死因（地基依赖废弃）列为头号工程风险。
6. **元数据平面按工业规律设计**：§4.3 的六条公理直接落进架构。

### 5.2 分层架构总图

```
┌──────────────────────────────────────────────────────────────────────────┐
│  前端壳:  CLI(partisync)   Tauri2 桌面/移动   Web 客户端   第三方工具(rclone等) │
└──────────┬───────────────────────────────────────────────────────────────┘
           │  RPC (localhost, quic-rpc/tonic; specta 生成 TS 绑定)
┌──────────▼───────────────────────────────────────────────────────────────┐
│                       partisd  ——  Rust 核心守护进程                       │
│                                                                          │
│  ┌─ ①统一资产层 PartiGraph ────────────────────────────────────────────┐ │
│  │ Entry(目录树+闭包表) · ContentIdentity · Tag DAG · Sidecar · Policy  │ │
│  └──────────────────────────────────────────────────────────────────────┘ │
│  ┌─ ②寻址: ps:// space/path · ps-content://blake3 · ps-device://… ──────┐ │
│  ┌─ ③元数据引擎 ───────────────────────────────────────────────────────┐ │
│  │ L0 设备本地: redb(KV) + SQLite(关系视图)   → 10⁷–10⁸ 条目            │ │
│  │ L1 Hub: 分片 fjall/rocksdb + openraft(Raft) → 10⁹–10¹² 条目          │ │
│  │ L2 联邦: 多 hub 按 space ID 联邦（无全局事务）                         │ │
│  └──────────────────────────────────────────────────────────────────────┘ │
│  ┌─ ④内容仓库 CAS ─────────────────────────────────────────────────────┐ │
│  │ fastcdc 分块(1MiB均) → BLAKE3 块哈希 → 块库 → pack 打包 → EC/分层      │ │
│  └──────────────────────────────────────────────────────────────────────┘ │
│  ┌─ ⑤同步与对账引擎 ───────────────────────────────────────────────────┐ │
│  │ watcher 事件日志 · 域分离 oplog(HLC) · Merkle 分级对账 · 冲突策略      │ │
│  └──────────────────────────────────────────────────────────────────────┘ │
│  ┌─ ⑥传输引擎 ─────────────────────────────────────────────────────────┐ │
│  │ 设备↔设备: iroh+iroh-blobs(BLAKE3验证流) · 云: OpenDAL+MPU+预签名      │ │
│  │ 泛化 delta(块清单交换) · 并行流水线 · 令牌桶限速 · 断点续传             │ │
│  └──────────────────────────────────────────────────────────────────────┘ │
│  ┌─ ⑦存储抽象 Provider SPI（OpenDAL 0.59, 50+ 后端 + 直连 aws-sdk-s3）───┐ │
│  ┌─ ⑧AI 层 ────────────────────────────────────────────────────────────┐ │
│  │ Sidecar 管线(缩略图/EXIF/OCR/whisper转写/BGE-M3+CLIP嵌入)            │ │
│  │ 混合检索(tantivy BM25 + usearch HNSW + reranker) · MCP server(rmcp)  │ │
│  │ 数据集出口(DVC式manifest) · C2PA 溯源 · Agent 预览-提交事务           │ │
│  └──────────────────────────────────────────────────────────────────────┘ │
│  ┌─ ⑨对外网关（把 PartiSync 暴露成标准协议）────────────────────────────┐ │
│  │ FUSE(fuser,非POSIX语义+写回日志) · S3 端点(axum自实现)               │ │
│  │ WebDAV(dav-server) · FTPS(libunftp) · SFTP(russh) · MCP(流式HTTP)    │ │
│  └──────────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────┘
           │ iroh(QUIC, 打洞+中继)           │ 可选
┌──────────▼──────────┐          ┌──────────▼──────────────────────────────┐
│  其他设备 partisd    │          │  partisync-hub: 分片元数据 + Raft + EC   │
│  手机/NAS/工作站/盒子 │◀────P2P──▶│  空间注册 · 全局检索 · 跨设备对账加速     │
└─────────────────────┘          └─────────────────────────────────────────┘
```

### 5.3 统一资产模型 PartiGraph

继承 Spacedrive V2 验证过的模型并做规模扩展（核心表 SQL 草案）：

```sql
-- 空间：同步与授权的基本单元（一台设备的库/一个云端桶/一个项目数据集）
CREATE TABLE space (
  id TEXT PRIMARY KEY,            -- ULID
  name TEXT NOT NULL,
  owner_device TEXT NOT NULL,     -- 权威属主设备
  enc_key_slot INTEGER,           -- 空间级密钥槽（E2EE）
  created_at INTEGER, options JSON
);

-- 设备与卷
CREATE TABLE device (id TEXT PRIMARY KEY, name TEXT, slug TEXT UNIQUE,
  pubkey BLOB, capabilities JSON, last_seen INTEGER);
CREATE TABLE volume (id TEXT PRIMARY KEY, device_id TEXT,
  fingerprint TEXT,           -- 稳定指纹：换挂载点/换机器不变（学 Spacedrive）
  is_removable INTEGER, is_cloud INTEGER);

-- 连接的外部存储（OpenDAL 配置）
CREATE TABLE provider (id TEXT PRIMARY KEY, scheme TEXT,  -- s3/webdav/ftp/...
  config_encrypted BLOB, health JSON);

-- 条目：目录树节点（文件/目录/符号链接/外部引用）
CREATE TABLE entry (
  id TEXT PRIMARY KEY,            -- ULID（平面 ID，可哈希分片）
  space_id TEXT, parent_id TEXT REFERENCES entry(id),
  kind INTEGER,                   -- file/dir/symlink/virtual
  name TEXT, content_id TEXT,     -- → content (可空：目录/未算哈希)
  size INTEGER, mtime_ns INTEGER, attrs JSON,
  state INTEGER,                  -- indexed/placeholder/materialized/tombstone
  device_path TEXT                -- 属主侧真实路径
);
CREATE TABLE entry_closure (     -- O(1) 子树查询（Spacedrive 验证过的模式）
  ancestor TEXT, descendant TEXT, depth INTEGER);
CREATE INDEX idx_entry_parent ON entry(space_id, parent_id);
CREATE INDEX idx_entry_name ON entry(space_id, name);

-- 内容身份：跨设备/跨位置去重的锚点
CREATE TABLE content (
  id TEXT PRIMARY KEY,            -- BLAKE3 全文件哈希（hex）
  size INTEGER, chunk_tree TEXT,  -- 块清单根（Merkle root）
  sampled_hash TEXT,              -- 两段式：先采样哈希粗判，再全量哈希确认
  mime TEXT, kind TEXT,           -- image/video/model/dataset/…
  c2pa JSON                       -- 溯源清单（可空）
);
CREATE TABLE chunk (              -- 内容块：去重与 delta 的原子
  hash TEXT,                      -- BLAKE3(chunk)
  size INTEGER,
  refs_json TEXT                  -- 压缩存储的 {content_id, offset} 反向引用
);
CREATE TABLE tag (…);             -- DAG：closure 表 + 同义词 + 关系强度（0-1）
CREATE TABLE sidecar (            -- 挂在 content 上：一份内容只算一次 AI
  content_id TEXT, kind TEXT,     -- thumbnail/ocr/transcript/embedding
  variant TEXT, format TEXT, store_ref TEXT, status INTEGER, version INTEGER);

-- 同步日志（域分离）
CREATE TABLE oplog (              -- 共享域变更：HLC 排序，对端 ACK 后裁剪
  hlc TEXT PRIMARY KEY, space_id TEXT, domain INTEGER, -- 0=设备自有 1=共享
  entity TEXT, op TEXT, payload BLOB);
CREATE TABLE scan_journal (       -- 本地 fs 事件日志（watcher 落盘）
  seq INTEGER PRIMARY KEY, path TEXT, event INTEGER, at INTEGER);
```

设计要点：
- **Entry 与 Content 分离**是去重的根：N 个 Entry → 1 个 Content → 1 份 Sidecar（缩略图/embedding 不重复算）。
- **`state=placeholder` 支撑「骨架同步」**：先把 10⁸ 条目录结构秒级铺开（stub），内容按需物化（ hydrate）——这是处理超大空间可用性的关键（OneDrive/Git LFS/Seafile 占位符模式）。
- `chunk` 表在设备侧是**热缓存**（最近访问的块），hub 侧是**全量分片库**。
- 全文与向量索引不进主库（SQLite FTS5 上限低），由 §5.10 索引引擎单独承载。

### 5.4 统一寻址

三种 URI 互相映射（对齐 SdPath，并刻意兼容云 CLI 习惯）：

```
ps://<space>/<path>            统一命名空间视图（用户与 AI 主要面对这个）
ps-device://<device-slug>/<path>   定位到某设备的物理文件
ps-content://<blake3-hex>      内容寻址（跨设备去重的自然语言："给我这个内容最快的副本"）
s3://bucket/key, dav://…       原生协议 URI 直接透传
```

URI 解析规则固定为 RFC 3986 兼容；`ps://` 是 PartiGraph 的投影——**同一个资产可以同时有 N 个 ps-device 来源与 1 个 ps-content 身份**，调度器按「速度/成本/可用性」选副本。

### 5.5 Provider SPI（存储抽象）

以 **Apache OpenDAL 0.59（50+ 后端）**为底座，其上补两层：

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    // OpenDAL Operator 转发大部分操作
    fn od(&self) -> &opendal::Operator;

    // OpenDAL 没有或需特化的能力（能力协商式设计）
    fn caps(&self) -> ProviderCaps;          // 哈希类型/校验和、MPU、预签名、事件通知、
                                             // 服务端 copy、mtime 精度、原子 rename…
    async fn events(&self, cursor: &Cursor) -> Option<EventBatch>; // S3 SQS/webhook 等
    async fn delta_chunks(&self, key: &str, want: &[ChunkHash])
        -> Option<ChunkPlan>;                // 仅内部协议支持；云端恒为 None
    async fn presign_put(&self, key: &str, ttl: Duration) -> Option<Url>;
}
```

能力协商（caps）吸取 rclone 的教训：**哈希/mtime/原子性异构性用显式能力位表达，而不是散落在每个命令的 flag 里**。同步引擎根据 caps 自动降级策略（有校验和→校验和比对；无→size+mtime+采样哈希）。

### 5.6 元数据引擎：三层结构

| 层 | 引擎 | 容量 | 说明 |
|---|---|---|---|
| **L0 设备** | redb（KV：scan_journal、oplog、chunk 热表）+ SQLite/sqlx（关系：PartiGraph 视图） | 10⁵–10⁸ 条目 | 无守护依赖、可嵌入；10⁷ 文件 ≈ 3 GB 级索引 |
| **L1 Hub** | 分片 **fjall**（纯 Rust LSM，活跃）或 rust-rocksdb（久经考验但 C++ FFI）+ **openraft**（Raft 组，每分片一组） | 10⁹–10¹² 条目 | **Name/File/Block 三层哈希分片（学 Tectonic）+ 目录树 range 分区（学 ADLS HNS/DanceNN，支撑 O(1) 目录改名）**；每条目 ≤300B |
| **L2 联邦** | 多 hub 按 space ID 联邦路由；空间注册表（权威）+ 联邦查询代理 | 聚合 10¹²+ | 无跨 hub 事务（公理 6）；跨 hub 检索用扇出-聚合 |

关键实现决策：
- **分片策略**：`entry` 按平面 ID 哈希分片（负载均匀、无热目录）；`entry_closure`/目录路径按 **range 分区 + 动态分裂**（支持目录级扫描与原子 rename）。这两者组合是 Tectonic（哈希）+ Azure HNS/DanceNN（range 树）的合成，是已知最优解。
- **对账**：每空间维护按 ID 区间分级的 **Merkle 树**（Dynamo 反熵），设备↔hub 定期交换根哈希，只对分歧区间做增量拉取；配合 oplog 水位线（学 BEP delta index）做快路径。
- **GC/裁剪**：学 restic prune 的痛——**不做全局独占锁 prune**。采用「分代 + 每空间根」的增量标记-清扫：pack 带创建代，引用计数视图按空间快照计算，删除延迟一个宽限期；hub 上 GC 按分片并行，可暂停可限速。

### 5.7 同步与对账引擎

**域分离同步**（Spacedrive V2 的核心洞察，直接采用）：
- **设备自有域**（entry 的位置/属主状态）：单写者，属主状态权威——不引入 CRDT/共识。设备离线时的修改在重连后以 oplog 重放。
- **共享域**（tag、用户元数据、content 身份、评论）：HLC 排序 oplog + LWW（可按字段定义 merge）；创建冲突天然 union 合并。
- **内容域**：CAS 不可变，收敛是平凡的；只对账「谁有什么」（块索引），不对账内容本身。

**事件管线**：
```
notify(watcher) → scan_journal(落盘, seq) → 归并器(去抖/合并) → 
  ├─ 即时通道: 内存 oplog → 实时推送对端 (iroh)
  └─ 批量通道: 摘要块 → Merkle 更新 → 对账
云端: provider.events(SQS/webhook) 或游标增量 LIST → 同一 journal 接口
```

**冲突策略**：目录级策略（保留两者/新者胜/手选/CT 借鉴的自定义 merge 函数）；默认「保留两者 + 记录血缘」——**宁可多一份副本，不可静默丢数据**（rclone bisync 的 `file.conflict1` 是正确方向，PartiSync 把它做成一等公民并挂进资产图谱）。

**对账协议**（设备↔设备、设备↔hub 同构）：
1. 交换空间列表 + 每 space 的 oplog 水位（快路径：只拉增量）；
2. 水位对不上或首次相遇 → Merkle 根比对 → 递归下钻分歧区间（慢路径）；
3. 内容差异解析到块级 → 进入传输引擎的 ChunkPlan。

### 5.8 传输引擎

**三条通道，同一套分块/调度/限速底座**：

| 通道 | 技术 | 适用 |
|---|---|---|
| 设备↔设备 | **iroh 1.x + iroh-blobs**（QUIC 打洞 + 中继回退；BLAKE3 验证流式） | 手机/NAS/工作站直连；连接迁移支持移动网络切换 |
| 设备↔云 | OpenDAL + **S3 MPU**（>100MB 自动启用）+ 预签名直传（浏览器/Agent 不经核心） | 全部 50+ 云后端 |
| 设备↔hub / hub 内部 | 自有 QUIC 协议（quinn）+ **泛化 delta**：接收方先送「我有的块哈希清单」（BEP 块交换 + casync 种子思想的合体），发送方只流缺失块 | hub 集群复制、大资产分发 |

**泛化 delta**（把 rsync 思想搬到对象存储时代的关键）：对支持「按块哈希拉取」的对端（PartiSync 对端、hub、自有 CAS 桶），delta 是原生的；对不支持的对象存储，退化为两档：① 小文件（<256MB）整文件传；② 大文件在**目标侧 PartiSync 代理**（若安装了 partisd 的 NAS/边缘节点）维持块索引，实现近 delta——并把该事实通过 caps 如实上报。

**调度与可靠性**：
- 分块流水线并发（BDP 补偿）：默认在途窗口自适应 RTT×bw；MPU 并行块；
- 令牌桶限速（学 rclone --bwlimit，支持 HH:MM 日程表）+ 全局/通道级两级；
- 断点续传：MPU 天然可续；自有协议按块记账；云端下载按 range 续传；
- 完整性：每块 BLAKE3 + 文件级 BLAKE3 + S3 边界映射 CRC64NVMe/SHA-256；**永不使用 MPU ETag 做校验**；
- io_uring（Linux）+ sendfile 零拷贝 + BLAKE3 SIMD 多线程（哈希不挡路）。

### 5.9 内容仓库（CAS）与存储分层

- **分块**：fastcdc（min 256KB / avg 1MiB / max 4MiB）——与 restic/borg/kopia 的经验值一致；AI 权重等超附录文件用同一分块器天然支持 checkpoint 级 delta。
- **块库**：内容寻址 `hash → {device 热缓存 | hub 分片 | pack 文件 | 远端对象}`。
- **pack 打包**（公理 4）：~128–256MB pack（restic/kopia 经验），小文件打包 + 目录 blob 化；hub 上对 pack 做 **RS(10,4) 或 LRC(12,2,2) 纠删**（f4/Azure 几何），修复带宽限流（学 f4 的 10% reconstruction-read 上限）。
- **分层**：设备 NVMe（热）→ hub HDD（温）→ S3/Glacier 冷（生命周期规则自动沉降水位）。
- **去重收益的量化口径**：`unique_bytes / logical_bytes` 每 volume/每空间持续输出（Spacedrive 的 `unique_bytes` 概念），让用户看得见省了多少。

### 5.10 索引与检索引擎

- **全文**：tantivy 0.26（BM25，文件名/标签/OCR 文本/转写文本多字段）。
- **向量**：usearch（HNSW）本地库；hub 级向量随空间分片（内容分片哈希路由，检索扇出合并）。
- **混合检索**：BM25 + 向量 RRF 融合 + bge-reranker 精排（fastembed 全家桶：BGE-M3 稠密+稀疏+ColBERT 一趟前向、CLIP 图像嵌入）。
- **Sidecar 管线**（挂 content 不挂 entry，去重后只算一次）：缩略图/EXIF → OCR（ort）→ 音视频转写（whisper-rs）→ 嵌入（fastembed）→ C2PA 校验。全部走持久作业系统（checkpoint + MessagePack 状态，学 Spacedrive jobs）。
- **检索面**：本地毫秒级；hub 上对 10⁹ 条目目标 LIST p99 <100ms、混合检索 p95 <300ms（见 §7 KPI）。

### 5.11 安全模型

- **身份**：设备 = Ed25519 密钥对（node id 即公钥，与 iroh 一致）；配对用 BIP39 助记词 + 挑战签名（学 Spacedrive）。
- **加密**：空间级 E2EE（XChaCha20-Poly1305；密钥层次：根密钥 → 空间密钥 → 文件密钥/块密钥派生，学 restic/kopia 的 AEAD 经验）；密钥存 OS keychain + 恢复短语。
- **共享**：空间角色（owner/editor/viewer）+ 内容级分享链接（预签名 + 可选密码 + 过期）；**不可信设备可收预加密数据**（学 Syncthing）。
- **AI 与加密的张力**（诚实处理）：E2EE 空间的语义索引只允许**在持有密钥的设备端**计算（嵌入/全文均端上算，hub 只见密文索引的密文形式或仅存端上）——默认非加密空间才启用 hub 级检索，加密空间检索性能下降是明码标价的取舍。

### 5.12 AI 原生层

1. **MCP server**（rmcp 3.4.0，**spec 2026-07-28 起 stateless 化：去掉 initialize 握手与会话，每请求自带能力；Roots/Sampling 已废弃，一切皆 tools**——对文件基建是利好，HTTP 面可水平扩展）。工具集设计：
   - `asset_search`（混合检索：元数据过滤 + 全文 + 向量，返回分页 + `ttlMs` 缓存提示）
   - `asset_read`（分页/范围读，支持超大文件）
   - `asset_organize`（移动/改名/打标——**必须走 Preview→Commit→Verify 事务**，学 Spacedrive ActionManager；MRTR 多轮往返正好承载「确认覆盖」对话）
   - `dataset_export`（生成 DVC 式 manifest + 校验和 + S3 直传链接——训练管线拿数据的标准姿势）
   - `job_status` / 长任务走 MCP tasks 扩展
2. **语义整理**：「把过去三个月的发票按公司归类」「找出所有重复的 4K 视频并保留最高画质副本」——全部先出预览（diff 视图）再提交。
3. **数据集/模型资产一等公民**：`kind=dataset/model` 的内容启用 checkpoint 感知（CDC 增量保存）、数据集快照（lakeFS 证明「对象存储上的分支/提交」可行）、谱系图（derive 关系：数据集 → 训练任务 → 模型 → 评测报告，作为 entry 关系边存入 PartiGraph）。
4. **溯源**：摄取时用 c2pa-rs 校验并保留 C2PA manifest；PartiSync 自身对整理操作写审计日志。
5. **隐私**：本地嵌入默认（fastembed/ort，Apple Silicon 用 Metal 后端；candle 备选），可选 Ollama/云 API；一切 AI 计算走 Sidecar 作业管线，可审计可暂停。

### 5.13 对外网关（PartiSync 被消费的面）

| 网关 | 实现 | 语义声明（明确非目标） |
|---|---|---|
| **FUSE 挂载** | fuser | 学 mountpoint-s3：随机读 + 新文件顺序写完整支持；覆盖/改名/删除走**本地写回日志 + overlay**（这是 mountpoint-s3 明确不做、而 PartiSync 有本地索引可以做的差异化）；诚实文档化 |
| **S3 端点** | axum 自实现子集 | MPU、预签名、ListV2 游标、CRC64NVMe/SHA-256 校验和、版本化——**验收标准：rclone 把 partisync 挂为 S3 remote 完成 bisync** |
| **WebDAV** | dav-server | 尽力 ETag/LOCK；文档声明已知缺口 |
| **FTPS/SFTP** | libunftp / russh | 兼容清单项 |
| **MCP** | rmcp + Streamable HTTP | Agent 的一等入口（§5.12） |

---

## 6. 技术选型清单（2026-09 核实版本）

| 领域 | 选型 | 版本 | 理由/备选 |
|---|---|---|---|
| 异步运行时 | tokio | 1.53 | 事实标准 |
| TLS/QUIC | rustls / quinn | 0.23 / 0.11 | iroh 底座同源 |
| P2P 传输 | **iroh + iroh-blobs** | 1.2 / 0.103 | 1.0 已 GA（2026-06）；BLAKE3 验证流与内部寻址同构 |
| 存储抽象 | **OpenDAL** | 0.59.2 | Apache TLP、50+ 后端；S3 特化路径直用 aws-sdk-s3 |
| 本地 KV | redb（+ fjall 备选写重型） | 4.3 / 3.1 | 纯 Rust ACID；**禁用 sled**（2024-10 起停更） |
| 关系/查询 | sqlx + SQLite | 0.9 | 无聊且稳定（Spacedrive 教训）；hub 端 fjall/rocksdb 分片 |
| 共识 | openraft | — | hub 分片 Raft（TiKV/ZippyDB 同构思路） |
| 分块 | fastcdc | 5.0 | FastCDC 标准移植 |
| 哈希 | blake3 | 1.8 | SIMD 多线程 + 验证流式（bao） |
| 压缩 | zstd（+ seekable） | 0.14 | 随机访问压缩包 |
| 全文 | tantivy | 0.26 | Lucene 级 |
| 向量 | usearch（备选 hnsw_rs） | 2.26 | HNSW、紧凑 |
| 嵌入/推理 | fastembed（ort/candle 底） | 7.0 / 2.0rc / 0.11 | BGE-M3、CLIP 一站式 |
| 语音 | whisper-rs | — | 转写 Sidecar |
| FUSE | fuser | 0.18 | mountpoint-s3 同路线 |
| WebDAV 服务 | dav-server | 0.11 | 唯一成熟 Rust 库 |
| FTPS 服务 | libunftp | 0.23 | bol.com 生产背书 |
| MCP | **rmcp** | 3.4 | **官方** SDK，跟 spec 2026-07-28 |
| RPC/网关 | tonic/prost + axum | 0.14 / 0.8 | 控制面 + S3 端点 |
| 溯源 | c2pa-rs | 2.4 spec | Adobe 官方 Rust SDK |
| 前端壳 | Tauri 2（React） | 2.11 | 桌面+移动（Spacedrive 同路线）； specta 生成 TS 绑定 |
| 文件事件 | notify | 8.2 | 跨平台；网络盘回落轮询 |

---

## 7. 性能与规模 KPI（验收口径）

| 指标 | M2（单机） | M3（hub） | M5（万亿路径） |
|---|---|---|---|
| 索引吞吐 | 笔记本 10⁶ 文件 < 10 min | hub 10⁹ 条目全量导入 < 24 h（10 节点） | 联邦聚合 10¹¹+ 元数据条目在线 |
| LIST p99 | 10⁷ 条目 < 50 ms | 10⁹ 条目 < 100 ms | 分片后近似线性 |
| 混合检索 p95 | 10⁶ 条目 < 100 ms | 10⁹ 条目 < 300 ms | 扇出聚合 |
| delta 传输效率 | checkpoint 5% 变更 → ≤8% 流量 | 同左 | — |
| 去重率报告 | unique/logical 比例实时可见 | 同左 | — |
| 元数据成本 | — | ≤300 B/条目（盘上 ×3 写放大内） | ≤400 B 含块索引摊销 |
| 互操作 | rclone↔partisync(S3 remote) bisync 通过 | — | — |

---

## 8. 分阶段路线图

> 原则（吸取 Spacedrive/Unison/AList 教训）：**每阶段都交付「每天可用的窄核心」**；不先造大教堂。每个里程碑有明确的对外可用物与回退路径。

### M0 — 骨架与本地引擎（4–6 周）
- cargo workspace：`partisync-core / provider / metadata / cas / transfer / sync / index / gateway / cli`；
- 本地 provider + redb/SQLite 索引 + fastcdc/BLAKE3 CAS + CLI：`partisync init/index/ls/cp/rm/find/dedupe`；
- 持久作业系统 + scan_journal。
- **验收**：100 万文件索引 <10 min（M 系列芯片笔记本）；同卷去重报告可见；`find` 毫秒级。

### M1 — 协议广度（云 + 网关第一面）（6–8 周）
- OpenDAL 接入 S3/WebDAV/FTP/SFTP + caps 能力协商；S3 MPU + 断点续传 + bwlimit；
- `partisync serve webdav/s3` 第一版（**rclone 互操作验收**）；FUSE 只读挂载。
- **验收**：rclone 把 partisync 当 S3 remote 完成 copy/ls；1 GB 弱网传输断点续传无损。

### M2 — 同步引擎与多端（8–10 周）
- watcher 事件管线 + 单向 sync + bisync（oplog/HLC）+ 冲突策略；占位符（placeholder）骨架同步；
- iroh 设备配对（助记词）+ 设备直连传输（BLAKE3 验证流）+ 块级 delta；
- 空间级 E2EE。
- **验收**：手机↔NAS↔笔记本三端空间同步；断网修改重连收敛；加密空间在无密钥设备上不可见。

### M3 — Hub 与对账（8–10 周）
- `partisync-hub`：分片元数据（fjall + openraft）+ Merkle 分级对账 + 角色/授权；
- pack + EC(RS 10,4) + 分层（NVMe→HDD→S3 冷）+ 增量 GC；
- 空间注册表 + 全局检索（ tantivy/usearch 落地）。
- **验收**：单 hub 集群 10⁹ 条目；两台设备经 hub 对账 10⁷ 条目差异 <5 min。

### M4 — AI 层（6–8 周）
- Sidecar 全管线（缩略图/EXIF/OCR/转写/嵌入）+ 混合检索 + reranker；
- MCP server（asset_search/read/organize/dataset_export）+ 预览-提交事务；
- C2PA 摄取校验；数据集/模型 kind 特化（谱系边、checkpoint 增量）。
- **验收**：10⁶ 条目语义问答 p95<200ms；Agent 通过 MCP 完成「归类 + 导出 manifest」闭环。

### M5 — 万亿路径（探索期，持续）
- 联邦多 hub；块索引分片与冷分层；云事件流驱动增量摄取（SQS/webhook/游标）；
- 分布式扫描调度器（分片 LIST 并行 + 限速 + 断点）；合成元数据负载压测 10¹¹–10¹²。
- **验收**：仿真 10¹² 条目元数据平面；前缀分片扫描 10¹² 对象桶的策略基准报告。

### M6 — 产品化
- Tauri 桌面/移动壳打磨；WASM 扩展（学 Spacedrive：扩展定义自有数据模型并随空间同步）；SMB/NFS 评估；企业特性（审计/配额/多租户）。

---

## 9. 风险矩阵与对策

| # | 风险 | 证据 | 对策 |
|---|---|---|---|
| 1 | **范围过大，重蹈 Spacedrive「两年 pre-alpha」** | Spacedrive 资金断裂、V1 判不可维护 | 窄核心先行（M0-M1 全部独立可用）；每个里程碑发布可用物；不做全 POSIX 承诺 |
| 2 | **地基依赖废弃**（prisma-client-rust 之死） | Spacedrive V1 死因 | 数据层只用 sqlx/redb/SQLite 级「无聊」件；锁定版本 + 抽象层隔离 OpenDAL/iroh |
| 3 | **0.x 生态 API 漂移**（opendal/iroh/rmcp 一年三次 breaking） | rmcp 1→2→3；opendal 0.x | 内部 trait 隔离 + CI 锁定 + 升级演练 |
| 4 | **GC/prune 复杂度**（restic prune 之痛） | restic 独占锁 + S3 重写 | 分代 GC、每空间根、可暂停可限速；M3 起持续混沌测试 |
| 5 | **E2EE 与检索/AI 的矛盾** | 结构性张力 | 默认本地 AI；加密空间端侧检索；性能取舍明码标价 |
| 6 | **万亿目标的真实性质疑** | 单用户 10¹² 不成立 | 明确定位「联邦聚合」；文档公开推算（§4.1）；KPI 以仿真 + 真实混合口径 |
| 7 | **治理/信任**（AList 供应链事件） | AList→OpenList 分叉 | 从第一天起：独立基金会式治理文档、可复现构建、签名发布 |
| 8 | **云 provider 长尾异构**（rclone 70 后端的维护沼泽） | rclone 各后端哈希/mtime/原子性乱象 | 站在 OpenDAL 肩膀不重造；caps 显式协商；不承诺 100% 兼容矩阵，发布兼容度报告 |
| 9 | **安全实现错误**（加密/密钥） | restic/kopia 被审计过，自研易错 | 优先复用 audited 原语（age/XChaCha20/BLAKE3 KDF）；密钥层次文档化 + 第三方审计纳入 M4 预算 |
| 10 | **P2P 打洞率与中继成本** | iroh 声称 90%+ 直连 | 中继可自托管；hub 作为 ALWAYS 可达回退 |

---

## 10. 参考文献（按主题）

**算法与工具**：rsync [tech report](https://www.samba.org/rsync/tech_report/node3.html) · [rsync 3.5.0 33 CVE](https://www.samba.org/rsync/) · [rclone overview](https://rclone.org/overview/) · [rclone bisync](https://rclone.org/bisync/) · [rclone chunker](https://rclone.org/chunker/) · [BEP v1](https://docs.syncthing.net/specs/bep-v1.html) · [restic design](https://restic.readthedocs.io/en/v0.5.0/Design/) · [kopia architecture](https://kopia.io/docs/advanced/architecture/) · [casync](https://0pointer.net/blog/casync-a-tool-for-distributing-file-system-images.html)

**Spacedrive**：[V2 白皮书](https://v2.spacedrive.com/overview/whitepaper) · [data model](https://v2.spacedrive.com/core/data-model.md) · [library sync](https://v2.spacedrive.com/core/library-sync.md) · [networking](https://v2.spacedrive.com/core/networking.md) · [history（V1 死因）](https://v2.spacedrive.com/overview/history.md) · [GitHub](https://github.com/spacedriveapp/spacedrive)

**万亿级系统**：[GFS SOSP'03](https://research.google.com/archive/gfs-sosp2003.pdf) · [Colossus](https://cloud.google.com/blog/products/storage-data-transfer/a-peek-behind-colossus-googles-file-system) · [Haystack OSDI'10](https://www.usenix.org/legacy/event/osdi10/tech/full_papers/Beaver.pdf) · [f4 OSDI'14](http://www.cs.princeton.edu/~wlloyd/papers/f4-osdi14.pdf) · [Tectonic FAST'21](https://www.usenix.org/system/files/fast21-pan.pdf) · [WAS SOSP'11](https://sigops.org/s/conferences/sosp/2011/current/2011-Cascais/printable/11-calder.pdf) · [Azure LRC ATC'12](https://www.usenix.org/conference/atc12/technical-sessions/presentation/huang) · [Dynamo SOSP'07](https://dl.acm.org/doi/pdf/10.1145/1294261.1294281) · [S3 400T 对象](https://press.aboutamazon.com/2024/12/amazon-s3-expands-capabilities-with-managed-apache-iceberg-tables-for-faster-data-lake-analytics-and-automatic-metadata-generation-to-simplify-data-discovery-and-understanding) · [S3 强一致](https://aws.amazon.com/blogs/aws/amazon-s3-update-strong-read-after-write-consistency/) · [S3 性能指南](https://docs.aws.amazon.com/AmazonS3/latest/userguide/optimizing-performance.html) · [HDFS Federation](https://hadoop.apache.org/docs/stable/hadoop-project-dist/hadoop-hdfs/Federation.html) · [HopsFS FAST'17](https://www.usenix.org/conference/fast17/technical-sessions/presentation/shvachko) · [JuiceFS 架构](https://juicefs.com/docs/community/architecture/) · [JuiceFS 元数据选型](https://juicefs.com/en/blog/usage-tips/juicefs-metadata-engine-selection-guide) · [Pangu FAST'23](https://www.usenix.org/conference/fast23/presentation/li-qiang-deployed) · [DanceNN 解读](https://juejin.cn/post/7088582913347813412) · [DeepSeek 3FS](https://github.com/deepseek-ai/3fs)

**协议**：[S3 MPU](https://docs.aws.amazon.com/AmazonS3/latest/userguide/mpu-upload-object.html) · [S3 校验和](https://docs.aws.amazon.com/AmazonS3/latest/userguide/checking-object-integrity.html) · [mountpoint-s3 语义](https://github.com/awslabs/mountpoint-s3/blob/main/doc/SEMANTICS.md) · [WebDAV RFC 4918](https://datatracker.ietf.org/doc/html/rfc4918) · [FTPS RFC 4217](https://datatracker.ietf.org/doc/html/rfc4217) · [QUIC RFC 9000](https://datatracker.ietf.org/doc/html/rfc9000) · [BitTorrent v2](https://www.bittorrent.org/beps/bep_0052.html) · [iroh 1.0](https://www.iroh.computer/blog/v1) · [iroh-blobs](https://github.com/n0-computer/iroh-blobs)

**AI 面**：[MCP spec 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28/changelog) · [rmcp 官方 Rust SDK](https://github.com/modelcontextprotocol/rust-sdk) · [MCP 参考文件系统服务器](https://github.com/modelcontextprotocol/servers) · [fastembed-rs](https://github.com/Anush008/fastembed-rs) · [C2PA 2.4](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html) · [c2pa-rs](https://github.com/contentauth/c2pa-rs) · [DVC](https://dvc.org) · [lakeFS](https://lakefs.io) · [Ask Photos](https://blog.google/products-and-platforms/products/photos/updates-ask-photos-search/)

**同类/聚合**：[AList](https://github.com/alistgo/alist) · [OpenList](https://github.com/OpenListTeam/OpenList) · [oCIS](https://central.owncloud.org/t/released-owncloud-infinite-scale-7-2-0/64283) · [OpenDAL](https://opendal.apache.org/) · [CloudDrive2](https://www.clouddrive2.com/en/) · [Seafile](https://www.seafile.com/)

---

## 附录 A：建议的仓库结构

```
partisync/
├── crates/
│   ├── partisync-core/        # 类型、ULID、HLC、caps、错误体系
│   ├── partisync-graph/       # PartiGraph 模型 + SQLite/redb 仓储 + oplog
│   ├── partisync-cas/         # fastcdc 分块、BLAKE3、块库、pack、GC
│   ├── partisync-provider/    # Provider SPI + OpenDAL 适配 + caps 协商
│   ├── partisync-transfer/    # 三通道传输、MPU、delta、限速、流水线
│   ├── partisync-sync/        # watcher、journal、bisync、Merkle 对账
│   ├── partisync-index/       # tantivy + usearch + sidecar 管线
│   ├── partisync-ai/          # fastembed、whisper、MCP server、数据集出口
│   ├── partisync-gateway/     # FUSE、S3 端点、WebDAV、FTPS、SFTP
│   ├── partisync-hub/         # 分片元数据、openraft、联邦路由
│   ├── partisd/               # 守护进程组装
│   └── partisync-cli/         # CLI
├── apps/tauri/                # 桌面/移动壳
├── docs/                      # 本文档 + ADR（架构决策记录）
└── xtask/                     # 发布/基准脚本
```

## 附录 B：与竞品的关键差异化清单（对外沟通口径）

1. **对云端的块级 delta**（rclone 做不到，rsync 只对 rsync）；
2. **同步主路径上的内容去重**（备份工具才有的能力，放进资产管理）；
3. **全局资产图谱 + 跨设备内容身份**（Spacedrive 思想，扩展到 hub 规模）；
4. **既是协议消费者也是协议提供者**（rclone serve 能力 + 自有索引加持）；
5. **AI 一等公民**：本地语义索引 + MCP 工具面 + 数据集/模型特化 + C2PA；
6. **元数据平面遵循工业界万亿级公理**，从 L0 到 L2 平滑扩展，而非推倒重来。
