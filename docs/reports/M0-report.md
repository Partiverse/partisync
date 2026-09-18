# M0 里程碑报告（累计：WP00–WP05）

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

## 追加批二（M0-WP03，2026-09-18 第三次更新）

| 任务 | 结果 |
|---|---|
| M0-WP03-T01 | SPEC M0-WP03 + ADR-0004（fastcdc v2020，API 源码核验 + 8 倍数约束） |
| M0-WP03-T02 | CDC 分块器 + 内容寻址块库（引用计数 + 对象两级扇出）；P1–P4 属性测试落地 |
| M0-WP03-T03 | gen-fixture 大文件版本链（checkpoint 场景） |
| M0-WP03-T04 | 索引器分块集成（≥256KiB）+ schema v2（entry.chunk_root 防御性 ALTER）+ UI 块级卡片 |

**P2 性质的三轮校准（流程证据）**：插入扰动的边界性质经三轮修订——
「2×avg 内不变 → 8×avg 内不变 → **重同步存在且持续 + 翻转数 ≤64 有界**」。
根因：v2020 归一化跳过使候选点依赖搜索起点，重同步半径经验 11×avg+，且可引入额外边界。
最终表述与去重的实际可行性严格对应（重同步后块内容逐字节相同 ⇒ 只有扰动邻域重传）。
P3 末块余数豁免与 ±50% 均值带宽同步校准（SPEC 风险节登记）。

**checkpoint 场景端到端证据**：12 个 4MB 版本（2 基底 × 6 版本，尾部 5% 变异）
→ **43 个块引用仅对应 18 个唯一块：块级节省 33.7MB / 引用 50MB（~67%）**；
磁盘对象实际占用 16MB；索引全程 7.5s。文件级去重（0.9MB）与块级去重（33.7MB）
的量级对比在演示面同屏可见——这正是对 rclone 整文件重传的核心差异化证明。

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

## 追加批三（M0-WP04，2026-09-18 第四次更新）

| 任务 | 结果 |
|---|---|
| M0-WP04-T01 | SPEC + ADR-0005（notify 8.2 事件源；去抖合并自写，理由：合并语义与 journal 落盘点强耦合） |
| M0-WP04-T02 | schema v3（scan_journal 队列）+ 幂等应用 + upsert（保留 id/位置、刷新可变字段）+ 级联删除；P8 测试：生命周期/重放幂等/崩溃重放收敛/级联清理 |
| M0-WP04-T03 | watch 循环（notify→去抖合并→journal→应用）+ CLI `partisync watch` + UI 5s 自动轮询 |
| M0-WP04-T04 | 四个实测踩坑的修复（见下） |

**实测踩坑与修复（真实运行暴露，测试未能预见的类别）**：
1. **符号链接路径**：macOS `/tmp`→`/private/tmp`，FSEvents 上报规范路径，
   `to_vpath` 前缀剥离静默丢事件 → watch 根先 canonicalize；
2. **引用计数膨胀**：幂等重索引对未变文件重复 put → refs 43→86 →
   内容未变跳过分块重入库；
3. **幽灵句柄**：UI 持有被删除的 index.db 的打开句柄，报旧数据 → 运维约束 +
   「db/cas 必须成对重置」（SPEC M0-WP03 风险节留痕）；
4. **节省为负**：upsert 更新内容后旧 content 行残留 → 孤儿清理（绝对值断言为击杀点）。

**端到端证据（干净重建后）**：213 文件/56.1MB 基线；实时增/改/删三连 → watch 日志
`+1 / ~1 / -1` 逐一应用，UI 5s 内自动反映；文件级节省 898KB（正）、块级节省 33.3MB
（44→48 refs，未变文件零膨胀）。

**债务与下一步**：M0-WP05（作业系统持久化）未开始，顺延为下批首项；块引用在
「修改后旧块」上仍有孤儿（M3 GC 既定口径）；rename 以 removed+created 对处理。

## 追加批四（M0-WP05，2026-09-18 第五次更新）

| 任务 | 结果 |
|---|---|
| M0-WP05-T01 | SPEC + L5 登记（含 MessagePack 偏离声明：v1 状态即关系列） |
| M0-WP05-T02 | jobs 表（schema v4）+ 作业 API + indexer 作业感知；**处理序缺陷在写测试前抓住**：walkdir DFS 序 ≠ 字典序（`/a.txt < /a/b` 但 DFS 先访 `/a/`），checkpoint 划界会漏文件 → 改为全局字典序处理（流式归 WP07） |
| M0-WP05-T03 | CLI：index 作业化（Ctrl-C 优雅中断）、resume、jobs |
| M0-WP05-T04 | 真实 SIGINT 演示 + L5 生产对照（见下） |

**断点续扫端到端证据（3000 文件 / 98.7MB）**：
`kill -INT` 于 1000 文件处 → exit 130、status=interrupted、checkpoint=`/d2/d0/f3_0161.bin`
→ resume 跳过 1000、续处理 2000 → completed。
**L5 生产对照**：resume 结果 vs 独立全量 index——files/total_bytes/unique_contents/unique_bytes
四元组**全等**（3000 / 103,537,450 / 2,471 / 84,467,253）。

**流程备注**：clippy 1.94 新 lint（is_multiple_of）随工具链升级生效，属机械修正。
