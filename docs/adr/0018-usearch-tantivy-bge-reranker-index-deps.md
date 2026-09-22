# ADR-0018: M4-WP02 混合检索依赖批次：tantivy/usearch/bge-reranker

版本: 1.0 · 状态: **起草中（待 G0 批准）** ·
关联: M4-WP02（混合检索）、M4-WP01（Sidecar 管线，ADR-0017 先例）、
调研方案 §5.10（索引与检索引擎）
负责人: @lead · 起草日期: 2026-09-22

## 背景

M4-WP02 混合检索需要引入新的 Rust crate：

| 依赖 | 版本 | 用途 | 备注 |
|------|------|------|------|
| `tantivy` | 0.26 | BM25 全文索引 | 调研方案 §5.10 锚定 |
| `usearch` | 2.26 | HNSW 向量索引 | 调研方案 §5.10 锚定 |
| `fastembed` | =7.0.1 | bge-reranker 精排 | 与 ADR-0017 同版本锚定 |

本 ADR 是 ADR-0017（推理栈批次）的伴生，遵循相同原则：
feature 门控（`index-rerank`）默认关闭、清单钉版 + blake3 校验、CI 离线 fake 行为。

## 决策

### 1. 精确锁定版本（与调研方案 §8 一致）

```
tantivy  = "0.26"
usearch  = "2.26"
fastembed = "=7.0.1"   # 锚定，exact version
```

理由：调研方案 §5.10 已完成技术选型评估，版本钉死避免供应链升级风险。

### 2. feature 门控：reranker 默认关闭

```toml
# partisync-index/Cargo.toml
[features]
default = []
index-rerank = ["dep:fastembed"]   # bge-reranker cross-encoder
index-embed = ["dep:fastembed"]    # 向量生成（embedding stage，本 WP 不直接使用）
```

理由：
- bge-reranker cross-encoder 模型体积约 1GB，CI 环境不应背负下载
- 本地推理需 CPU/GPU 资源，feature 关时不引入运行时依赖
- 沿用 ADR-0017 先例（`ai-ocr`/`ai-embed`/`ai-transcribe` 同样默认关）

### 3. CI 离线行为

`cargo test -p partisync-index`：不带 `--features index-rerank`，测试套件使用 **fake 向量数据**（内存 vec，非真实模型输出）。

真实模型推理冒烟测试：**手动验收步骤**（不进入 CI 门禁）。

### 4. 模型清单钉版 + blake3 校验

reranker 模型文件（HuggingFace GGUF/SAFE Tensors）同样走 `ModelManager` 清单钉版，
与 WP01 `models.rs` 的 ADR-0017 机制完全一致。

### 5. 依赖隔离：不穿透 trait 边界

`tantivy`/`usearch`/`fastembed` 类型**仅在 `partisync-index` 内部可见**，
不对外暴露（trait 边界设计同 ADR-0015 iroh 裁定 1）。

### 6. 构建注意事项

- `usearch 2.26` 依赖 `numkong` SIMD 库，在 Apple Silicon macOS 上需 Clang 17+
  或设置 `NK_TARGET_NEON=0` 环境变量绕过（不降低性能，仅禁用可选 SIMD 路径）
- `tantivy 0.26` 为纯 Rust，无需特殊构建处理
- 如遇 `numkong` 编译错误，设置 `NUMKONG_DISABLE_SIMD=1` 或升级 Clang

## 影响范围

**新增依赖**：`tantivy`、`usearch`、`fastembed`（3 个 crates）

**受影响 crate**：`partisync-index`（新增）

**不受影响**：其他所有 crate（`index-rerank`/`index-embed` feature 默认关闭）

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| usearch SIMD 编译失败（Apple Clang 16） | 设置 `NUMKONG_DISABLE_SIMD=1`；CI 使用容器或 Linux x86 |
| fastembed 版本漂移 | exact version `=7.0.1` 锁定 |
| reranker 模型下载阻塞 CI | feature 默认关，CI 不下载模型 |

## 依赖清单

| crate | 版本 | feature | Manager |
|-------|------|---------|---------|
| tantivy | 0.26 | — | 无（纯索引库） |
| usearch | 2.26 | — | 无（纯索引库） |
| fastembed | =7.0.1 | `index-rerank` | `ModelManager`（与 ADR-0017 共用） |
