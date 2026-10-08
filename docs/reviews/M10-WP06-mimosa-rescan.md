# M10-WP06-T04 Mimosa 完整重扫登记（deep · 2026-10-08）

> Task-ID: M10-WP06-T04 · 日期: 2026-10-08 · SPEC: docs/specs/M10-WP06.md §2.3
> 执行: GLM-5.3-Flash (ZCode) · 边界纪律判例: docs/reports/M7-report.md §安全-Mimosa 行 / D-S2 ·
> 误报签收判例: docs/reports/M6-report.md §5.1 · anchor 台账判例: docs/specs/M8-WP00.md（Mimosa 行）
>
> **纪律行（先读）**：本档是对一轮机器静态扫描事实的**登记**，不是安全审计报告。
> 扫描 run status = **inconclusive**、evidence 边界 = **static_only_no_runtime_execution**
> （report 尾注原文见 §3）——**本次扫描不构成完整安全审计结论，本登记不构成安全放行
> 结论**（沿 M7-report / M8-report 判例纪律）。本档不含任何「项目安全」宣称。
> 外部审计双义务（M2-D1/OSCP + Mimosa 完整结论出具）**维持资金回笼触发不变，本次重扫
> 不替代**（SPEC §4）。

扫描发起：M10-WP06 SPEC 起草期（T01 同日，2026-10-08，用户既定 WP06 计划内动作），
目标 = 发布口径拍板前补 M10 增量面（远程 MCP 授权墙 / 记忆 tombstone+GC / GUI 检索
透出）的当期扫描事实登记（SPEC §1-④）。

## 1. 要素一：scanId（扫描身份）

| 项 | 值（工件实测） |
|---|---|
| scanId | `scan-2026-10-08T06-53-40.527Z-63a9f1cb4a97` |
| jobId | `scan-job-muz6k6sl-63ad197f7bcbc4f4`（status=completed，attempt 1，`security-scan-jobs/` 记录） |
| 发起/完成时刻 | 2026-10-08T06:53:35.973Z → 06:53:40.530Z（秒级完成，SPEC §6-R3 以 seal+coverage 工件为凭） |
| producer | mimosa 0.1.0 |
| target | repository · projectId `project:513f89be40964bf2db88da7e` · **depth deep** |
| 工件目录 | `~/.mimosa/security-scans/project-513f89be40964bf2db88da7e/scan-2026-10-08T06-53-40.527Z-63a9f1cb4a97/`（scan-manifest / findings / coverage / seal 四件 sealed JSON + report 投影） |

## 2. 要素二：seal（防篡改封印）

- **digest = `sha256:f9a368e3acb94fd213f8ad609564413c90337b21a2628c1658afd952c6867923`**
- 三工件逐项哈希（seal.json 实测）：scan-manifest.json
  `sha256:00e610d2…31c6d5b` · findings.json `sha256:0cdf4537…5070cc74` ·
  coverage.json `sha256:8819bf0d…4922fe3d`（全文哈希以仓外工件原件为准，
  本档登记截断值仅为可读性）。

## 3. 要素三：覆盖状态（inconclusive 边界如实登记）

`coverage.json` 实测：completeness=**partial** · runStatus=**inconclusive** ·
verdictEffect=**none**。

| 维度 | 实测值 | 说明 |
|---|---|---|
| 源文件 | 227 selected / **227 parsed** · readFailures=0 · parseFailures=0 · truncated=false | 全解析；**无 scanner_enobufs**（M7 D-S2 边界本轮未复现） |
| 路径分析 | 131 functions · 301 callEdges · traces=0 | 上轮（10-02）为 61/125，随 M10 增量代码面扩大 |
| threatModel | entryPoints=0 · principals=0 · authorizationSurfaces=0 | 工具级观察项为 0（与历次一致） |
| validation | investigated=0 · queries=0 | 静态发现未经自动验证 |
| 依赖扫描 | **1213 包** · 离线 advisory 匹配 **1 包 1 条**（unknown=0） | job 记录 dependencySummary 实测；**包级明细未列入工件**，本档如实登记计数不下断言——依赖 advisory 权威口径仍以 CI cargo deny/audit 门禁（allowed 名单维持）为准 |
| **gaps 原文** | 「调用图部分不完整：部分调用为动态派发或超出分析规模，跨文件可达性可能不完整」 | coverage.json `gaps[]` **逐字**登记；openQuestions=[] |
| 运行时边界 | report.md 尾注原文：「This report is a projection of sealed JSON artifacts. It is not runtime verification.」 | findings 证据态 = static-finding（evidenceState 实测），即 **static_only_no_runtime_execution** |

## 4. 要素四：唯一 HIGH advisory 同源对照与处置

**唯一 finding**（findings.json 实测，totals: high=1 / medium=0 / low=0 /
info=0 / businessLogic=0）：

