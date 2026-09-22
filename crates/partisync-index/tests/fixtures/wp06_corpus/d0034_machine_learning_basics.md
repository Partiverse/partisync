title: 机器学习基础
filename: machine_learning_basics.md
tags: [machine-learning, deep-learning, neural-networks, python, ai]
updated_ns: 1704067200000000000

# 机器学习基础

## 监督学习

### 线性模型

```python
from sklearn.linear_model import LogisticRegression
model = LogisticRegression()
model.fit(X_train, y_train)
pred = model.predict(X_test)
```

### 树模型

- 决策树
- 随机森林
- XGBoost / LightGBM（梯度提升）

### SVM

- 线性 / RBF kernel
- 大数据集难扩展

## 无监督学习

- K-Means
- DBSCAN
- PCA / t-SNE / UMAP

## 深度学习

### CNN（图像）

- 卷积层
- 池化层
- 经典架构：ResNet / EfficientNet

### RNN（序列）

- LSTM / GRU
- 已被 Transformer 取代

### Transformer（序列/语言）

- Self-attention
- Multi-head
- 经典：BERT / GPT

## 训练流程

1. 数据准备（清洗/划分/增强）
2. 模型定义
3. 损失函数（CrossEntropy / MSE）
4. 优化器（SGD / Adam）
5. 训练循环（forward/backward/update）
6. 验证（early stopping）
7. 测试

## 评估

- 分类：accuracy / precision / recall / F1 / AUC
- 回归：MSE / MAE / R²
- 排序：nDCG / MRR / Recall@K

## 过拟合

- 数据：更多数据 / 数据增强
- 模型：正则化（L1/L2/Dropout）
- 训练：early stopping / 交叉验证

## 工程

- 实验管理：MLflow / W&B
- 超参搜索：Optuna / Ray Tune
- 部署：TorchServe / Triton Inference Server

## 框架

- PyTorch（研究主流）
- TensorFlow / Keras（生产）
- JAX（高性能研究）
- HuggingFace Transformers（预训练模型）

## 与 PartiSync

- BGE-M3 embedding 模型（dense / sparse）
- CLIP 图像 embedding
- reranker 模型
- 详见 M4-WP01/WP02 SPEC