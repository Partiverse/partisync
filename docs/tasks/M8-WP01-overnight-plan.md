# 夜间长任务计划（2026-09-30 夜 → 2026-10-01 晨）

> 本文件是夜间会话的自包含作业指导：不依赖当日会话上下文即可执行。
> 晨间用户验收点见 §4。

## 0. 前置状态（2026-09-30 晚）

- main = `928d55f`：M7 关账落档（ADR-0026 已接受 / G3 三签 / Reviewed-By
  全量补认 / deny job 根修 + yoke-derive 0.8.4 升级）。
- **PR #54 OPEN**：M8-WP01 SPEC 草案（FUSE 实施一期）——**等待用户批准，
  夜间不得自行合并**（铁律 1：合入=批准）。
- 并行会话可能在做 M7-WP04（M8 路线图提案）——**全程独立 git worktree**
  （协议 §3）：`git worktree add ../partisync-night m8/wp01-impl`。

## 1. 夜间任务序列（按序执行；1 为无条件，2–4 视批准状态）

### T-N1（无条件）：实施准备工件（SPEC 批准前合法）

1. 任务卡起草：`docs/tasks/M8-WP01-T01.md`（fuser 入根 + crate 骨架）、
   `T02.md`（语义实现 + 探针移植）、`T03.md`（基准 + 复测 + 冒烟）——
   按 M7-WP03-T01 任务卡格式；
2. spike 探针移植清单：逐条列 `crates/partisync-fuse-spike/tests/probe_mount.rs`
   的断言 → 产品探针形态（含环境门控 SKIP 判例引用）；
3. `crates/partisync-fuse/SEMANTICS.md` 产品版草稿（基于 spike 版 + §2.3
   权限模型文字）；
4. **1.94 编译预演**：在 spike workspace 用 `rustup run 1.94 cargo check`
   验证 fuser 0.18 兼容性（风险 R1 提前排除；结果记录待 T01 消费）。
   产出落 `docs/tasks/M8-WP01-overnight-notes.md`。

### T-N2（PR #54 已合并=批准后）：T01 实施

按 SPEC §5 文件清单（**清单即锁，不越界**）：
- fuser 入根 `[workspace.dependencies]`（线位 `>=0.18.0, <0.19`）；
- `crates/partisync-fuse` 骨架（lib 从 spike 移植 + 产品化：模块拆分/
  文档/错误面收敛）；SEMANTICS.md；
- P15 转正（properties.md 候选行→正式行，测试列改产品探针路径）；
- 门禁三件套（fmt/clippy -D warnings/test）本地全绿；
- PR 开出（≤400 行；挂 M8-WP01-T01）；**合入须 CI 绿且无并行冲突——
  若 PR #54 尚未合并，本 PR 不得先合（依赖 SPEC 冻结）**。

### T-N3（T01 合并后）：T02 语义实现 + 探针

- 产品探针移植 + 容器真挂载实测（命令与输出入 PR 正文——协议 §4.1
  冒烟门禁：`docker run --rm --device /dev/fuse --cap-add SYS_ADMIN
  -v <repo>:/w -w /w rust:1.94 容器内 apk add rust cargo 后 cargo test
  -p partisync-fuse --test probe_mount -- --nocapture`；alpine+apk 路线
  沿 M7 判例，Docker Hub rust 镜像在本环境不可达）；
- gateway 组装接线（`partisync-gateway` 增挂载子命令，SPEC §5 内）；
- PR 开出（挂 M8-WP01-T02）。

### T-N4（T02 合并后）：T03 基准 + 复测 + 收尾

- 冷缓存读基准（O_DIRECT/drop-caches，n≥200，报告含页缓存口径对比）；
- macOS FUSE-T 复测（环境可得则做；不可得则登记触发条件——沿 T02 判例）；
- `docs/reviews/M8-WP01-bench.md` 报告 + PR（挂 M8-WP01-T03）；
- 记忆更新（memory/ 新文件 + MEMORY.md 索引行）。

## 2. 红线（夜间尤其）

- **不合并任何等批准的 PR**（#54 及夜间新开的 SPEC 类 PR）；
- 不越 SPEC §5 文件清单（清单即锁）；
- 不动 `.github/`、`deny.toml` 阈值、`rust-toolchain.toml`（ADR 路径才可）；
- 每任务一会话一分支一 Task-ID；commit 带 trailer 块；
- 发现并行会话占用工作树 → 立即 worktree 隔离，不 reset 对方分支
  （M7-WP03 事故判例：先 `git status` 确认干净再操作）。

## 3. 环境备忘（实测判例）

- Docker Hub rust 官方镜像不可达 → `alpine` + `apk add rust cargo`
  （1.96.1；1.94 pin 的 1.94 复测用 `rustup` 或登记偏差）；
- 容器挂载宿主目录做 cargo target 极慢 → target 用容器内路径；
- `docker run` 脚本留守护进程会悬挂 → 外层 `timeout` 保护；
- cargo audit gix fetch 恒败 → 手动 `git -c http.version=HTTP/1.1 pull`
  `~/.cargo/advisory-db` + `cargo audit --no-fetch`；
- gh GraphQL EOF → REST 旁路；push 用 HTTP/1.1。

## 4. 晨间用户验收点

1. PR #54（SPEC）：批准与否 → 决定 T02+ 是否已执行；
2. 夜间产出 PR 队列（T01/T02/T03 依批准状态）+ `overnight-notes.md`；
3. M7 关账确认：PR #53（ADR-0026 接受 + G3 三签 + Reviewed-By 补认）
   已合入 main=`928d55f`+。
