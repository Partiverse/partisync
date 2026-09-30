# M8-WP01 夜间工作笔记（T-N1 产出）

> 会话：2026-09-30 夜（ZCode，长任务窗口）。SPEC M8-WP01（PR #54）待批准；
> 本笔记记录批准前合法的准备工件结果。

## 1. 1.94 编译预演（SPEC §6 R1 排雷）——**PASS**

```
$ cd crates/partisync-fuse-spike && rustup run 1.94.0 cargo check
    Checking partisync-fuse-spike v0.1.0 (…/crates/partisync-fuse-spike)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.42s
```

- 本机 `rustup toolchain list`：1.94.0-aarch64-apple-darwin（active，即仓
  rust-toolchain.toml 钉的版本）；
- fuser 0.18.0 + `macos-no-mount` feature 在 1.94.0 **编译零警告**；
- 结论：风险 R1（fuser×1.94 不兼容）**本地排除**；Linux 侧 1.94 验证
  留 T02 容器实测（alpine rust 为 1.96.1，须以 `rustup` 装 1.94 或用
  `docker run rust:1.94-slim`——后者本环境拉取曾不可达，见环境备忘）。

## 2. spike 探针 → 产品探针移植清单（T02 消费）

源：`crates/partisync-fuse-spike/tests/probe_mount.rs`（真挂载 2/2 绿）

| # | spike 断言 | 产品探针形态 | 差异点 |
|---|---|---|---|
| 1 | `mount_once` 环境门控（/dev/fuse 检测 + SKIP stderr 留痕） | 原样移植 | crate 路径改 `partisync_fuse::PartiFuse` |
| 2 | readdir 可见性（预置 a.txt 列出） | 原样 | — |
| 3 | 随机读 offset=6 5B | 原样 + 增加跨块 offset（512/4096 边界） | 覆盖读路径边界 |
| 4 | unlink→EPERM / rename→EPERM / mkdir→EPERM | 原样 + rmdir/symlink/mknod 补齐（spike 实现有实现面，探针未逐项） | P15 全量 |
| 5 | 已存在文件写打开→EACCES | 原样 | — |
| 6 | 拒绝后文件原样（读回一致） | 原样 | P15 核心 |
| 7 | 顺序写 create→两段 write→后备文件一致 | 原样 | — |
| 8 | release 后写打开→EACCES | 原样 | — |
| 9 | 同名 create_new→EEXIST | 原样 | — |
| 10 | 跳写→EINVAL | **新增**（spike 因页缓存无法构造，产品探针用 `O_DIRECT`？——**开放**：VFS 侧构造路径待 T02 实测定案，构造不出则 SEMANTICS.md 口径维持「防线前置」说明） | 见 SEMANTICS.md 登记项 |
| 11 | 随机读微基准（n=1000 4KiB） | 移入 T03 基准报告（不入探针） | 与冷缓存口径分离 |
| errno 常量 | 本地 const（Linux 值） | 原样 | 探针仅 Linux 容器执行 |

## 3. 产品 SEMANTICS.md 要点（T01 消费，基于 spike 版增量）

- 权限模型一节（SPEC §2.3）：**关闭 DefaultPermissions**；attr 统一报告
  挂载进程 uid/gid；写类操作 FUSE 层前置拒绝——「本挂载面不做内核级
  权限过滤，授权决策属于 PartiSync 层」；
- 多用户机器暴露面警示（R2 评审三问之一）；
- 其余语义行沿 spike 版逐条保留（支持面/拒绝面表格）。

## 4. 任务卡清单状态

- [x] `M8-WP01-T01.md`（fuser 入根 + crate 骨架 + SEMANTICS 产品版 + P15 转正）
- [x] `M8-WP01-T02.md`（探针移植 + 容器实测 + gateway 组装）
- [x] `M8-WP01-T03.md`（冷缓存基准 + macOS 复测 + 报告）

## 5. 晨间待办移交

1. PR #54 批准状态确认（T02/T03 是否已执行以其为前提）；
2. T-N1 工件随 SPEC PR 同分支提交（本文件 + 任务卡 + SEMANTICS 草稿）；
3. Linux 1.94 容器方案待定：`rustup toolchain install 1.94.0-x86_64-unknown-linux-musl`（alpine musl 目标）在 apk rust 内可行性与网络成本。
