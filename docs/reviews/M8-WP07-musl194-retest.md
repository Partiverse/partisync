# M8-WP07 前置项：容器 musl 1.94 复测报告（一期偏差清偿）

> Task-ID: M8-WP07-T01 · 日期: 2026-10-03 · SPEC: docs/specs/M8-WP07.md §3
> 前置项「一期偏差清偿：容器 musl 1.94 全套件绿」· 上游: M8-WP01 遗留
> （ADR-0026 前置条件 2；本地 1.94 预演 PASS 见 M8-WP01）·
> 执行: GLM-5.3-Flash (ZCode)

## 1. 结论：**PASS（引擎侧全套件绿）**

| 项 | 值 |
|---|---|
| 环境 | `rust:1.94-alpine`（Docker 29.8.1，darwin/arm64 宿主） |
| 工具链 | rustc **1.94.0**（4a4ef493e 2026-03-02）= rust-toolchain.toml pin 同源，aarch64-unknown-linux-musl |
| 口径 | `cargo test --workspace --exclude partisync-desktop -j 1`（口径理由见 §3） |
| 结果 | **92 个 test binary/doc-test 段：537 passed / 0 failed / 12 ignored（既有 #[ignore]：bench/nightly-only + doc 示例），零失败零 panic** |
| 耗时 | 冷构建+全测 ≈ 50 分钟（-j 1） |

对照 CI（ubuntu-latest gnu 同工具链 1.94.0）：全套件同绿——**musl 与 gnu 行为一致，未发现一期兼容问题**（SPEC R5 风险不成立）。FUSE 一期交付（fuser 纯 Rust + 探针容器路线）经此复测确认可向 WP07 二期推进。

## 2. 探索过程如实登记（6 次环境性失败，非 musl 兼容性问题）

| # | 症状 | 根因 | 处置 |
|---|---|---|---|
| 1 | `unicode_ident rlib not found` | 复用宿主 darwin target 目录跨 host 污染 | `CARGO_TARGET_DIR` 隔离 |
| 2 | `glib-sys` pkg-config 失败 | desktop 壳需要 webkit2gtk（musl 无系统包） | 口径排除 desktop（§3） |
| 3 | rustup 通道 TLS `handshake eof` | 网络抖动 | `RUSTUP_TOOLCHAIN` 直连本地工具链 |
| 4 | `failed to find tool "c++"` | 镜像缺 C++ 工具链（usearch/link-cplusplus 需） | `apk add g++ cmake make` |
| 5 | 日志丢失 | 会话托管中断 | 脱离托管（nohup + 落盘日志） |
| 6/8 | `ld signal 9 (Killed)` / 整容器硬杀 | **Docker Desktop VM 内存上限**（静态 musl 链接大测试二进制） | `-j 2` 不够 → `-j 1` 过；后续复测直接 `-j 1` |
| 7 | （过程跑，干净收尾但日志 tail 窗口截断总量） | — | 第九跑全量日志定版 |

**可复现命令**（九跑终版）：

```bash
docker run --rm -v "$PWD":/ws:ro -v /tmp/musl-target:/tmp/target -w /ws \
  -e CARGO_TARGET_DIR=/tmp/target -e RUSTUP_TOOLCHAIN=1.94.0-aarch64-unknown-linux-musl \
  rust:1.94-alpine sh -c 'apk add --no-cache g++ cmake make >/dev/null; \
  cargo test --workspace --exclude partisync-desktop -j 1'
```

## 3. 口径注记（desktop 排除理由）

partisync-desktop（Tauri 壳）依赖 webkit2gtk-4.1，musl 无对应系统包且**不在 musl 发布面**（release.yml：cli/hub = linux gnu tar、desktop = macOS dmg）——与 M8-WP01「探针容器路线」口径一致。desktop 的平台面由 CI macos/ubuntu job 覆盖。

## 4. 验收回填

- [x] SPEC M8-WP07 §3「一期偏差清偿（前置项）」→ **PASS**（本报告）
- [x] ADR-0026 前置条件 2（1.94 复测）→ 引擎侧证据闭合
