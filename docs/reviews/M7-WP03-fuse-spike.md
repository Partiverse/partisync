# M7-WP03 T01 报告：FUSE spike——mountpoint-s3 式语义实测

> 任务：M7-WP03-T01（SPEC M7-WP03 §3 T01，2026-09-30 批准 PR #47）
> 日期：2026-09-30 · 基线 main=`9e4a311`
> 载体：`crates/partisync-fuse-spike`（独立 workspace，**产品依赖图零改动**）
> 结论：**T01 交付完成**——语义面可行，实测通过；fuser 0.18.0 线位成立
> （进产品依赖图需 ADR-0026 + 用户拍板）

---

## 1. 结论速览

| 项 | 结论 |
|---|---|
| fuser 线位 | **0.18.0 成立**（API 全部源码查证，零凭记忆） |
| 语义可行性 | **PASS**——mountpoint-s3 式「诚实非 POSIX」在 fuser 0.18 上完整可实现，拒绝面探针 2/2 绿 |
| 实测环境 | **Linux 容器真挂载**（`/dev/fuse` + SYS_ADMIN，alpine + rust 1.96.1） |
| 性能 | 随机读 p95 **74.6 µs** / 顺序读 8761 MiB/s（**页缓存路径**，见 §3 解读纪律） |
| 阻塞项 | **0**；无新增依赖进产品图；deny/clippy 面零影响 |

## 2. fuser 0.18.0 API 查证记录（红线：禁凭记忆）

线位经 crates.io 索引确认 `max_stable = 0.18.0`（`cargo add` 解析落位，非记忆）。
逐项查证点（全部在 `~/.cargo/registry/src/*/fuser-0.18.0/`）：

| 查证项 | 位置 | 结论 |
|---|---|---|
| `Filesystem` trait 签名（15 方法） | `src/lib.rs:404-545` | read/write/create/setattr/readdir 逐签名记录（本 spike 实现按此落位） |
| `FileAttr` 字段 | `src/lib.rs:173-200` | 16 字段含 `flags`/`blksize`/`crtime`（macFUSE 扩展） |
| `mount` / `spawn_mount` 入口 | `src/lib.rs:1051+` | `mount` 阻塞版 / `spawn_mount` 返回 `BackgroundSession`（探针用后者） |
| `Config` 结构 | `src/mnt/mount_options.rs:10` | **`#[non_exhaustive]`**——不能 struct 字面量，必须 `Config::default()` + 字段赋值（本 spike 首次编译即撞此坑，已修） |
| `Errno` 常量 | `src/ll/mod.rs:54+` | `Errno::EPERM/EACCES/EEXIST/EINVAL/EISDIR/ENOTDIR/EIO/EBADF/ENOENT` 全在（`NonZeroI32` newtype） |
| `OpenFlags::acc_mode()` | `src/open_flags.rs:37` | 返回 `OpenAccMode`（`O_RDONLY`/`O_WRONLY`/`O_RDWR`）——写打开拒绝面据此实现 |
| `ReplyDirectory::add` 需 `&mut self` | `src/reply.rs` | 参数须 `mut reply: ReplyDirectory`（编译错误实证） |
| **build.rs 平台分支** | `build.rs:1-56` | **关键发现**：Linux+无 libfuse feature → `pure-rust`（**免 C 库依赖**）；macOS → probe macFUSE pkg-config（需 `macFUSE >= 2.6.0`），或 feature `macos-no-mount` 跳过挂载；其他平台 → libfuse3/2 fallback（fail hard） |
| pure 挂载机制 | `src/mnt/fuse_pure.rs:105-130` | Linux/macOS 直走 `mount(2)` syscall（`fuse_mount_sys`），**不依赖 fusermount 助手**（`AutoUnmount` 选项才需要） |
| readdir 语义 | 实现惯例 | 不回 `.`/`..`（内核自行解析）——fuser 示例 passthrough 反而回显，本 spike 采内核惯例并注记 |

**条件编译结论（macOS 本机路径）**：fuser 提供 `macos-no-mount` feature
（`Cargo.toml:53`，空 feature 列表）供无 macFUSE 环境编译验证——本 spike
依赖带该 feature：Linux 上 `build.rs` 先命中 pure-rust 分支**不受影响**，
macOS 本机可编译但**不可挂载**（需 FUSE-T/macFUSE，R1 前置未满足）。

## 3. 实测环境与数字（实测证据，非纸面）

### 环境（R1 三级降级：路径 ② 命中）

