title: Rust async runtime 对比
filename: rust_async_runtime_compare.md
tags: [rust, async, tokio, async-std, runtime]
updated_ns: 1704067200000000000

# Rust async runtime 对比

## 主要候选

| Runtime | 作者 | 维护活跃度 | 生态 |
|---|---|---|---|
| Tokio | Tokio team | 高 | 最广 |
| async-std | async-std team | 中 | 中 |
| smol | Yoshua Wuyts | 高 | 小 |
| glommio | Glommio team | 中 | 极小 |

## Tokio 优势

- 生态垄断（HuggingFace / hyper / tonic / sqlx）
- 多线程 + 工作窃取
- 工具链完善（tokio-console）
- 文档完善

## Tokio 劣势

- 体积较大
- 学习曲线陡（多线程模型）
- 「我们承担了过多责任」争议

## 选择

**99% 场景选 tokio**——生态 + 工具链碾压。

## PartiSync 现状

- 全栈 tokio 1.x
- `#[tokio::test]` 单测
- `tokio::spawn` 任务调度
- `tokio::sync::mpsc / RwLock / Mutex`

## 常见模式

### spawn 后台任务

```rust
let handle = tokio::spawn(async move {
    process_in_background().await
});
// 主流程继续
result = handle.await?;
```

### 超时

```rust
let result = tokio::time::timeout(
    Duration::from_secs(30),
    fetch_data()
).await??;
```

### 并发扇出

```rust
let results = futures::future::join_all(
    urls.iter().map(|u| fetch(u))
).await;
```

### 选择接收

```rust
tokio::select! {
    msg = rx1.recv() => { ... }
    msg = rx2.recv() => { ... }
    _ = tokio::time::sleep(Duration::from_secs(5)) => { ... }
}
```

## 调试

- `tokio-console` —— 实时任务视图
- `tracing` + `tracing-subscriber` + `tracing-bunyan-formatter`
- flamegraph（CPU 热点）

## 陷阱

- ❌ 在 async 上下文用 std::sync::Mutex
- ❌ 在 async 上下文阻塞 I/O
- ❌ 持有 Mutex 跨 await point
- ❌ `tokio::spawn` + Send bound 遗漏
- ✅ 用 `tokio::sync::Mutex`
- ✅ 用 `tokio::task::spawn_blocking` 包阻塞调用

## 经验

- 测试用 `tokio::time::pause()` 加速
- 超时务必设置（避免 hang）
- 取消语义：`CancellationToken`