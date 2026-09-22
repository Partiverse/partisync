title: Rust 性能基准方法论
filename: rust_perf_bench_methodology.md
tags: [rust, performance, bench, criterion, methodology]
updated_ns: 1716240000000000000

# Rust 性能基准方法论

## 工具

- `criterion` 0.5（统计显著 + 火焰图）
- `cargo bench` 子命令
- `iai` 用于冷启动对比（机器码级别）
- 火焰图：`cargo flamegraph` + `pprof`

## 测量对象

- 序列化/反序列化吞吐
- Hash 函数吞吐（BLAKE3 vs SHA-256）
- 全文本检索（tantivy）p95
- 向量检索（usearch HNSW）recall@10 vs QPS

## 测量方法

### 热身

- 5 秒热身（让 CPU 频率稳定到 boost）
- 50 个样本（criterion default）

### 环境

- CPU 隔离：`taskset -c 0` 单核
- ASLR 关：测量吞吐更稳定
- governor：`performance`（不要 `powersave`）

### 噪声

- 同机器其他进程空闲
- 5 次完整测量取中位数
- 偏差 >10% 重跑

## 报告

KPI 报告固化：
- 配置（CPU/内核/OS/编译器版本）
- 命令（可一键复现）
- 数值（mean / median / std / p95）
- 与上次对比（diff %）

## 反模式

- ❌ 在 debug 模式跑 bench
- ❌ 单次跑就报数字（无统计）
- ❌ 不写配置就发报告
- ❌ bench 通过减功能「快」起来