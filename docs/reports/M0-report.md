# M0 里程碑报告（终版：WP00–WP07 全量 · G3 评审材料）

生成：`cargo xtask report M0` + 人工汇总 · 日期：2026-09-18 · 仓库：44+ 提交全部挂 Task-ID

## 1. 范围与结果（逐工作包对照 SPEC）

| 工作包 | SPEC | 结果 | DoD |
|---|---|---|---|
| WP00 质量基建 | [M0-WP00](../specs/M0-WP00.md) | criterion 基准 + baseline 机制；cargo-mutants 机制 | ✅ D2/D5 清偿 |
| WP01 核心类型 | [M0-WP01](../specs/M0-WP01.md) | ULID（M-1 演练）+ HLC（P5）+ 错误分类学 + ProviderCaps | ✅ |
| WP02 元数据存储 | [M0-WP02](../specs/M0-WP02.md) | SQLite schema v1–v3 + Store（闭包表 L4）+ 索引器 + 网页演示面 | ✅ |
| WP03 CAS | [M0-WP03](../specs/M0-WP03.md) | fastcdc 分块（P1–P3）+ 内容寻址块库（P4）；checkpoint 场景 67% 块级节省 | ✅ |
| WP04 事件管线 | [M0-WP04](../specs/M0-WP04.md) | notify → scan_journal → 幂等应用（P8）；watch 命令 + UI 自动轮询 | ✅ |
| WP05 作业系统 | [M0-WP05](../specs/M0-WP05.md) | 持久作业 + checkpoint（批粒度）+ resume；L5 恢复等价性 | ✅ |
| WP06 CLI 矩阵 | [M0-WP06](../specs/M0-WP06.md) | ls/find/dedupe（百万库实测，薄封装） | ✅ |
| WP07 达标验收 | [M0-WP07](../specs/M0-WP07.md) | **10⁶ 文件索引 7m55.6s < 10min**；KPI 报告 + 例行变异门禁 | ✅ |

范围变更：MessagePack → 关系列（WP05 §4 偏离声明）；基准 WP04 自 M-1 顺延后于 WP00 清偿。
**头号 KPI 达成路径**：28m33s（基线）→ 21m28s（WAL+NORMAL）→ **7m55.6s（批处理）**，3.6×，
全部优化与代价留痕（[m0-kpi.md](bench/m0-kpi.md)）。

## 2. KPI 达标表（对照调研方案 §7 M0 列）

| KPI | 目标 | 实测 | 证据 |
|---|---|---|---|
| 10⁶ 文件索引 | <10 min（Apple Silicon） | **7m55.6s** | [m0-kpi.md](bench/m0-kpi.md) |
| 去重报告可见 | 实时 | 文件级 4.42GB（13.3%）+ 块级 | 同上 + UI 卡片 |
| find 延迟 | 毫秒级 | 0.88s 含进程启动（百万库） | §6 实测 |
| 同卷去重报告 | 100% 数学正确 | L4/P4 + stats 模型测试 | 全仓 67 测试 |

## 3. 测试证据

- **全仓 67 测试 0 失败**；属性测试：P1–P5 + L1–L5 全落地
  （CDC 确定性/内容定义性、CAS 引用计数、HLC 单调全序、闭包子树模型对照、**L5 恢复等价性**）；
- **变异分数**：core+cas 142 变异体 117 caught = **82.4%**（门槛 ≥60%；miss 全部归档：
  [mutants-m0-wp00.md](bench/mutants-m0-wp00.md)）；
- **端到端**：SIGINT 断点续扫（1000+2000=3000 无缺口）+ resume vs 全量四元组全等；
  watch 实时管线（fs 增改删 → UI 5s 内反映）；
- **红→绿纪律**：ULID（11红/2绿）、HLC（8红/4绿）、journal（3红）——测试先行贯穿；
- 互操作/覆盖率接线随远端（D1/D3 债务）。

## 4. 安全

- unsafe 增量 **0 行**（workspace forbid）；cargo deny 配置就绪；
- 演示面仅绑 127.0.0.1（公网暴露需鉴权——M1 SPEC 强制项）；
- 持久性代价：WAL+NORMAL（OS 断电丢尾部提交）已评估留痕——索引可重建 + P8 重放兜底。

## 5. ADR 与债务

**ADR**：0000 引导 / 0001 getrandom / 0002 依赖批 / 0003 工具链 1.94 / 0004 fastcdc / 0005 notify。

**债务**：D1 远端与 CI 实跑 · D3 testcontainers · D4 发布管线 · D6 人工签核 ·
新增：D7 覆盖率工具接线（CI）· D8 索引器流式外排（内存峰值 1M 路径 ~100MB，M1）·
D9 fmt_bytes MiB 标注 · 块库孤儿（修改后旧块）归 M3 GC（既定口径）。

## 6. AI 使用披露（自动统计，`cargo xtask report M0`）

- 挂接 M0-* 提交 36+（本次累计 45+ 提交全链可追溯：`scripts/check-task-ids.sh` OK）；
- AI 辅助 100% / 人工终审 0%（**G3 放行前置：人工签核**）；
- 流程有效性实例：SPEC 契约错误（HLC wall 分支）由属性测试抓出；
  幻影 std API 被编译门禁拦截；变异测试抓出三类断言盲区；
  四个集成边界缺陷（符号链接路径/引用膨胀/幽灵句柄/孤儿内容）仅真实运行暴露。

## 7. 抽查审计记录

待 G3 评审时执行（抽样 5 任务重建）。

## 8. M0 结论与 M1 建议

- **结论：M0 全部工作包完成、全部 DoD 满足，KPI 达标。放行待人工签字（G3）。**
- M1 建议：① 首周清 D3/D4/D7（CI 补全：覆盖率/基准差值/夜间套件）；② Provider SPI
  （钉子项，双人评审）；③ serve s3 第一版（rclone 互操作认证为 M1 头号 DoD）。

## 放行签字（G3）

- [ ] 架构负责人：＿＿＿＿＿＿
- [ ] 评审人：＿＿＿＿＿＿
