# M0 里程碑报告（累计：WP00 + WP01 + WP02）

生成：`cargo xtask report M0` · 日期：2026-09-18 · 范围说明：本报告累计覆盖 M0 前三批
（质量基建 / 核心类型 / 元数据存储与演示面）；WP04–WP07（watcher/作业系统/CLI 矩阵/达标）进行中。

## 1. 范围与结果

| 任务 | SPEC | 结果 |
|---|---|---|
| M0-WP00-T01 | SPEC 批准 | M0-WP00/WP01 两份规格 G0 落盘 |
| M0-WP00-T02 | D2 清偿 | criterion 0.8.2 基准 ×3 + `bench.sh save/check` + `m0` 基线（check 模式漂移 ±2% 实测） |
| M0-WP00-T03/T04 | D5 清偿 | cargo-mutants 27.1.0 机制 + 首轮抽检（见 §3）；fmt 残留修正 |
| M0-WP01-T01/T02 | HLC | 测试先行 8 红/4 绿 → 实现 12 绿；**P5 属性测试发现 SPEC v1.0 契约推理缺陷（recv 漏 wall 分支），SPEC v1.1 修订留痕** |
| M0-WP01-T03 | error/caps | 错误分类学（Severity/classify_io 表驱动）+ ProviderCaps（保守 Default + 降级策略纯函数）；core 全量变异 93.6% |

范围变更：无降级。Cargo.lock 入库（可复现构建前提，随 T03 提交）。

**追加批（M0-WP02，2026-09-18 第二次更新）**：

| 任务 | 结果 |
|---|---|
| M0-WP02-T01 | SPEC M0-WP02 + ADR-0002（六项依赖批次）+ L4 登记，G0 批准 |
| M0-WP02-T02 | schema v1（5 表 + 闭包表）、Store（9 方法）、最小索引器；**L4 模型对照属性测试通过**（随机树闭包子树 ≡ 朴素 DFS）；工具链 1.94.0（ADR-0003，sqlx 0.9 前提） |
| M0-WP02-T03 | xtask gen-fixture（LCG 确定性、可控重复率） |
| M0-WP02-T04 | CLI index/ui 子命令 + 网页演示面（五 JSON 端点 + 单页界面） |

**真实运行证据**：300 文件夹具（35% 重复率）索引 → 图谱 300 文件/60 目录/9.9MB，
唯一内容 264（**去重节省 1.1MB**，31 重复组）；五端点 curl 断言通过；
`ancestors_of` 语义修正（排除自身，depth>0）随测试发现落定。

## 2. KPI 与基准

- `m0` 基线已存（criterion target/criterion/，报告引用非 git 资产）：
  ulid/encode ≈ 67.7ns · ulid/parse ≈ 48.0ns · ulid/field ≈ 1.9ns；
- M0 整体 KPI（10⁶ 文件索引 <10min）属 WP07 范围，未到评审窗口。

## 3. 测试证据

- **测试先行红绿**：HLC 桩 8 红/4 绿（commit 可考）→ 实现全绿；全套件 42 测试 0 失败；
- **变异分数**：core 92 变异体，73 caught / 5 missed（4 等价 + 1 保守默认等价）→ **93.6%**，
  有效分数≈100%；证据与归因：[mutants-m0-wp00.md](bench/mutants-m0-wp00.md)；
- **属性测试**：P5（HLC 严格单调+跨设备全序，200 步混沌序列）通过；
- **流程有效性再证**：属性测试抓到 SPEC 契约错误（wall-only 分支）——
  「规格错误由测试发现」的完整 SOP 闭环实例，SPEC v1.1 修订留痕。

## 4. 安全

- unsafe 增量 0；新依赖 criterion（dev）经 workspace 集中声明；
- `NetworkUnreachableExtended` 幻影 API 被编译门禁当场拦截（AGENTS「查证再写」红线的机制有效性实证）。

## 5. ADR 与债务

- 本批无新 ADR（getrandom/baseline 均沿用既有决策）；
- 债务更新：**D2/D5 关闭**；D1（远端）、D3（testcontainers）、D4（发布管线）、D6（人工签核）保持。

## 6. AI 使用披露（自动统计）

- 挂接 M0-* 提交：8 / 任务 7 / AI 辅助 8/8 / 人工终审 0/8（待人工复核窗口）。

## 7. 抽查审计记录

本批未执行（抽样随 M0 整体 G3 评审一并做）。

## 8. 下一步

按执行方案 §6.1 继续：WP02 元数据存储（SQLite schema v1 + entry_closure，钉子清单项）→
WP03 CAS（fastcdc/blake3，P1–P4 属性测试）→ WP04 扫描 → WP05 作业系统 → WP06 CLI → WP07 达标。

## 放行签字（G3——待 M0 整体完成后）

- [ ] 架构负责人：
- [ ] 评审人：
