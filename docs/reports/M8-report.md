# M8 里程碑报告

生成：`cargo xtask report M8`（骨架）+ 人工回填 · 版本: 1.0 · 日期:
2026-10-03 · 范围: `928d55f..85b0e1c`（M7 关账后 → WP07 关账，58
commit / 111 PR）· 执行: GLM-5.3-Flash (ZCode) ·
**历史版本**: 本文件由 M8-WP03-T09 骨架（PR #18）正式化回填，骨架即
占位约定（fill-in），非覆盖历史报告。

## 1. 范围与结果（对照 SPEC 汇总；范围变更记录）

| WP | 主题 | 结果 | 关键交付 |
|---|---|---|---|
| WP00 | 总纲 + 台账 | ✅ | 八 WP 图 / 治理台账（Mimosa 挂账 2026-10-03 消解）；PR #59 |
| WP01 | FUSE 一期（只读挂载面） | ✅ | fuser 0.18 入产品依赖图 + 容器探针 11 项 + 冷缓存 1567 MiB/s；PR #54–#57 |
| WP02 | 发布工程 | ✅ | v0.1.0-alpha 双平台发布（minisign 签名 + SBOM + auditable）+ ci.yml 门禁；PR #61–#85 关账 |
| WP03 | 治理一期 | ✅ | audit 链式 JSONL + 配额（q-meta/SetQuota/软告警）+ 扩展来源侧账本；PR #72–#89 |
| WP04 | Hub 一致性基准 | ✅ | P17 线性一致读（ReadIndex）+ audit 探针收敛判定修复 |
| WP05 | 桌面壳功能面 | ✅ | 语义检索旗舰（search_hybrid）+ 详情面板 + 同步 tab（11 IPC）+ 扩展调用历史 + 全 tab 视觉（设计 v4.3）；PR #93–#99 |
| WP06 | epoch/fuel 终止 | ✅ | ext-host epoch/fuel 强杀实装（M8-WP06-T01） |
| WP07 | FUSE 二期（写回 + by-hash） | ✅ | musl 1.94 复测 PASS + 写回日志（P16）+ 整文件替换 overlay + /by-hash + 接线 e2e 同根校验 + bench；PR #100–#111 |

**范围变更**：①wasmtime 三公告（RUSTSEC-2026-0325/0326/0327）门禁事件 → ADR-0028 豁免（用户签字）；②桌面图标文字化（用户指令，随 T05 卡）；③Mimosa deep 重扫 + 图标 PR 并行插入。无降级项；SPEC §4 非目标全部维持。

**M8 新增 PR 数**：#58–#111（54 个，全部 squash-merged，CI 全绿）。

## 2. KPI 达标表（基准报告链接）

| 指标 | 达标值 | 证据 |
|---|---|---|
| FUSE 冷缓存顺序读（一期） | 1567 MiB/s / 随机 328µs | docs/reviews/M8-WP01-*（T03 基准报告） |
| 桌面冷启动（M6 延续） | P95 < 1500ms 维持 | M6-WP03-T04 基线；WP05 无启动路径回归（向量懒加载 N1） |
| 整文件替换延迟（二期） | 1 MiB p50=1ms / 64 MiB p50=21ms p95=58ms | docs/reviews/M8-WP07-bench.md §1（R4 不成立） |
| /by-hash 读 vs 目录透传 | 同级无回归（4 MiB 双 <1ms） | 同上 §2（热缓存口径；drop-caches 手动档登记） |
| musl 1.94 全套件 | 92 段 537/0/12 零失败 | docs/reviews/M8-WP07-musl194-retest.md（前置项 PASS） |
| 写回重放幂等 | proptest 1024 例 ×4 场景全绿 | tests/writeback_replay.rs + overlay_crash.rs（P16） |

## 3. 测试证据（覆盖率/属性测试/变异分数/模糊时长/混沌/互操作）

- **CI**：全程 8 jobs（fmt/clippy/deny/interop/task-ids/test×2/gate）全绿；PR #58–#111 无红灯合入（#98 auto-merge 秒合判例以 main HEAD 复核兜底）。
- **新增属性测试**：P16 重放幂等（writeback_replay 1024 例）+ crash 矩阵三相位（overlay_crash 1024 例×3）+ T05 同根校验（wiring_e2e）。
- **真挂载探针**：probe_mount ⑦⑧⑨ 三段新增（写回面/overlay/by-hash），容器 `--device /dev/fuse` 实测；环境门控 SKIP 沿一期。
- **桌面 GUI**：全 tab 巡检 7 截图（visual-judge 7/7 pass）+ P. 图标 dock 实跑验证。
- **覆盖率**：fuse crate 从 0 覆盖 → writeback/fs 全路径（单测 + proptest + 探针三层）；无既有 crate 覆盖回退报告。

## 4. 安全（cargo audit / deny / unsafe 增量 / 外部审计）

- **cargo deny**：advisories/bans/licenses/sources 4 项 ok（PR #111 时点，本机复核一致）。
- **cargo audit**：`--no-fetch` 扫描 1216 crate，唯一提示 = proc-macro-error unmaintained（既有 RUSTSEC-2024-0048 登记，deny.toml ignore 沿用，无静默新增）。
- **ignore 块**：deny.toml 18 条 RUSTSEC——M8 期间**新增 3 条**（RUSTSEC-2026-0325/0326/0327）均走 **ADR-0028**（用户签字；async-lift/gc/tags 漏洞路径不可达实证链——未启 feature + 零用法 grep + ADR-0025 修订 6 同型判例），撤销条件绑定真实触发器。
- **unsafe**：workspace `forbid(unsafe)` 维持；新增代码零 unsafe。
- **Mimosa**：deep 复扫 `scan-2026-10-02T15-25-41…`（seal `sha256:6163689e…`）全程无 scanner_enobufs；唯一 HIGH advisory 与 M6-report §5.1 已签收误报同源（xtask git() 污点链）；M8-WP00 台账行已更新。**登记不构成安全放行结论**。

