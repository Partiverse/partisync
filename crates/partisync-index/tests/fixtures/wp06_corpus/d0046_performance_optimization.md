title: 性能优化方法论
filename: performance_optimization_methodology.md
tags: [performance, optimization, profiling, methodology, engineering]
updated_ns: 1715817600000000000

# 性能优化方法论

## 黄金法则

**测量优先**——不要凭直觉优化。

## 步骤

### 1. 设定目标

- p95 延迟 < 100ms
- 吞吐量 > 1000 req/s
- 内存 < 200MB

### 2. 测量基线

```bash
# 火焰图
cargo flamegraph --bin myapp

# pprof / perf
perf record -F 99 -g ./myapp
perf report

# criterion 基准
cargo bench
```

### 3. 找热点

- CPU 热点：火焰图
- 内存：heaptrack / valgrind
- IO：iostat / strace
- 锁：perf lock

### 4. 优化

**自上而下**：
- 算法（O(n²) → O(n log n)）
- 数据结构（哈希 vs 树）
- 缓存（避免重复计算）
- 并行（多核）
- IO（批量、异步）

### 5. 验证

- 再跑基准
- 对比 diff
- 没改善就回退

## 常见反模式

- ❌ 优化未测量的代码
- ❌ 牺牲可读性换性能
- ❌ 提前优化
- ❌ 单次测量就发报告

## PartiSync 经验

- tantivy TopDocs.limit(n) 不是 Collector——必须 .order_by_score()
- usearch 必须 reserve 前 capacity
- sqlx 0.9 动态 SQL 走 AssertSqlSafe
- 快路径避免锁（Arc<AtomicX>>）

## 工具

- **cargo flamegraph** —— 火焰图
- **criterion** —— 统计显著基准
- **pprof** —— 多语言 profile
- **heaptrack / valgrind** —— 内存
- **tokio-console** —— async 任务
- **strace / dtrace** —— 系统调用

## 报告

每次优化留档：
- 优化前数值
- 优化方法
- 优化后数值
- 副作用（如代码复杂度）
- 复现命令

## 文化

- 性能 = 可量化目标
- 优化 = 投资回报
- 测 = 诚实的艺术