# M7 里程碑报告

生成：`cargo xtask report M7`（骨架，人工部分见各节注释）
回填：2026-09-30（WP00–WP03 闭合；WP04 滚动增补中）

## 1. 范围与结果（对照 SPEC 汇总；范围变更记录）

| WP | 交付 | 核心验收 |
|---|---|---|
| **WP00** M7 总纲 | M7-WP00 总纲 + WP01/WP02 SPEC 正式化 + xtask trace squash 债清偿 + 并行治理工件 | 4 任务全绿（PR #25/#26/#31 等）；trace 多任务提取修复（squash 非末位失明） |
| **WP01** WASM 扩展运行时实施 | `partisync-ext-host`（Engine 单例 + 磁盘缓存 + 注权 manifest + linker 全名注入 + ext 工具注册表）+ gateway 接线 + 桌面壳扩展面板 + 示例扩展 | **六项基准全 PASS**（冷启动 p50 11.46 ms / RTT 28.83 µs / RSS 增量 10.8 MB / 默认拒 47 测试 / clippy 零警告 / deny+audit 零新增豁免）；CI 时长 +6.1%（rust-cache 吸收冷编译）；**真机点验 source:extension 闭环**；T01–T05 全闭合（PR #33–#42） |
| **WP02** AI 安全复核窗口 | T01 M2-D1 密码学面复核 + T02 OSCP/M4-D2 渗透复核（双报告） | 双 **Conditional Pass（AI 执行，不构成外部审计）**；M2 五项整改全 HEAD 有效；三域无新 P0/P1/P2；18 探针 HEAD 全绿（实为 24 探针）；M4→M7 攻击面增量映射缺口 0；延续义务双报告留痕挂资金回笼 |
| **WP03** SMB 桥接评估 + 企业特性（评估 WP） | T01 FUSE spike（独立 workspace）+ T02 Samba 桥接实测 + T03 企业特性议题 + T04 ADR-0026 草案 | fuser 0.18.0 线位成立（拒绝面真挂载 2/2 绿）；smbd **实测可 export FUSE 挂载点**；**ADR-0026 草案待用户拍板**（4 项前置条件未闭环） |
| **WP04** M8+ 前瞻计划提案 | 滚动增补（M7-WP03 增补判例），commit 50659b2 | 提案主体待批准回填 M7-WP00 §1 |

**范围变更记录**：无已批准 SPEC 被缩水。WP03 为评估 WP（不产出产品代码，产出决策建议）；WP04 为滚动增补（非 M7-WP00 原定 WP 图）。**执行顺序**：WP01 → WP02（WP00 §5 R1 预定的「WP01 关账后稳定点」）。

## 2. KPI 达标表（基准报告链接）

| 指标 | 基准 | 实测 | 报告 |
|---|---|---|---|
| 宿主冷启动（缓存命中） | <100 ms | p50 **11.46 ms** / p95 11.71 ms | [M7-WP01-bench](M7-WP01-bench.md) §1 |
| 宿主冷启动（缓存未命中） | <100 ms | 12.31 ms | 同上 |
| 单次 tool call RTT（10 KiB JSON） | <5 ms | p50 **28.83 µs** / p95 29.54 µs（n=1000） | 同上 |
| 单扩展 RSS 增量 | <30 MB | **≈10.8 MB**（超 spike ±50% 容差带，按 SPEC 走「超差须解释」路径） | 同上 §3 |
| CI 时长增量 | 量化对照 | **+6.1%**（pre 11m25s → post 12m07s，n=21/13；冷编译 14m53s 一次性） | 同上 §2 |
| FUSE 随机读（4 KiB） | 无基线（新增面） | p50 1.2 µs（页缓存）/ **p95 74.6 µs**（真实往返） | [M7-WP03-fuse-spike](../reviews/M7-WP03-fuse-spike.md) §3 |

## 3. 测试证据（覆盖率/属性测试/变异分数/模糊时长/混沌/互操作）

- **属性/不变量**：P13（P14 + P15 候选）登记入 [docs/tests/properties.md](../tests/properties.md)；
  P13/P14 由 ext-host 47 测试覆盖，P15 候选由 FUSE spike 拒绝面探针覆盖（真挂载实测）。