| 项 | 值 |
|---|---|
| 标题/消息 | 「git 是 security 入口」——不可信数据「命令行参数」流入 git() → 污点链：git(sink:security) |
| 位置 | `xtask/src/main.rs:343`（实测 = `fn git(args: &[&str])` 定义行） |
| severity / kind | high · cross-file · advisory=true · evidenceState=static-finding |
| **anchor** | `sha256:efe56d0c859c37d0f40f27982b798084c1dfd0705435b5ab08714660e8825cea` |
| findingId / occurrenceId / instance | `finding:5435b5ab…` / `occurrence:f1e9d043…` / `sha256:183af347…` |
| proofGaps 原文 | 「静态 advisory 需要人工确认真实数据流和可利用性。」 |

### 4.1 同源对照（本会话四连实测：四次扫描 anchor 逐字相同）

| 扫描（本 project 目录） | 日期 | anchor 前缀 | 行号 | 处置登记 |
|---|---|---|---|---|
| `scan-2026-09-28T10-20-22…` | 2026-09-28 | `efe56d0c859c37d0` | :338 | **人工终审签收误报**（M6-report §5.1，用户 2026-09-28 签收「接受/误报」） |
| `scan-2026-09-30T20-48-58…` | 2026-10-01 | `efe56d0c859c37d0` | :343 | 引用 M6 §5.1 签收（M8-WP00 台账行） |
| `scan-2026-10-02T15-25-41…` | 2026-10-02 | `efe56d0c859c37d0` | :343 | 同上（M8-WP00 台账行「原样复现」） |
| **`scan-2026-10-08T06-53-40…`（本次）** | 2026-10-08 | `efe56d0c859c37d0` | :343 | 本档 §4.2 |

四轮 `grep -o '"anchor"'` 实测 96 位十六进制**逐字相同**（findingId/occurrenceId
亦同）= 同一发现，行号漂移 338→343（M6 签收时 :338）为代码移动所致，非新发现。

### 4.2 处置（维持既有，不改代码）

- **维持 M6-report §5.1 人工签收处置（误报/接受）**，签收理由照录：xtask 为
  dev-only 开发工具不入产品依赖图；调用点参数硬编码；无外部可控数据可达该 sink。
  本会话复核现状：调用点 **2 处**（`xtask/src/main.rs:353/:358`），参数全字面量
  + `hash` 溯源 `git log %H` 输出（M7-report 已人工核实 `Command::new+args`
  不经 shell、命令行参数在 `match args.as_slice()` 分叉即终止）。
- **SPEC §6-R4 对照纪律**：若出现**非同 anchor** 新发现 → 独立评审另立任务。
  本次扫描 **零新 anchor**（唯一 finding 即上述同源项），纪律未触发。
- 行号漂移不改处置（M8-WP00 台账行判例延续）。

## 5. 要素五：边界声明（不构成安全放行结论）

1. **本登记 ≠ 完整安全审计结论**：run status=inconclusive（调用图部分不完整，
   gaps 原文见 §3）+ 静态-only 无运行时执行（§3 末行原文）——沿 M7-report
   §Mimosa 行 / M8-report 判例纪律逐字声明。
2. **外部审计双义务维持**：M2-D1/OSCP 外部审计 + Mimosa 完整结论出具，条件
   触发 = 资金回笼（SPEC §4；M9-report §5 债表行），**本次重扫不替代、不闭合
   该行**——该行状态仍为 ⏳，仅补本次登记指针（M9-report 注记随本 PR）。
3. **本档无「项目安全」宣称**：全部内容为扫描器输出事实登记 + 既有处置引用，
   不含任何安全状态断言；SPEC §6-R3「不夸大为完整结论、不假绿」照办。

## 6. 扫描谱系（本项目六次扫描纵向对照）

| # | scanId（前缀） | 日期 | depth | 文件 parsed | HIGH | run status | 登记 |
|---|---|---|---|---|---|---|---|
| 1 | `…09-28T10-20-22…` | 2026-09-28 | deep | 160（spike commit 期 enobufs 债在身） | 1（同 anchor，:338） | inconclusive | M6-report §5.1（人工签收） |
| 2 | `…09-29T01-54-20…` | 2026-09-29 | deep | 170（1209 包，含 spike/WASM 面，enobufs 债清偿） | 1（同源，零新增） | inconclusive | M6-report §5.2 |
| 3 | `…09-30T10-06-31…` | 2026-09-30 | deep | 186（HEAD 完整重扫） | 1（人工核实不可利用） | inconclusive | M7-report §安全 |
| 4 | `…09-30T20-48-58…` | 2026-10-01 | deep | 191（hook 失同步 enobufs 债清偿） | 1（同 anchor，:343） | inconclusive | M8-WP00 台账行 |
| 5 | `…10-02T15-25-41…` | 2026-10-02 | deep | 203 | 1（同 anchor，原样复现） | inconclusive | M8-WP00 台账行 |
| 6 | **`…10-08T06-53-40…`** | **2026-10-08** | **deep** | **227**（0 失败） | **1（同 anchor，零新增）** | **inconclusive** | **本档（T04）** |

谱系结论（事实性，非安全宣称）：M10 增量面入扫后（203→227 文件）唯一 finding
仍为四次同 anchor 项，**零新增 advisory anchor**；scanner_enobufs 本轮零发生。

---
*工件原件（sealed JSON）在仓外 `~/.mimosa/security-scans/…`（路径 §1）；本档为
其事实登记投影，冲突时以 sealed 工件为准。*
