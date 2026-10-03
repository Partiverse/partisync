# ADR-0026: FUSE 挂载面 —— fuser 线位与进产品依赖图决策（草案）

版本: 0.2 · 状态: **已接受（2026-09-30 用户拍板「批准 ADR-002」即 ADR-0026；
随本 PR 合入生效。0.1→0.2 变更 = 状态行 + §决策前置条件处置表 + 修订登记）**
关联: SPEC M7-WP03（评估 WP，批准 2026-09-30，PR #47）、ADR-0003（toolchain
1.94 pin）、ADR-0015（对外协议网关层）、调研方案 §3.3（FUSE mountpoint-s3
语义）/ §3.4（SMB 桥接路线）/ §9.3（语义诚实）、执行方案 §6.7 主题三
负责人: @lead · 起草日期: 2026-09-30（M7-WP03-T04）· 接受日期: 2026-09-30

## 修订登记

| 修订 | 日期 | 内容 | 依据 |
|---|---|---|---|
| 0.1 | 2026-09-30 | 初稿（T04 草案） | PR #51 |
| 0.2 | 2026-09-30 | 草案→已接受；前置条件处置表（§决策节末） | 用户拍板 2026-09-30 |
| 0.3 | 2026-10-03 | **前置条件 3 承接启动**：写回日志落地（unlink/rmdir/rename 经 `.partisync-writeback/` JSONL 日志路径，先日志后应用 + 重放幂等 = P16 转正；SEMANTICS.md 同步修订）——overlay 整文件替换 / 索引接线 / by-hash 归 T03–T05 | SPEC M8-WP07（批准 PR #59）；PR 本卡 |

## 背景

执行方案 §6.7 主题三「SMB 桥接评估」自 M6 顺延 M7（M6-report §1 记录为
顺延非裁剪），2026-09-29 用户拍板立项为 M7-WP03。调研方案 §3.4 已定路线：
**Rust 原生 SMB 服务端栈不成熟，v1 不实现线协议**——用 FUSE（fuser）前置
本地 POSIX 语义，需要 SMB 时桥接 Samba。

SPEC M7-WP03 按 M6-WP04 判例设计为**评估 WP**：产品依赖图零改动，实测
技术边界，产出进产品图的决策建议。三任务实测结论：

- **T01**（[FUSE spike 实测](../reviews/M7-WP03-fuse-spike.md)）：fuser
  **0.18.0** 线位成立，API 逐项查证；mountpoint-s3 式「诚实非 POSIX」
  语义完整可实现，拒绝面探针真挂载 2/2 绿；Linux pure-rust 实现**免 C
  库**（build.rs 实证），pure 挂载直走 `mount(2)`；macOS 需 macFUSE/
  FUSE-T 或 `macos-no-mount` feature。
- **T02**（[Samba 桥接评估](../reviews/M7-WP03-samba-bridge-eval.md)）：
  smbd 4.23.8 **实测可 export FUSE 挂载点**（SMB2 协议往返通）；oplock/
  lease 无对象可锁（本 FS 拒一切写）；**鉴权约束已捕获未闭环**
  （`NT_STATUS_ACCESS_DENIED`，假设 = `DefaultPermissions` + 属主 uid
  vs guest 落 nobody）。
- **T03**（[企业特性议题登记](../reviews/M7-WP03-enterprise-topics.md)）：
  扩展来源侧供应链面空白，签名/registry 列为 M8+ 候选。

## 决策（待拍板）

**推荐路线：采纳 fuser `>=0.18.0, <0.19` 入产品依赖图，宿主落在
`partisync-gateway`（对外协议网关层，ADR-0015），语义遵循
mountpoint-s3 式「诚实非 POSIX」+ SEMANTICS.md 声明面。**

**本 ADR 状态为草案**——下列各项须用户拍板后本 ADR 方可转「已接受」，
且进产品图前须完成 §决策前置条件。

### 线位与落位

| 项 | 内容 |
|---|---|
| 线位 | `fuser >=0.18.0, <0.19`（0.18 为当前 max_stable，T01 查证） |
| 落位 | `crates/partisync-gateway`（对外协议网关层，ADR-0015 现有职责「FUSE/S3/WebDAV/FTPS/SFTP/MCP」） |
| feature | 无需 libfuse 系（Linux pure-rust）；macOS 需 macFUSE/FUSE-T 运行时依赖（**用户侧环境前置**，非 crate 面） |
| toolchain | 实施 WP 须在 1.94 pin 下复测（T01/T02 容器实测用 1.96.1，向上兼容证据；R6 登记） |
| 语义面 | 随机读 + 新文件顺序写；拒绝面（unlink/rmdir/rename/mkdir/mknod/symlink/setattr/已存在文件写打开）显式 EPERM/EACCES，拒绝先于破坏性效果（P15，实施期转正） |

