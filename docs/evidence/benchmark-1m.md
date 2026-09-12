# 100万资产检索性能基准测试报告 (1M Asset Benchmark)

- **测试日期**: 2026-09-12 10:50:14 UTC
- **脚本入口**: `scripts/benchmark-1m.sh` (实现项 T6-02)
- **验证项**: 解决审计 T6-04（100万资产 p95 < 100ms 实测证据）
- **MeiliSearch 实例**: http://localhost:7700
- **索引资产总数**: 1000000
- **LMDB 数据库体积**: 1408.01 MB
- **并发客户端数**: 50
- **请求总数**: 5000

---

## 1. 压测完整执行日志

```text
=== partisync benchmark ===
Target Docs : 1000000
Start Index : 0
Batch size  : 5000
Concurrency : 50 clients
Requests    : 5000 total
Only Search : true

--- Phase 1: Skipping index (--only-search specified) ---
Documents in index: 1000000

--- Phase 2: Search benchmark (50 clients, 5000 requests) ---

=== Results (5000 ok / 0 errors) ===
Duration : 3.49s
RPS      : 1434.57 req/s
p50      : 43.01 ms
p90      : 44.13 ms
p95      : 44.79 ms
p99      : 45.96 ms
min      : 0.30 ms
max      : 62.95 ms
avg      : 34.63 ms

Latency distribution (ms):
      0.3 -     3.4 ms |   917 | █████████████
      3.4 -     6.6 ms |   102 | █
      6.6 -     9.7 ms |     5 | 
      9.7 -    12.8 ms |    10 | 
     12.8 -    16.0 ms |    17 | 
     16.0 -    19.1 ms |    28 | 
     19.1 -    22.2 ms |     3 | 
     22.2 -    25.4 ms |     4 | 
     25.4 -    28.5 ms |     1 | 
     28.5 -    31.6 ms |     0 | 
     31.6 -    34.8 ms |     4 | 
     34.8 -    37.9 ms |     1 | 
     37.9 -    41.0 ms |     0 | 
     41.0 -    44.2 ms |  3440 | ██████████████████████████████████████████████████
     44.2 -    47.3 ms |   433 | ██████
     47.3 -    50.4 ms |    12 | 
     50.4 -    53.6 ms |     8 | 
     53.6 -    56.7 ms |     9 | 
     56.7 -    59.8 ms |     4 | 
     59.8 -    63.0 ms |     2 | 

PASS: p95 (44.79 ms) < 100 ms
```

---

## 2. 指标提取与 MCD 达标判定

从基准测试结果中提炼核心指标：
- RPS      : 1434.57 req/s
- p50      : 43.01 ms
- p90      : 44.13 ms
- p95      : 44.79 ms
- p99      : 45.96 ms
- min      : 0.30 ms
- max      : 62.95 ms
- avg      : 34.63 ms
- PASS: p95 (44.79 ms) < 100 ms

- **MCD 目标**: 100 万资产搜索 p95 < 100ms
- **实测判定**: ✅ **通过 (PASS)** — 实测 p95 严格小于 100ms，达成 MCD 工业级性能指标。

