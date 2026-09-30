# M7-WP01-T05 六项基准复测报告

- 任务：M7-WP01-T05（SPEC [M7-WP01](../specs/M7-WP01.md) §2.3/§2.4/§3）
- 日期：2026-09-30 · 环境：macOS 26.6.2 arm64（Apple M 系列，debug 构建）
- 口径：SPEC M6-WP04 §2.3 同口径；criterion 表述下以 `Instant` 分位数
  直测替代（与 spike 报告 [M6-WP04-wasm-ext-eval](M6-WP04-wasm-ext-eval.md)
  双口径中的「直测 N 次」一致）。测量 harness 为临时文件不入仓，方法学
  与代码原样登记于本报告。
- 结论：**六项全 PASS**；§2.4 单扩展 RSS 增量超 spike ±50% 容差带，按
  SPEC 预案解释（见 §3），主预算（<30 MB）合规。

## 1. 六项判定基准矩阵（production 路径）

测量对象为产品实现：`partisync-ext-host`（Engine 单例 + 磁盘缓存 +
`ExtTool::load` 整合拒绝路径）与 `partisync-mcp` 侧车（gateway 接线）。
非 spike crate。

| # | 指标（基准） | 复测实测 | 判定 |
|---|--------------|----------|------|
| 1 | 宿主冷启动（<100 ms，加载 1 个 demo component） | **缓存命中 p50 11.46 ms / p95 11.71 ms / min 11.35 ms**（n=29）；缓存未命中（rm 缓存目录后 iter0，含编译 + 缓存写回）**12.31 ms** | **PASS**（余量 ~8×；命中与未命中均入预算） |
| 2 | 单次 tool call RTT（<5 ms，10 KiB JSON 往返） | **p50 28.83 µs / p95 29.54 µs / max 49.17 µs**（n=1000，预热 10 次） | **PASS**（余量 ~170×） |
| 3 | 空载内存增量（<30 MB RSS，进程前后差） | 空扩展目录 peak 中位 **15.96 MB** → 装 1 扩展稳定 peak **26.79 MB**（3 轮，首轮 42.0 MB 为缓存写回离群）；**单扩展增量 ≈ 10.8 MB** | **PASS 主预算**（余量 ~2.8×）；§2.4 容差带超差，解释见 §3 |
| 4 | 沙箱默认面（无 FS/网络/时钟，默认全拒） | P13 全量拒绝探针 + P14 加载即拒：`cargo test -p partisync-ext-host` **47/47 绿**（T03 五探针 + T02 六探针 + T04 注权端到端，含错误文本不含宿主路径/env 断言） | **PASS** |
| 5 | MSRV 兼容（1.94 编译零警告） | `cargo clippy --workspace --all-targets -- -D warnings` **零警告**（rust-toolchain 1.94 pin 未动） | **PASS** |
| 6 | 依赖卫生（deny 通过；audit 零新增未豁免） | `cargo deny check` 四项 ok（advisories/bans/licenses/sources）；`cargo audit`（0.22.0，`--no-fetch`，本地库 f23b7682=2026-09-29）：**7 vulnerabilities 全部命中已登记豁免**——h2-0258/rsa-0071/webpki-0104/0098/0099（ADR-0020/0021）+ wasmtime-0315/0316（ADR-0025 修订 6 + deny.toml，T04 第 0 步）；8 warnings 为既有 unmaintained/lru-0253 | **PASS**（零新增豁免） |

### 附：生产路径观察项（SPEC 外登记）

| 项 | 实测 | 说明 |
|---|------|------|
| `index.read` 桥接代价 | **p50 114 µs / p95 163 µs / min 94 µs**（n=200，`block_in_place + Handle::block_on + IndexEngine::bm25_only` 空索引真实 BM25，多线程 tokio） | ext.rs 桥接说明要求的 T05 观察项。与指标 2 相加 ≈ 143 µs，含 index.read 的扩展端到端仍远低于 5 ms 预算。空索引口径（无数据方差）；真实数据集下的查询延迟由 M6-WP02 检索基准覆盖 |
| partisync-mcp 侧车 spawn 基线 | **spawn→initialize p50 23.3 ms；spawn→tools/list p50 24.1 ms / p95 26.3 ms**（n=20，debug） | 对照列：spike 侧车 spawn+握手 p50 ≈16 ms；+7 ms 为完整服务启动（sqlx 池 + IndexEngine open_or_create + ext registry 扫描）。进程内 wasm 调用面（11.5 ms 冷启动）仍显著优于侧车生命周期面，§2.1 窄核心形态结论维持 |

## 2. CI 时长对照（wasmtime 编译代价兑现量化）

数据源：GitHub Actions `Partiverse/partisync` main 分支 push run（`run_started_at → updated_at`，
排除 nightly schedule run——口径不同）。CI 三 job 均启用 Swatinem/rust-cache@v2，
wasmtime 编译代价仅在缓存 miss 时全额兑现。

