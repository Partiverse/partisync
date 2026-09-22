title: usearch HNSW 实战坑位
filename: usearch_hnsw_pitfalls.md
tags: [usearch, hnsw, vector-search, rust, retrieval]
updated_ns: 1704412800000000000

# usearch HNSW 实战坑位

## 真实 API（usearch 2.26）

不是训练出来的「自然」API，是看 README 后必须读源码才清楚的 API。

```rust
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

let opts = IndexOptions {
    dimensions: 768,
    metric: MetricKind::Cos,    // 注意不是 Cosine
    quantization: ScalarKind::F32,
};
let index = Index::new(&opts)?;
index.reserve(N)?;              // 必调！add 前 reserve
index.add(key, &vector)?;       // key 是 u64 不是 String
```

## 关键坑位

### 1. key 是 u64，不是 String

需要把 content_id 哈希到 u64：
```rust
let hash = blake3::hash(content_id.as_bytes());
let key = u64::from_le_bytes(hash.as_bytes()[0..8].try_into().unwrap());
```

### 2. 必须 reserve

不加 reserve：抛 `"Reserve capacity ahead of insertions!"`

### 3. MetricKind 名

- `Cos`（不是 `Cosine`）
- `L2sq`（L2 平方）
- `Haversine`

### 4. 默认量化是 BF16

精度损失——显式指定 `ScalarKind::F32`

### 5. Index::new 返回 Result

不是 `Index::default()`。

## HNSW 参数

```rust
let opts = IndexOptions {
    dimensions: 768,
    metric: MetricKind::Cos,
    quantization: ScalarKind::F32,
    connectivity: 16,           // 默认 16
    expansion_add: 128,         // 默认 128
    expansion_search: 64,       // 默认 64
};
```

## 性能 vs 召回

- `expansion_search` 越大 → 召回↑ 但 QPS↓
- `connectivity` 越大 → 索引大 但召回↑
- 推荐：recall@10 >0.95 + QPS >5k

## 持久化

```rust
index.save("/path/to/file.usearch")?;
let index = Index::load("/path/to/file.usearch")?;
```

## PartiSync 用法

- `~/.partisync/vectors.usearch`
- content_id → u64 映射表在 tantivy 索引附 metadata
- 向量维度动态（从文件头读取）

详见 `docs/specs/M4-WP02.md`。