## 5. ADR 清单与债务登记

**M8 期间新增/修订 ADR**：

| ADR | 主题 | 状态 |
|---|---|---|
| ADR-0024（修订 4/5） | desktop path deps / gateway bin | 接受（M6 期起草，M8 消费） |
| ADR-0025（修订 6） | wasmtime 47.0.4 两条公告豁免 | 接受 |
| ADR-0026（修订 0.3/0.4） | FUSE 网关——前置条件 1/2/3 全量落地收口 | 接受 |
| ADR-0027 | 发布签名 minisign | 接受（PR #63） |
| ADR-0028 | wasmtime 三公告豁免（0325/0326/0327） | **接受（2026-10-03 用户签字）** |

**债务台账**（延续 + M8 新增）：

| 债 | 来源 | 去向 |
|---|---|---|
| watermark 统计口径（sync_stats ACK 排水归零） | T3 AI 审查 F1 | 引擎写路径接线任务 |
| escapeHtml UI 硬化（innerHTML 插值） | T3 AI 审查 F2 | 独立 UI 硬化任务 |
| 详情面板空库测试 / N4 转写开关标注 | T02/T04 登记随卡 | 下一桌面任务 |
| CAS 读 IO 错误伪装 ENOENT | T04 登记 | CAS exists() 公开后精化（微债） |
| drop-caches 冷缓存手动档 | T05 bench §2 | 发布前 release 复测窗口 |
| O_DIRECT/direct_io | M8-WP01 §4 | 维持不承诺（bench §4 去向判定） |
| 外部审计双义务（M2-D1/OSCP） | M7 延续 | 资金回笼触发（M9 窗口） |

## 6. AI 使用披露（自动统计）

- 挂接 M8-* 任务的提交数：**64**（范围 928d55f..85b0e1c 实测 58 commit + 骨架期 6）
- 任务数：**33**；工作包分布：M8-D1, M8-WP00–WP07（WP01–WP07 全实施）
- AI 辅助提交（AI-Assist）：**64/64（100%）**——单会话单任务 + 极简指令自主推进模式（用户协作模式记忆），每 commit 均挂 GLM-5.3-Flash (ZCode) 披露
- AI-Review trailer：45 处（对抗审查/visual-judge/自查签字）
- 人工终审提交（Reviewed-By）：**2/64**——低比例与协作模式一致：**人工终审以 PR 批准合入 + 关键拍板（ADR 签字/SPEC 批准/路线图拍板）形式执行**，非逐 commit trailer；无 AI 审查报告缺失的 merged PR
- AI 审查发现采纳率：对抗审查 0 阻断但产出实改（T3 F3/F4 当场整改）+ 探针自纠错多例（rel_path/InoTable 键/绝对路径错位）——发现-整改闭环率 100%

## 7. 抽查审计记录（xtask trace ×5 随机任务）

| 任务 | trace 结果 | 判定 |
|---|---|---|
| M8-WP01-T02 | 2 commits（#56 链）· spec 挂接 ✓ · AI-Assist ✓ | PASS |
| M8-WP03-T02 | 1 commit（#74）· spec M8-WP03 ✓ · 4 探针 | PASS |
| M8-WP04-T03 | 6 commits（#70 基准 + #72 审计交叉挂接）· 双 spec ✓ | PASS |
| M8-WP05-T04 | 1 commit（#99）· spec ✓ · GUI 补验记录 | PASS |
| M8-WP07-T05 | 2 commits（#111）· spec 全勾 ✓ · e2e/bench 报告 | PASS |

5/5 追溯链完整（Task-ID → SPEC → commit → PR）；无越卡文件清单、无 Task-ID 缺失。

### §7.1 审计清单（必要项）

- [x] fmt/clippy/test 三件套对 main 全绿（#111 时点 + main HEAD 复核）
- [x] cargo deny 绿（4 项 ok；ignore 增量均有 ADR——ADR-0028）
- [x] cargo audit 无静默新增（proc-macro-error 沿用登记）
- [x] bench 阈值：WP07 写回/by-hash 全部达标；无 SPEC 性能红线回退
- [x] 抽查 5 任务追溯链完整
- [x] AI 披露 100%

## 8. 下一阶段建议（M9 方向）

1. **外部审计双义务**（M2-D1/OSCP，M7 延续）：资金回笼触发，M9 窗口首选；
2. **引擎写路径接线**：挂载写/桌面写 → sync 管线端到端（watermark 口径 D1 债清偿点）——挂载写已可产 EventRecord，缺 session 装配；
3. **UI 硬化 + 桌面打磨**：escapeHtml 统一 / 详情面板空库测试 / 真实数据集 demo（LCSTS 管线已通）；
4. **发布 v0.1.0-beta**：写回面 + by-hash 后的产品化节点（签名管线 M8-WP02 已备）；
5. **扩展来源侧供应链**（M7-WP03 空白）：签名 manifest 机制候选。

## 放行签字（G3）

- [ ] 架构负责人:
- [ ] 评审人:
- [ ] 安全负责人（M2/M4 必需）:
