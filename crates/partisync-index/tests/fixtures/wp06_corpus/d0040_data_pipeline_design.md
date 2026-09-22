title: 数据管道设计
filename: data_pipeline_design.md
tags: [data-engineering, pipeline, etl, streaming, design]
updated_ns: 1715817600000000000

# 数据管道设计

## 模式

### 批处理（Batch）

- Airflow / Dagster
- 适合：日报/周报
- 延迟：小时/天

### 流处理（Streaming）

- Kafka + Flink
- 适合：实时监控
- 延迟：秒级

### Lambda 架构

- 批 + 流 双写
- 复杂但容错强

### Kappa 架构

- 仅流（Kafka 日志）
- 简化运维

## 数据湖 vs 数据仓库

| 维度 | 数据湖 | 数据仓库 |
|---|---|---|
| 数据 | 原始（结构/非结构） | 清洗后结构化 |
| 存储 | 对象存储（S3/OSS） | 列存（Parquet） |
| 处理 | Spark / Trino | SQL 引擎 |
| 适用 | 探索/ML | BI 报表 |

## PartiSync Sidecar 管线（M4-WP01）

本质是轻量 ETL：
- Extract：从本地 fs 读文件
- Transform：缩略图 / EXIF / OCR / 转写 / C2PA
- Load：SQLite content + sidecar_items + BlobSink

## 关键决策

### 幂等性

每 stage 设计幂等（重跑不污染）。`ensure_enqueued` 用 `INSERT OR IGNORE`。

### 检查点（checkpoint）

作业失败可断点续走。`jobs.checkpoint` 列存当前 stage。

### 反压（backpressure）

管线无外部反压（资源本地）。但单侧（jobs）有内存保护（batch size 上限）。

### 失败处理

- 永久失败：标 failed + 原因
- 临时失败：retry with backoff
- 致命错误（DB 损坏）：fatal + 提示用户

## 监控

- 作业计数（done / skipped / failed）
- p95 耗时
- 单阶段失败率
- 资源占用（CPU/内存/IO）

## 调试

- 作业 ID 入所有 log
- 失败原因结构化（code + message）
- 重跑入口（CLI `partisync sidecar-run`）

## 与 PartiSync 集成

- WP04 iroh 数据面 → 跨设备增量同步
- WP03 MCP job_status → 暴露作业状态
- WP05 C2PA → 管线第六阶段