- **对抗审查（变异测试手法）**：WP01 T04-A 审查员用变异测试（绕 trait/篡改入参/错误 JSON 退化）验证断言强度——沿此惯例。
- **渗透**：pen_test **24/24 绿**（18 原始 + 6 整改回归，HEAD 重跑，T02 报告）。
- **闸门**：clippy `-D warnings` 零警告（1.94 pin）；deny 四项 ok；audit 7 vuln 全命中既有豁免（5×ADR-0020/0021 + 2×ADR-0025 修订 6）。
- **对抗审查记录**：WP01 各任务 R2 深度 PASS（3 轮 PR #33/#34/#35 审查 + P1 全处置）。

## 4. 安全（cargo audit / deny / unsafe 增量 / 外部审计）

- **cargo audit**（0.22.0，本地库 f23b7682=2026-09-29）：7 vuln 全命中已登记豁免，**零新增**。
- **cargo deny**：advisories/bans/licenses/sources 四项 ok，零新增豁免。
- **unsafe**：workspace `unsafe_code = "forbid"` 全程未放宽，M7 零 unsafe 增量。
- **Mimosa deep 扫描**（2026-09-30，HEAD 完整重扫）：scan `scan-2026-09-30T10-06-31.900Z-833816693aa7`，seal `sha256:a894b62f…`，**1 条 HIGH advisory**（xtask `git()` 污点链，cross-file static）——**人工核实为不可利用**：`Command::new+args` 不经 shell；3 个调用点参数全字面量；`hash` 来自 `git log %H`；命令行参数在 `match args.as_slice()` 分叉即终止。**覆盖仍 partial/inconclusive**（gaps：动态派发跨文件可达性不完整；threatModel 阶段观察项为 0）——**不构成完整安全审计结论**。产物：`~/.mimosa/security-scans/project-513f89be40964bf2db88da7e/scan-…/`。
- **M2 密码学审计复核**（T01）：五项整改全 HEAD 有效，KDF 冻结未回退；P3×3 债登记（F-1 已处置 cfg(test) 收窄，F-2/F-3 登记）。
- **外部审计/人工 vendor 签字**：**未清偿**（见 §5 债务 D-S1）——AI 复核不满足执行方案 §7.1.10 的人工签字要求。

## 5. ADR 清单与债务登记

**M7 新增 ADR**：0025 修订 6（wasmtime 公告豁免，T04 第 0 步）· 0026（FUSE 挂载面，**草案未接受**，待用户拍板 + 4 项前置）。

| # | 债务 | 状态 | 窗口 |
|---|---|---|---|
| D-S1 | 外部密码学审计 + OSCP 持证人 0.5 天复核（人工 vendor 签字） | **未清偿** | 资金回笼事件（WP02 双报告 §延续义务留痕；M7-WP00 §5 R4 每次关账复核） |
| D-S2 | scanner_enobufs 覆盖边界（Mimosa 动态派发不可达） | 披露性挂账 | 工具侧能力边界，非项目可修 |
| D-S3 | ADR-0026 决策前置（鉴权映射/冷缓存基准/macOS 复测/写面范围/桥接取舍） | 阻断 | 实施 WP 立项前 |
| D-S4 | 扩展来源侧供应链面（签名/registry 无） | 登记 | M8+ 候选（T03 报告，威胁模型待拍板） |
| D-S5 | guest epoch/fuel 正解（WP01 SPEC R8 后半） | 登记 | 后续 WP |
| D-S6 | endpoint_secret 与 iroh 传输未合流（F-2 登记） | 观察 | 设备通道认证握手立项时 |
| D-S7 | F-1/F-3 密码学小债（F-1 已处置，余 F-3 dalek 种子清零边缘） | 观察 | 无动作 |
| D-S8 | O-1 gateway tools/list 频率限制 | 条件触发 | 网络暴露面出现时 |

**范围顺延（非裁剪）**：M6 顺延的「SMB 桥接评估 + 企业特性」由 WP03 承接（评估完成，实施待 ADR-0026 拍板）；reranker（ADR-0023 P2）维持挂起（BM25 已饱和）。