| 层级 | 内容 |
|---|---|
| 主用 | **Linux 容器真挂载**：`alpine` + `rust 1.96.1`（apk）+ `--device /dev/fuse --cap-add SYS_ADMIN`；fuser pure-rust 实现直走 `mount(2)`（§2 查证） |
| 未用 | 路径①（macFUSE 安装需 sudo 密码，headless 不可得）· 路径③（纸面）——**未触发** |
| toolchain 偏差登记 | 容器 rust **1.96.1** vs 仓 pin **1.94**：spike 向上兼容验证（更高版本编译通过 = 1.94 亦可）；实施 WP 若采纳 fuser 须在 1.94 pin 下复测（ADR-0026 前置） |

### 语义探针（`probe_mount_lifecycle_and_semantics`，2/2 绿）

| 探针 | 断言 | 结果 |
|---|---|---|
| ① readdir 可见性 | 预置 `a.txt` 在挂载点列出 | PASS |
| ② 随机读 | `seek(6)` + 5B = `"parti"` | PASS |
| ③ 拒绝面 | `unlink`→EPERM / `rename`→EPERM / 已存在文件写打开→EACCES / `mkdir`→EPERM | PASS（逐项 errno 精确断言） |
| ③b 拒绝后完整性 | 拒绝面探针后文件原样可读（**拒绝先于破坏性效果**，P15 核心） | PASS |
| ④ 顺序写 | `create_new` + 两段 write → 后备文件 `part1-part2-`；release 后写打开→EACCES | PASS |
| ⑤ O_EXCL | 同名 `create_new`→EEXIST | PASS |

### 性能（`probe_random_read_bench`，1 MiB 伪随机文件）

| 指标 | 实测 | 解读纪律 |
|---|---|---|
| 随机读 4 KiB（n=1000） | min 1.1 / **p50 1.2** / p95 74.6 / max 202.1 µs | **p50 = 页缓存命中路径**（同一文件反复读，VFS 缓存直接满足，不经 FUSE 往返）；**p95/max 才反映真实 FUSE 往返**（~75-200 µs/次往返 + seek+read 系统调用放大） |
| 顺序读吞吐 | 8761.8 MiB/s（20 轮 / 0.002 s） | 页缓存路径上限，非设备 I/O 能力；仅作「FUSE 不成为顺序读瓶颈」的弱证据 |

**未测项（诚实登记）**：无 `O_DIRECT` 冷缓存真实设备读（容器内无 backing
设备、无法 drop caches 免特权）；macOS FUSE-T（NFS 桥）路径未测（R1
前置未满足）。**实施 WP 若采纳，须补冷缓存读基准 + macOS 本机复测**。

## 4. 语义契约（P15 候选）

`crates/partisync-fuse-spike/SEMANTICS.md`——mountpoint-s3 判例（调研方案
§9.3 语义诚实）。核心不变量：**拒绝面操作在破坏性效果发生前显式失败**
（unlink/rmdir/rename/mkdir/mknod/symlink/setattr → EPERM；已存在文件写
打开 → EACCES），新文件写为**严格顺序追加**（跳写 → EINVAL，release 后
不可再写打开）。P15 候选行已登记 `docs/tests/properties.md`（spike 期
登记，实施 WP 转正——沿 M6-WP04 P13 判例）。

**设计要点**：拒绝面返回 `EPERM`（语义「不允许」）而非 trait 默认
`ENOSYS`（语义「未实现」）——拒绝语义必须可被客户端区分（P15 探针
逐项断言 errno 即此）。

## 5. 后续项

| 项 | 去向 |
|---|---|
| ADR-0026（fuser 线位 + 是否进产品依赖图） | T04（本 WP 收官任务），状态「草案」未接受 |
| 跳写探针的 `O_DIRECT` 形态 | 页缓存下 VFS 侧无法构造到达 `setattr`/`write` offset 检查的路径；SEMANTICS.md 已登记为后续项 |
| 冷缓存读基准 / macOS FUSE-T 复测 | ADR-0026 采纳前的前置验证项 |
| 覆盖/改名/删除（写回日志 + overlay） | 实施 WP 二期（调研方案 §3.3 差异化项），本 spike 显式拒绝不做 |
| Samba 桥接评估 | T02（本 WP） |

## 6. 复核日志

- 执行：GLM-5.3-Flash（ZCode 会话，M7-WP03-T01）
- 探针：容器内 2/2 绿（含 nocapture 实测数字输出，无 SKIP 标记）
- 本地：macOS 编译验证绿（`macos-no-mount`）；探针本地 SKIP 路径优雅（无 /dev/fuse 时跳过并 stderr 留痕，R2 预案生效）
- 环境踩坑登记：Docker Hub rust 官方镜像在本环境近乎不可达（10 min 零 layer）→ 改 alpine + apk rust；容器 `-v` 必须用 spike 目录绝对路径（误挂仓库根曾触发全工作区 glib 构建失败——与 spike 无关）
