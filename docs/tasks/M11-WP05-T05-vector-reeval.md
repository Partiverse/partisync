# Task: M11-WP05-T05 语义向量检索二期重触发立项评估(T1 触发后)

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP05-T05 |
| **类型** | docs-only(重触发立项评估件,只评估不实施) |
| **来源** | memory-vector-eval §6 触发纪律「T1–T3 任一满足即启动立项评估」+ T1 满足(叶量 118076 实测,M11-WP05-T04;用户裁决「T1 语料满足」已登记)+ M11-WP05-T03 准备态清偿清单(「BGESmallZHV15 冒烟重试」项)清偿完成(dim=512 实测) |
| **创建日期** | 2026-10-11 |
| **冒烟环境** | 本机单机实测(仓外 /tmp/bge-smoke,fastembed 7.0.1 生产同版本 pin,模型缓存 Xenova/bge-small-zh-v1.5,经 hf-mirror 下载腿固化、本次缓存命中离线推理)——单机实测非 CI |

## 交付

docs/reviews/M11-WP05-t05-vector-reinitiation-eval.md(新):T1–T4 逐项复核
(T1 满足——#226/#227/#228 证据链 + 用户裁决登记;T2 满足但自建语料降格
口径沿 t03;T3 冒烟清偿——BGESmallZHV15 dim=512 十句全 512 实测、逐句
延迟真中位 3.52ms(harness 打印 3.6ms 为上中位 lat[5],修正登记见评估
件 §2 注/§7)、模型文件 blake3 全套算出供 MANIFEST 填实;T4 窗口对齐
建议——实施位随 WP06 SPEC 批准后窗口)→ **实施立项放行(有条件)**,与
t03「暂缓」判定对照升级;前置条件 = 新立实施 SPEC 落锤维度契约(方案 B
512d)/memory 专享 key 域/降级语义/MANIFEST blake3 钉版 + 新 ADR +
R2 评审面。诚实边界:T2 降格、LCSTS 语料真实口径、冒烟单机非 CI、
存储/时长预估全部纸面标注。

## 涉及文件清单(Iron Rule 9)

docs/reviews/M11-WP05-t05-vector-reinitiation-eval.md(新)·
docs/tasks/M11-WP05-T05-vector-reeval.md(本卡)

## 验收

- [ ] T1–T4 逐项复核全部有证据锚点(仓内 path:line 或本件全文引录的实测实录)
- [ ] 判定与 t03「暂缓」对照升级理由明确(T1 硬条件已由 M11-WP05-T04 实测 + 用户裁决清偿),无伪造触发
- [ ] T3 冒烟为本会话实际运行实录(dim=512;命令、输出、blake3 哈希全文引录于评估件)
- [ ] MANIFEST 候填哈希齐备(model.onnx/tokenizer.json/config.json/tokenizer_config.json/special_tokens_map.json 五文件 blake3)
- [ ] 诚实边界登记:T2 自建语料降格、LCSTS≠生产流量、冒烟单机非 CI、l2norm 未测、纸面预估逐项标注
- [ ] docs-only;零代码 diff;零新依赖;零既有文件修改