## 6. AI 使用披露（自动统计）
- 挂接 M7-* 任务的提交数：38
- 任务数：17
- 工作包分布：M7-WP00, M7-WP01, M7-WP02, M7-WP03, M7-WP04
- AI 辅助提交（AI-Assist）：38/38
- 人工终审提交（Reviewed-By）：1/38；**未带 37 处随本 G3 签字一并补认**
  （用户指令「Reviewed-By 全量补认确认」，2026-09-30；先例 M6-WP99-T01 /
  M2/M4 §6——补认以本签字表为凭，不逐 commit 改写历史）
  - 补认范围：统计窗口内全部 M7-* commit（`b0dd43c…76f0094` 前的
    squash merge 主线 + G3 落档 commit 除外）；其中代码面 PR 均经
    CI 8/8 + 对抗审查（WP01 三轮 R2 + 变异测试手法），docs 面 PR 均
    经 CI + SPEC 验收对照，符合 R0/R1 抽审与全审口径
  - M7-WP00-T01: 3 commit(s)
  - M7-WP00-T02: 1 commit(s)
  - M7-WP00-T03: 1 commit(s)
  - M7-WP01-T01: 1 commit(s)
  - M7-WP01-T02: 2 commit(s)
  - M7-WP01-T03: 2 commit(s)
  - M7-WP01-T04: 15 commit(s)
  - M7-WP01-T05: 1 commit(s)
  - M7-WP02-T00: 1 commit(s)
  - M7-WP02-T01: 2 commit(s)
  - M7-WP02-T02: 1 commit(s)
  - M7-WP03-T00: 1 commit(s)
  - M7-WP03-T01: 1 commit(s)
  - M7-WP03-T02: 2 commit(s)
  - M7-WP03-T03: 2 commit(s)
  - M7-WP03-T04: 2 commit(s)
  - M7-WP04-T01: 1 commit(s)

## 7. 抽查审计记录（随机 5 任务，仅凭工件重建故事）
<!-- 审计演练记录见 docs/reports/M-1-audit-rehearsal.md -->

| 任务 | 可重建的关键链 |
|---|---|
| M7-WP01-T02 | manifest 三阶段校验 → linker 全名注入（f26eebc+e3b73dc 缺陷修正：扁平名注册永不可达）→ 30 测试 |
| M7-WP01-T04 | 六层修复链（#36–#43）：Tauri 接线 → withGlobalTauri → IPC args 绑定 → sidecar handshake+_meta → inline onclick(CSP) → 首启建库；根因=测试只验函数不经 invoke/CSP 层 |
| M7-WP02-T01 | M2 五项整改逐项 file:line 证据 + sync 13/13·wp04 6/6·wp07 9/9 |
| M7-WP03-T01 | fuser API 查证表 → 语义实现 → 容器真挂载 2/2 绿（SE M7-WP03 §2/§3） |
| M7-WP03-T02 | smbd export FUSE 实测（协议层通）→ ACCESS_DENIED 约束捕获 → 根因假设 + 复测触发条件（诚实登记） |

## 8. 下一阶段建议

1. **用户拍板三件**：① ADR-0026（fuser 进产品图？4 项前置）② M8+ 前瞻计划提案（WP04）③ 扩展签名威胁模型（T03 草案转 SPEC）；
2. **M7 关账（WP99）**：~~Reviewed-By 补认（当前 1/38）、G3 三签~~ ✅ 已完成（2026-09-30 用户确认，见 §6/§放行签字）；
3. **债务窗口**：D-S1 外部审计（资金回笼）；D-S3 前置闭环后开 FUSE 实施 WP 一期。


## 放行签字（G3）

- [x] 架构负责人：@lead（2026-09-30 用户指令「M7 G3 关账三签确认」，沿 M6-WP99-T01 判例落档）
- [x] 评审人：@lead（同上）
- [x] 安全负责人（M2/M4）：@lead（同上；附条件：延续义务 D-S1 外部审计 + OSCP 人工复核挂资金回笼，WP02 双报告 §延续义务节 + 本报告 §5 留痕）
