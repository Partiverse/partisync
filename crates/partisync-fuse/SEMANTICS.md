# PartiFuse 语义声明（SEMANTICS.md，产品版）

> 学 mountpoint-s3 的 SEMANTICS.md 判例（调研方案 §9.3 语义诚实原则）：
> 本挂载面**明确非 POSIX**。一期（M8-WP01）支持的操作与拒绝的操作逐条
> 列出，不做含糊的「全 POSIX」承诺。任何语义变更必须先改本文件（语义
> 先行）。spike 版溯源：`crates/partisync-fuse-spike/SEMANTICS.md`。

## 支持的操作

| 操作 | 语义 | 约束 |
|---|---|---|
| 目录列举（readdir） | 后备目录实时视图 | 无 `.`/`..` 条目（内核自行解析） |
| 随机读（read） | 已存在文件任意 offset 读 | `O_RDONLY` 打开 |
| 新建文件（create） | `O_CREAT\|O_EXCL` 强制——存在同名即 `EEXIST` | 仅新文件 |
| 新文件顺序写（write） | **仅追加**：offset 必须等于当前长度，跳写/回写 `EINVAL` | release（close）后不可再写打开 |
| getattr/lookup | 实时 `symlink_metadata` | symlink 等不支持类型显式 `EPERM` |

## 显式拒绝的操作（拒绝先于任何破坏性效果——P15）

| 操作 | errno |
|---|---|
| 已存在文件写打开（`O_WRONLY`/`O_RDWR`，含 append） | `EACCES` |
| unlink / rmdir / rename / mkdir / mknod / symlink | `EPERM` |
| setattr 全部形式（truncate/perm/uid/gid/时间戳） | `EPERM` |
| 已存在文件任何修改路径 | 不存在（写打开层已前置拒绝） |

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
- **覆盖/改名/删除**：一期不做（显式拒绝）；差异化方案（本地写回日志 +
  overlay，调研方案 §3.3）留二期独立 WP。
