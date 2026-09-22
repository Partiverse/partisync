title: 大模型评估方法论
filename: llm_evaluation_methodology.md
tags: [llm, ai, evaluation, benchmark, mt-bench]
updated_ns: 1721116800000000000

# 大模型评估方法论

## 基准选择

| 任务 | 基准 | 规模 |
|---|---|---|
| 中文理解 | C-Eval | 14k 题 |
| 代码 | HumanEval / MBPP | 164 / 974 题 |
| 多轮对话 | MT-Bench | 80 题 × 8 类 |
| 长上下文 | LongBench | 21 任务 |
| 推理 | GSM8K / MATH | 8.5k / 12.5k 题 |

## 评估流程

1. 离线批量推理（vLLM / TGI serving）
2. 自动评分（GPT-4 judge 或规则评分）
3. 人工抽样校验（10% 子集）
4. 出报告：模型 × 任务 × 数值

## 关键陷阱

- **数据污染**：训练数据与评测集重叠会虚高
- **位置偏差**：多选题选项顺序影响
- **长度偏差**：长答案不一定更好
- **GPT-4 judge 偏置**：对自己的输出打分偏高

## PartiSync MCP 评估

- 黄金查询集：30-50 个用户真实意图（asset_search）
- ground truth：人工标注 + 双人盲标
- 评分：nDCG@10 / MRR / Recall@10

详见 `docs/specs/M4-WP06.md`。