### 决策前置条件（未完成即不得采纳）

1. **鉴权约束闭环**（T02 §3）：属主身份映射方案定案（guest 映射属主 /
   FUSE 侧 attr 统一映射 / 关闭 DefaultPermissions 上移授权决策）——后者
   扩大 FUSE 宿主的权限责任面，需权衡记录；
2. **冷缓存读基准 + macOS FUSE-T 本机复测**（T01 §5 未测项登记）；
3. **写回日志 + overlay 二期范围**（覆盖/改名/删除）单独立项，**不在一期**；
4. 用户对「SMB 桥接随 FUSE 落地」的取舍拍板（桥接非必需——局域网场景亦可
   直接挂 FUSE）。

### 前置条件处置（2026-09-30 接受时定案）

| # | 处置 | 说明 |
|---|---|---|
| 1 | **随实施 WP 一期闭环**（不再阻断接受） | FUSE 宿主**关闭 `DefaultPermissions`**、由 FUSE 层自管权限判定（spike 现状即无内核权限检查语义），授权决策上移 PartiSync——M2 设备身份模型对齐；**宿主权限责任面扩大**记为实施 WP 的 R2 评审项（方案 c 已按本 ADR §后果预登记权衡） |
| 2 | **转为实施 WP 验收项**（不阻断接受） | 冷缓存基准 + macOS FUSE-T 复测列为一期验收硬项（T01 已有页缓存口径实测） |
| 3 | **维持**：写面二期单独立项 | 一期只读 + 新文件顺序写（SEMANTICS.md） |
| 4 | **维持可选**：SMB 桥接不随一期强制 | 局域网可直挂 FUSE；桥接属主映射待企业需求确认 |

## 后果

**正面**：
- 对外协议面新增本地 POSIX 挂载（无守护进程、单用户、零配置）；
- CAS 内容寻址与「对象不可变」天然同构（SEMANTICS.md 语义）；
- Samba 桥接路径已实测可行（企业 LAN 场景可选）。

**负面 / 成本**：
- 第三方 crate 入根须 cargo deny 通过（实施 WP 门禁）；
- macOS 部署需 FUSE-T/macFUSE 环境前置（安装需 sudo——本机实测受限）；
- SMB 桥接引入属主身份映射约束（T02 实测捕获）；
- 只读语义一期，Windows/macOS 客户端的「保存/改名」期望不满足（显式
  拒绝 + 文档诚实标注，SEMANTICS.md）。

**风险**：
- R-s1 鉴权约束未闭环即进产品 → 用户可见的访问拒绝（登记为阻断前置条件）；
- R-s2 macOS 环境前置不可得 → macOS 用户无法挂载（降级：文档标注 + 后续
  FUSE-T 打包分发议题）；
- R-s3 只读语义与用户直觉冲突 → 依赖 SEMANTICS.md 诚实标注与 UI 引导。

## 实施 WP 切分建议（T04 产出，WP03 收官）

| WP | 内容 | 备注 |
|---|---|---|
| WP-1 | 一期：只读 + 新文件顺序写，fuser 入根，SEMANTICS.md，探针转正（P15） | 决策前置条件闭环后开工 |
| WP-2 | 二期：写回日志 + overlay（覆盖/改名/删除） | 差异化项（调研方案 §3.3），独立 SPEC |
| WP-3 | 可选：Samba 桥接打包（smbd 配置模板 + 属主映射） | 随用户需求；T02 约束为设计输入 |

## 备选方案与否决理由

| 方案 | 否决理由 |
|---|---|
| Rust 原生 SMB 服务端 | 调研方案 §3.4：生态不成熟（2026-09 复核维持） |
| 跳过 FUSE 直接 SMB | 同上；且放弃本地 POSIX 面（用户单机场景的核心诉求） |
| 先做写语义再评估 | 与调研方案 §3.3 的差异化定位冲突——只读面先立（诚实边界），写面二期 |

## 关联工件

- SPEC [M7-WP03](../specs/M7-WP03.md)（批准 2026-09-30）
- T01 [docs/reviews/M7-WP03-fuse-spike.md](../reviews/M7-WP03-fuse-spike.md)
- T02 [docs/reviews/M7-WP03-samba-bridge-eval.md](../reviews/M7-WP03-samba-bridge-eval.md)
- T03 [docs/reviews/M7-WP03-enterprise-topics.md](../reviews/M7-WP03-enterprise-topics.md)
- spike 载体 `crates/partisync-fuse-spike`（独立 workspace，SEMANTICS.md）
