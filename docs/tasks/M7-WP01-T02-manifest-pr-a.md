# Task: capability manifest 模块 + 加载期校验（[P14] 主实现，M7-WP01-T02 (PR-A)）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M7-WP01-T02 (PR-A) |
| **类型** | 实施（R1：新增 module + serde 反序列化校验，[P14] 核心契约） |
| **优先级** | P0（SPEC M7-WP01 §3 T02 第一子任务；T02 拆 PR-A/PR-B 守 ≤400 行） |
| **范围** | `crates/partisync-ext-host/src/manifest.rs` + `src/lib.rs` + `Cargo.toml`（serde/serde_json dev） + `tests/manifest_p14.rs` + 本任务卡 |
| **创建日期** | 2026-09-29 |
| **来源** | SPEC M7-WP01 §2.2（capability 白名单冻结表）+ §3 T02 验收（[P14] 加载即拒） + docs/tests/properties.md P14（PR #27 已登记） |

## 交付物

1. **`manifest` 模块**（`crates/partisync-ext-host/src/manifest.rs`，~210
   行）：`Capability` 枚举（`IndexRead` / `ClockRead`，serde tag = 点号
   `index.read` / `clock.read`，与 SPEC §2.2 表 1:1 对齐）+ `Manifest`
   类型（`tool_name` + `capabilities[]`）+ `validate()`（加载期校验：
   工具名形态 + 撞名拒绝）+ `load(path)`（IO + parse + validate 三阶段，
   任一阶段失败 = 加载拒）+ `ValidateError` / `ManifestError`（手写
   `Display`/`std::error::Error` impl，**零新增顶层依赖**——确认
   workspace 无 `thiserror`，故按 spike / M6-WP03 既有惯用手写）。
2. **`src/lib.rs` 暴露**：`pub mod manifest` + re-export
   `Capability/Manifest/ManifestError/ValidateError`。
3. **Cargo.toml 依赖**：serde + serde_json（与 workspace 同源；属 manifest
   模块必需，非「无聊依赖」新增）；dev-dep `tempfile`（[P14] 探针临时
   目录）。
4. **[P14] 集成探针 ×6**（`tests/manifest_p14.rs`）：
   - 缺 manifest → `ManifestError::Io`
   - 撞名（`asset_read` / `search` / `cas_stats`）→ `ToolNameCollision`
   - 未知 capability（`fs.read` / `net.connect`）→ `ManifestError::Parse`
   - 非法工具名（空 / 含空格 / 超长）→ `BadToolName`
   - 合法 manifest round-trip（重复声明容忍）
   - 公共表示稳定（`Display` / `as_str` 出点号）

## 明确不做（T02a 边界）

- host function 注入（`clock.read` 实装、`IndexRead` trait 引入）→
  T02b（PR-B）
- MCP 工具面接线 + 列举前缀 → T04
- §2.2 演化（白名单表与 Capability 分离）→ 待真实需求触发，不预占

## 验收

- [x] `cargo test -p partisync-ext-host` 16/16 绿（unit 7 + [P14] 探针 6
      + smoke 3 复跑未回归）
- [x] `cargo clippy -p partisync-ext-host --all-targets -- -D warnings`
      零警告；`cargo fmt --all --check` 绿
- [x] `cargo deny check` 零新增豁免（workspace serde/serde_json/tempfile
      已在工作区既有依赖图）
- [x] **零顶层依赖新增**（手写 Error impl 替代 thiserror；serde/serde_json
      走 workspace；tempfile 走 dev-only 3.x 沿 M6-WP03 判例）
- [ ] CI 全绿 + 时长对照（T02b 一并入 PR 序列汇总，避免空 PR 浪费
      CI 资源）

## 钉子清单 disposition（PR-A 触及表面）

n/a — PR-A 不触及 schema migration / unsafe / pub cross-crate API /
crypto/auth / governance 表面。manifest 模块为新 crate 内新 module
（无既有调用方）；dev-dep `tempfile` 走既有 3.x 工作区线位（M6-WP03
判例）。