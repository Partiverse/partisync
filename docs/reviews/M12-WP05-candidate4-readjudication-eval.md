# M12-WP05-T01 复裁评估件——候选 4（记忆域语义向量检索）判定清账

> Task-ID: M12-WP05-T01 · 日期: 2026-10-11 · 复裁对象:
> docs/reviews/M11-WP05-t05-vector-reinitiation-eval.md §3「实施立项：
> 放行（有条件）」· 上游: M12 charter WP05

## 1. 放行前置条件逐项核销（t05 §3 四项）

| # | t05 前置条件 | 核销证据 | 判定 |
|---|---|---|---|
| ① | 新立实施 SPEC 落锤四件（512d 契约/memory 专享 key 域/降级语义/MANIFEST blake3 钉版） | M11-WP06.md §2：512d（BGESmallZHV15 实测全 512，T06 证据件③）/ key = blake3(memory_id) 前 8B 独立实例（memory_vector.rs:375-388）/ 降级 = fail-open 回退 BM25+显式标记（§2.4，t05 注「张力由实施 SPEC §6-Q2 二选一显式覆盖」——已落锤 fail-open，非暴力扫）/ MANIFEST 五文件 blake3（models.rs:58-88，T06 证据件④真文件重验） | ✅ |
| ② | 新 ADR（记忆域语义检索面） | ADR-0035（#236：graph 独立通道/fail-open/异步伴生+崩溃回填） | ✅ |
| ③ | R2 评审面（memory_search 公共面） | mode 为可选参数增量（非 breaking）；对抗审查逐条处置落码 #240（阻断项「向量落盘恢复闭环」等，T05 卡 :90）+ P25 G1 登记先于测试（#236，properties.md:32） | ✅ |
| ④ | 依赖方向裁决（graph 不得反向依赖 index/ai） | graph 核心依赖仅 usearch（workspace pin，Cargo.toml:22）；fastembed = feature 门控可选（`graph-embed` optional，Cargo.toml:26/:32），嵌入腿在 partisd 壳层（`--memory-embed`）——领域层核心零能力层依赖 | ✅ |

## 2. T1–T4 终态

- **T1** 满足（118,076 叶 + 用户裁决 #228，登记在案）；
- **T2** 自建语料降格沿案（诚实边界维持，LCSTS≠生产流量）；
- **T3** 清偿（dim=512 冒烟 t05 §2 + WP06 真模型 e2e 双重实证）；
- **T4** 实施窗口 = WP06，**已发生**（#237 底座 → #238 接线 → #239/#240 面+审查 → #241 真模型 e2e）。t05 §1 T4 行原文：partiverse S4「≈2027-03」为**外部需求锚（demand-side anchor），非实施阻塞**。

## 3. 判定

**「有条件放行」→「条件全部核销，候选 4 主体清账（已实施）」**。
候选 4 触发热纪律（M10-WP04-memory-vector-eval §6）全程闭环：触发 →
t03 暂缓 → T1 清偿 → t05 有条件放行 → WP06 实施 → 本件清账。

**剩余诚实边界（观察项，非缺口，不阻塞任何判据）**：位级确定性未证
（P25 显式不承诺）；语义收益为 e2e 演示级实证、非全量召回率证明；
真实工作负载表现未证（t05 :198）——随 GA 窗口 `rc-feedback` 渠道收
集真实使用反馈，另窗口评估。

## 4. charter 勘误（随本件落档）

M12-roadmap-proposal.md §1-WP05 行「实施仍待 S4 窗口，本里程碑不实
施」系**笔误**——t05 原案 T4 实施位 =「随 WP06 SPEC 批准后窗口」且已
发生，S4 仅为外部需求锚。勘误为：「实施已于 M11-WP06 落地；本 WP 复
裁即清账判定，无实施项」。charter 修订记录 v1.1 随本 PR 登记。

## 5. M12-WP05 收官口径

本 WP 唯一任务 = T01（本件 + charter 勘误）；无实施项、无后续任务。
候选 4 线在 roadmap 上的状态由「条件触发」改记「已清账」。