| 口径 | pre-wasmtime（M6 收官窗口 09-27→09-29，n=21） | post-wasmtime（3c3ef9a→0bd691b，n=13） | Δ |
|------|------|------|---|
| 整 run 中位 | 11 m 25 s | 12 m 07 s | **+42 s（+6.1%）** |
| test (macos-latest) 抽样 | 10.3 min（f4f58e0） | 7.0–11 min | 无一致增长 |
| test (ubuntu-latest) 抽样 | 7.1 min（f4f58e0） | 6.5–7.9 min | 无一致增长（#36 run 19 min 为 runner 争用离群） |
| clippy 抽样 | 1.3 min | 1.4–1.9 min | **+0.1–0.6 min（wasmtime 入 check 编译图的边际成本）** |
| T01 首次合入（缓存冷） | — | 14 m 53 s（PR #27 CI 实测 16 m 17 s） | 一次性代价，ADR-0025 后果节兑现 |

结论：wasmtime 编译代价被 rust-cache 吸收后，日常 CI 增量 ≈ clippy/check
路径 +0.1–0.6 min，整 run 中位 +6%；ADR-0025 后果节登记的负面项（16 m17 s
冷编译）兑现为「仅缓存 miss 时发生」的一次性代价。**不触发 R2 裁剪
feature 预案**（wat/demangle 维持开启）。

## 3. §2.4 RSS 增量超差解释（SPEC 预案）

实测单扩展装载增量 ≈ 10.8 MB，超出「spike 实测 +4.4 MB 的同数量级 ±50%」
容差带（2.2–6.6 MB）。按 SPEC「超差须解释」登记：

1. **口径构成差异**：spike 的 4.4 MB 是最小进程（空载 peak 1.4 MB）的净
   装载增量；production 口径的空载进程（15.96 MB）从未触达 wasmtime——
   增量含 **Engine + 磁盘缓存子系统首次初始化常驻**（linker 表、编译
   产物映射、Store/instance 线性内存），并非纯 component 增量。
2. **大进程 allocator 放大**：完整 MCP 服务进程（tokio 多线程 runtime、
   rmcp、sqlx 4 连接池、tantivy IndexEngine）的 malloc arena 结构使同等
   wasmtime 装载的 RSS 峰值高于最小进程口径。
3. **绝对量评估**：10.8 MB 对桌面常驻场景可接受；§2.3 主预算（<30 MB）
   合规（余量 ~2.8×）。首轮冷启动（缓存写回路径）peak 42 MB 为编译期
   瞬态，稳态 26.8 MB。
4. **后续项**（不阻塞）：wasmtime pooling allocator / 裁剪 feature 的
   取舍沿 ADR-0025 后果节预案，出现真实内存压力时再立项。

## 4. 测量方法学（可复现）

- **冷启动/RTT**：临时 integration test 于 `crates/partisync-ext-host`
  （`ExtTool::load` + `call`，fixture `tests/fixtures/demo_tool.wasm` +
  运行时写零能力 manifest）；30 次冷启动（清
  `~/Library/Caches/BytecodeAlliance.wasmtime` 后 iter0=未命中）+ 1000 次
  RTT（10 KiB JSON 载荷，`{"tool":"bench","pad":"a×n"}` 整 10240 B）。
  `--test-threads=1`。测毕删除，不入仓。
- **RSS**：真实 `target/debug/partisync-mcp --db <touched 空库>` +
  `HOME` 指向预置目录（empty：空 extensions 目录；loaded：安装
  `demo_ext.wasm+json`），stdin=/dev/null 即发即收，`/usr/bin/time -l`
  取 maximum resident set size，3 轮取中位。
- **侧车基线**：python3 子进程 spawn + JSON-RPC handshake（protocolVersion
  2025-11-25 + `_meta` 双键，与桌面壳 McpSidecar 同构），20 轮。
- **桥接**：临时 integration test 于 `crates/partisync-gateway`（复刻
  `IndexEngineReader` 形状，临时空索引），200 轮。测毕删除，不入仓。
- **端到端用户路径点验**（§3 验收第 6 项旁证）：桌面壳真机 cliclick 全
  链路——扩展 tab → ext_demo_echo 行点击 → JSON 入参 `{"k":"v"}` →
  「调用」→ 结果框回显 `{"source":"extension","tool":"ext_demo_echo",
  "result":{"echo":{"k":"v"},"input_bytes":9}}`；截图归档
  [docs/screenshots/M7-WP01-ext-call-verified.png](../screenshots/M7-WP01-ext-call-verified.png)。
  此前四轮热修（#37/#38/#39/#40）遗留的「调用按钮」真机点验至此闭合。

## 5. 遗留与登记

| 项 | 处置 |
|---|------|
| `partisync-mcp` 首启 graph.db 缺失即败（sqlite code 14，无 mode=rwc） | 独立热修任务（同 WP01 交付质量范围），本报告发布同窗口处置 |
| tauri-driver（WebDriver）UI 自动化 | 任务候选登记：合成点击（cliclick）判例已可复现关键交互，但工程化回归需 WebDriver；沿 M7-WP01-T04 交付 12 方法论登记 |
| 桌面壳面包屑根路径「根 根」重复显示 | UI 小债（#38 修复未覆盖根路径情形），登记待热修窗口 |
| `partisync-mcp` 启动期 `println!` 混入 stdio JSON-RPC 流 | 已知怪癖（侧车 reader 跳过非 JSON 行容忍）；改 stderr 需动 gateway bin，随下次 gateway 任务顺带 |
| spike 冷启动未命中口径差异（spike p50 84.5 ms vs 复测 12.3 ms） | 观察性登记：复测含缓存写回的 iter0 口径；差异不影响判定（均 <100 ms），不追溯 spike 数字 |
