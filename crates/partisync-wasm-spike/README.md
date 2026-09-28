# partisync-wasm-spike — WASM 扩展选型 spike（M6-WP04-T02）

> 独立 workspace：**不列于根 `members`、不进产品依赖图**（SPEC M6-WP04 §3 T02 / §4 非目标）。
> 依赖（wasmtime 47.0.4 等）只存在于本目录的独立 `Cargo.lock`；根 `deny.toml`/CI 不感知本 crate。
> 结论：`docs/reports/M6-WP04-wasm-ext-eval.md`（T01）与 `docs/adr/0025-wasm-extension-runtime.md`（T03）。

## 目录

```
src/host.rs          宿主面：engine（含磁盘编译缓存）+ 默认拒权 linker + demo tool 调用
src/bin/spike_report.rs   冷启动/RTT/拒权探针 汇总（百分比位统计）
src/bin/rss_baseline.rs   空载进程（配合 /usr/bin/time -l 测 RSS 增量）
src/bin/rss_loaded.rs     装载态进程（同上）
tests/deny_probes.rs [P13] 沙箱默认拒权探针 + 零能力工具对照
benches/             criterion 冷启动与 call RTT（SPEC §2.3 指定测法）
guest/demo-tool      demo guest：JSON→JSON tool（wasm32-unknown-unknown + wasm-tools component new）
guest/probe-deny     拒权探针 guest：std FS/net/clock（wasm32-wasip2）
assets/*.wasm        已构建的 component 产物（入仓，重建见下）
deny.toml            spike 独立 deny 配置（零豁免起步）
```

## 重建

```bash
# guests（rustup target add wasm32-wasip2 wasm32-unknown-unknown）
cd guest/demo-tool && cargo build --target wasm32-unknown-unknown --release
wasm-tools component new target/wasm32-unknown-unknown/release/demo_tool.wasm -o ../../assets/demo_tool.wasm
cd ../probe-deny && cargo build --target wasm32-wasip2 --release
cp target/wasm32-wasip2/release/probe-deny.wasm ../../assets/probe_deny.wasm

# host（在 crates/partisync-wasm-spike/）
cargo test --release          # [P13] 探针
cargo run --release --bin spike_report
/usr/bin/time -l target/release/rss_baseline   # 与下行峰值 RSS 相减
/usr/bin/time -l target/release/rss_loaded
cargo bench                    # criterion
cargo deny check && cargo audit
```

## 已知边界（进报告）

- wasmtime 47.0.0–47.0.3 有 RUSTSEC-2026-0269，下限 47.0.4；48.x 需 rustc 1.95、49.x 需 1.96，
  与仓库 1.94.0 pin 不兼容（SPEC R2 实测结论）。
- demo guest 必须用 `wasm32-unknown-unknown` + `wasm-tools component new`：
  wasip2 目标的 std 会保留 `wasi:io/poll` import，零拒权 linker 下无法实例化。
- 测量环境为负载 28–37 的 macOS 开发机，数字偏保守（负载越高越慢）。
