# PartiFuse 语义声明（SEMANTICS.md，产品版）

> 学 mountpoint-s3 的 SEMANTICS.md 判例（调研方案 §9.3 语义诚实原则）：
> 本挂载面**明确非 POSIX**。一期（M8-WP01）支持的操作与拒绝的操作逐条
> 列出，不做含糊的「全 POSIX」承诺。任何语义变更必须先改本文件（语义
> 先行）。spike 版溯源：`crates/partisync-fuse-spike/SEMANTICS.md`。

## 支持的操作

| 操作 | 语义 | 约束 |
|---|---|---|
| 目录列举（readdir） | 后备目录实时视图 | 无 `.`/`..` 条目（内核自行解析）；`.partisync-writeback/` 写回日志目录**不出现** |
| 随机读（read） | 已存在文件任意 offset 读 | `O_RDONLY` 打开 |
| 新建文件（create） | `O_CREAT\|O_EXCL` 强制——存在同名即 `EEXIST` | 仅新文件 |
| 新文件顺序写（write） | **仅追加**：offset 必须等于当前长度，跳写/回写 `EINVAL` | release（close）后不可再写打开 |
| getattr/lookup | 实时 `symlink_metadata` | symlink 等不支持类型显式 `EPERM`；`.partisync-writeback/` 路径 `EACCES` |
| unlink（二期，M8-WP07-T02） | **写回日志路径**：先日志（`.partisync-writeback/wal.jsonl` append）→ 应用到 backing → 压实 | 目录 → `EPERM`；不存在 → `ENOENT`；apply 失败 backing 原状（P16） |
| rmdir（二期，M8-WP07-T02） | 同上日志路径 | 仅空目录（非空 `ENOTEMPTY`，拒绝先于日志写入） |
| rename（二期，M8-WP07-T02） | 同上日志路径；同挂载点内跨目录允许，目录拓扑仍由同步管线管理 | 源不存在 → `ENOENT`；目标非空目录 → `ENOTEMPTY`；`.partisync-writeback/` 涉入 → `EACCES` |
| 已存在文件写打开 → release（二期 T03） | **整文件替换**：写入 `.partisync-writeback/staging/` 暂存（预填原内容，顺序追加契约不变），release 时 WAL Replace 先落盘 + 原子 rename 到位（无半提交） | 同路径暂存持有期间第二写打开 `EBUSY`（后写者拒绝——落锤 Q1）；crash 相位由 P16 覆盖 |
| truncate（setattr size，二期 T03） | 整文件替换特例：暂存持有期 = staging 截断（内核 O_TRUNC = open→setattr(0) 序列，fuser 判例）；独立调用 = 原子 Replace 到 size（0 = 清空写） | perm/uid/gid/时间戳形式仍 `EPERM` |
| flush（二期 T03） | close 时同步应用整文件替换（WAL Replace + 原子 rename）——**close 返回时 backing 即新内容**；release 兜底重放（Replace 幂等）。多次 flush（dup fd）安全 | flush 可多次触发；Replace 幂等 |

## 显式拒绝的操作（拒绝先于任何破坏性效果——P15）

| 操作 | errno |
|---|---|
| mkdir / mknod / symlink | `EPERM`（目录拓扑由同步管线管理——二期不变） |
| setattr perm/uid/gid/时间戳形式 | `EPERM`（truncate 形式已开放见支持表） |
| `.partisync-writeback/` 涉入的一切写操作 | `EACCES` |

## 权限模型（ADR-0026 前置条件 1 处置）

- **本挂载面不做内核级权限过滤**：挂载时**不启用** `DefaultPermissions`，
  内核不按 attr 做二次权限检查；授权决策属于 PartiSync 层（与 M2 设备
  身份模型对齐）。
- attr 统一报告后备文件的真实 uid/gid + `0o644/0o755`——仅供客户端显示，
  不构成强制。
- ⚠️ **多用户机器暴露面警示**：挂载点对所有本地用户可读（Linux FUSE 默认
  allow_other 取决于 `/etc/fuse.conf` 的 `user_allow_other`；默认仅挂载
  用户可见）。单用户场景安全；多用户部署须评估暴露面（ADR-0026 §后果
  R2 评审项）。

## 设计意图

- **内容寻址适配**：CAS 中的对象不可变——挂载面天然「读多写少、新版本
  即新文件」，与 mountpoint-s3 对 S3 对象的语义映射同构。
- **覆盖/改名/删除**：一期显式拒绝；二期（M8-WP07）unlink/rmdir/rename
  走本地写回日志 + 已存在文件整文件替换 overlay（差异化方案，调研方案
  **§5.13**——原 §3.3 引用为勘误，随 M8-WP07 修订）——mountpoint-s3
  明确不做、PartiSync 以本地索引可做。
