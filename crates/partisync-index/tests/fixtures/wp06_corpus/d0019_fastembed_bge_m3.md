title: fastembed BGE-M3 嵌入实践
filename: fastembed_bge_m3.md
tags: [fastembed, bge-m3, embedding, retrieval, semantic-search]
updated_ns: 1704412800000000000

# fastembed BGE-M3 嵌入实践

## BGE-M3 是什么

BAAI 通用嵌入模型：
- 多功能（dense / sparse / multi-vector）
- 多语言（100+ 语言）
- 多粒度（短句到 8k token 文档）

## 三种向量

| 类型 | 维度 | 用途 |
|---|---|---|
| dense | 1024 | 语义相似 |
| sparse | 词汇权重 | 精确词命中 |
| colbert | token-level | 细粒度排序 |

## fastembed 集成

```rust
use fastembed::{TextEmbedding, EmbeddingModel};

let model = TextEmbedding::try_new(
    Default::default(),
    EmbeddingModel::BGEM3,
)?;
let embeddings = model.embed(vec!["hello world"], None)?;
```

## 模型文件

- 路径：`~/.fastembed/`
- 体积：~2.3 GB（BGE-M3 dense）
- 首次运行自动下载（CI 须 fake 模型）

## CI 兼容

ADR-0018 定义 CI fake 模型路径，特征门控 `index-rerank` 默认关。

## 性能

- Apple M2：单文档 ~50ms
- NVIDIA A100：单文档 ~5ms
- 批量 32：吞吐提升 5x

## 坑位

- 模型首次下载必须联网
- `Embeddings` 类型是 `Vec<Vec<f32>>` 注意 inner/outer 顺序
- BGE-M3 输出要 L2 normalize（usearch 余弦等价内积）

## PartiSync 用法（M4-WP02）

- `stage=embed` 产出 BGE-M3 dense（768d 简化版）
- usearch HNSW 索引
- 检索时 query 端生成查询向量 → 走 hybrid_search

## 关联

详见 `docs/specs/M4-WP02.md` §3-4。