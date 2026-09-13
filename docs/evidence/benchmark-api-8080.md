# 8080 产品链路延迟基准（B-02，2026-09-13 12:32:51）

- 链路：client → partisync-server :8080 `GET /api/v1/assets`（q=asset_, limit=20, 随机 offset 前 10 页）→ Meilisearch → JSON
- 负载：50 并发 × 5000 请求；命中 999982 条基准语料
- 口径：**指定并发饱和下的客户端观测延迟**（含 API 序列化与网络），非单请求检索延迟（B-01 修正）

```
requests=5000
codes: {'200': 5000}
p50=20.66ms p90=34.00ms p95=40.13ms p99=54.82ms
success_rate=100.00%
```

- 单请求低负载参照（20 次 min/avg/max，见运行日志）：MCD 目标 p95 < 100ms 在**两种口径下**均需满足才算完整。
