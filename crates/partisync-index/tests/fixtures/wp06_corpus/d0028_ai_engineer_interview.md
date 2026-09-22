title: AI 工程师面试准备
filename: ai_engineer_interview_prep.md
tags: [interview, ai, machine-learning, career, prep]
updated_ns: 1726348800000000000

# AI 工程师面试准备

## 基础

### 数学

- 线性代数（矩阵分解、特征值）
- 概率统计（贝叶斯、MLE、MAP）
- 优化（SGD、Adam、二阶方法）
- 信息论（熵、KL 散度、互信息）

### ML 基础

- 监督 vs 无监督 vs 强化
- 过拟合 / 偏差-方差
- 正则化（L1/L2/Dropout）
- 评估指标（accuracy/precision/recall/F1/AUC）

### 深度学习

- 反向传播
- CNN / RNN / Transformer
- 优化器（SGD/Momentum/Adam）
- 归一化（BatchNorm/LayerNorm）

## LLM 专题

### 架构

- Transformer（attention、FFN、残差）
- GPT（decoder-only）
- BERT（encoder-only）
- 位置编码（sinusoidal / RoPE / ALiBi）

### 训练

- 预训练 + SFT + RLHF/DPO
- 数据配比
- 学习率调度（warmup + cosine）
- 分布式（DDP / ZeRO / FSDP）

### 推理

- KV cache
- Speculative decoding
- PagedAttention（vLLM）
- Quantization（INT8/INT4/GPTQ/AWQ）

### 应用

- RAG（向量检索 + prompt 拼接）
- Function calling
- Agent 框架
- Fine-tuning（LoRA / QLoRA / Full）

## 工程

- PyTorch / TensorFlow
- HuggingFace Transformers
- vLLM / TGI / SGLang
- 评估框架（lm-eval-harness）

## 系统设计

- 推荐系统
- 检索引擎
- 实时推理服务
- 大模型 serving

## 行为面试

- STAR 法则（情境/任务/行动/结果）
- 项目深度追问准备
- 与团队协作的失败 + 反思

## 准备节奏

- 前 4 周：基础复习 + LeetCode 100 题
- 中 4 周：LLM 专题 + 1 个 side project
- 后 2 周：mock interview + 公司文化

## 目标公司

- 字节豆包团队
- 阿里通义
- DeepSeek
- 月之暗面（Kimi）