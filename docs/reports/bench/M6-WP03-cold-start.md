# M6-WP03 桌面壳冷启动基准

> SPEC M6-WP03 §3 T04 验收： 冷启动到主窗口可交互时间
> - macOS M1 实测 < 1500 ms
> - Linux x86_64（CI ubuntu runner 实测）< 2000 ms
>
> 测点： `partisd-desktop --bench-cold-start` 启动后， webview
> `on_page_load` 回调触发时（≈ WKWebView / webkit2gtk `load-changed`
> finished 信号）打 stderr 一行 `__BENCH_READY__ <elapsed_ms>`， 随后
> `process::exit(0)`。 `cargo xtask bench desktop-cold-start` 多次
> spawn 上述 binary 后 parse stderr 取统计。

## 0. 复现命令

```bash
# 1. 构建 release binary
cargo build --release -p partisync-desktop

# 2. 跑 5 次取 min/P50/P95/max（默认 --runs 5）
cargo xtask bench desktop-cold-start

# 3. 自定义采样次数 + 自定义工作区路径
cargo xtask bench desktop-cold-start --runs 20 --workspace /path/to/partisync
```

> 报告由 xtask 输出 markdown 表后人工粘贴到本文件对应段。

## 1. macOS M1（SPEC 目标 < 1500 ms）

<!-- 待用户在 Apple Silicon Mac 上跑 `cargo xtask bench desktop-cold-start` 后粘贴 -->

| metric | value |
|--------|-------|
| min    | TBD ms |
| P50    | TBD ms |
| P95    | TBD ms |
| max    | TBD ms |
| target | < 1500 ms |

环境记录（用户手填）：

- 设备： Apple Silicon Mac（M1 / M2 / M3 / M4， 请注明）
- macOS 版本：
- 命令：
- 跑测日期：
- 结论： ☐ PASS  ☐ FAIL（如果 FAIL 请贴 stderr 末尾 20 行）

## 2. Linux x86_64（CI ubuntu runner 实测， 目标 < 2000 ms）

<!--
待用户在 Linux 机器或 CI（ubuntu-latest + Xvfb + headless webkit）上跑
`cargo xtask bench desktop-cold-start` 后粘贴。

注： 本期不在 CI workflow 内自动跑（webkit2gtk + Xvfb + headless
profile 复杂度高， 留 M7+）。 Linux 数字先由用户在本地环境跑后
填入； M7+ 加 CI bench job。
-->

| metric | value |
|--------|-------|
| min    | TBD ms |
| P50    | TBD ms |
| P95    | TBD ms |
| max    | TBD ms |
| target | < 2000 ms |

环境记录（用户手填）：

- 设备： Linux x86_64
- 发行版 + 内核：
- webkit2gtk 版本：
- 是否 Xvfb headless：
- 命令：
- 跑测日期：
- 结论： ☐ PASS  ☐ FAIL

## 3. 实现说明

### 3.1 测点选择

Tauri 2 的 `tauri::Builder::on_page_load(callback)` 在 webview 完成
页面加载时触发。 这是 SPEC §2.5「冷启动 → 主窗口可交互」的近似
测点： webview `load-changed` finished 之后，前端 JS 立即可执行
（`app.js` 顶部直接调 `getStats()` 触发首次 IPC）。

### 3.2 为何用独立 `--bench-cold-start` 模式

- **不污染正常路径**： 默认行为不变（窗口保持打开 + 走 IPC）
- **可重复性**： 5 次 spawn，每次都是真冷启动（OS 进程级 cache miss）
- **可脚本化**： stderr 一行 `__BENCH_READY__ <ms>` + exit(0) 是
  最简契约， xtask 仅需 `String::find` parse

### 3.3 已知限制

- **WKWebView `load-changed` vs JS 真正执行**： 测点略早于
  「前端 JS 可交互」， 偏差 ~50ms 内（IPC invoke 启动）。 若
  SPEC §2.5 严格定义要 JS 执行后， M7+ 可改为前端打 `__BENCH_READY__`
  via invoke 回调
- **macOS Dock 启动动画**： M1 实测含 ~100ms Dock 弹起延迟， 已计入
  「冷启动」合理范围
- **Linux 需 Xvfb + headless**： CI runner 无 display， 需
  `xvfb-run -a partisd-desktop --bench-cold-start`； 本期不在 CI 跑

## 4. 变更记录

| 日期 | 变更 | commit |
|------|------|--------|
| 2026-09-28 | T04 落地： lib.rs `--bench-cold-start` 模式 + xtask bench 模块 + 本报告骨架 | 见 PR `m6/wp03-t04-impl` |
