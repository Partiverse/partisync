# M-1 里程碑报告

生成：`cargo xtask report M-1` · 阶段：工程基建与审计演练 · 日期：2026-09-18

## 1. 范围与结果（对照 SPEC 汇总；范围变更记录）

| 工作包 | SPEC | 结果 | DoD 核对 |
|---|---|---|---|
| WP01 仓库骨架 | [M-1-WP01.md](../specs/M-1-WP01.md) | 12 产品 crate + 工具链钉版 1.93.1 + forbid(unsafe) + deny 配置 | 全部验收项满足（fmt/clippy/test 零警告零失败） |
| WP02 CI 门禁 | [M-1-WP02.md](../specs/M-1-WP02.md) | ci.yml 五 job（fmt/clippy/test×2 矩阵/deny/task-ids）+ 区间校验脚本 | 脚本本地验证通过；workflow 实跑待远端（债务 D1） |
| WP04 基准基建 | 执行方案 §6.0 | **降级出范围**（登记债务 D2）：演练以「测试即规格」为主，criterion 推迟至 M0 | 范围变更：经范围控制记录于 §5 债务 |
| WP05 追溯与治理 | [M-1-WP05.md](../specs/M-1-WP05.md) | xtask trace/report + commit-msg 钩子 + AGENTS.md + 模板四件套 + P-registry | 全部验收项满足（含别名缺失问题 I1 的发现与修复） |
| WP07 审计演练 | [M-1-WP07.md](../specs/M-1-WP07.md) | ULID 全循环：红(11/2)→绿(14/0)→AI 对抗审查（F1 真缺陷闭环）→演练报告 | 全部验收项满足；人工终审待签 |

范围变更：WP03（测试基建的 testcontainers 夹具）与 WP06（发布管线）依执行方案 §6.0 的
「本地等效先行」原则顺延至 M0 首周（CI 落地时同批补齐），已登记债务 D3/D4。

## 2. KPI 达标表（基准报告链接）

本阶段无产品性能 KPI（骨架期）。等效证据：

- 门禁全绿输出（本地，2026-09-18）：fmt ✓ / clippy `-D warnings` ✓ / 24 测试二进制全绿；
- `cargo tree -p partisync-core`：运行时依赖仅 getrandom（SPEC 验收「零实现依赖」满足）。

## 3. 测试证据

- **测试先行（红→绿）**：桩实现 11 failed / 2 passed（commit `1c3a70f`）→ 实现 14 passed / 0 failed；
- **属性测试**：L1 往返、L2 时间戳序、L3 字母表格式（properties.md 登记号），proptest 默认 256 案例全过；
- **审查报告**：[M-1-WP07-ai-review.md](../reviews/M-1-WP07-ai-review.md)——5 发现，F1 真缺陷修复+回归测试闭环；
- **CI run 链接**：暂无（无远端，D1）；本地门禁命令与输出已列入审计演练报告 §2。

## 4. 安全

- cargo audit/deny：deny 配置就绪，仓库尚无锁定依赖面（仅 getrandom/proptest 链），
  audit job 首跑随 CI（D1）；手动 `cargo deny check` 建议随 CI 验证；
- unsafe 增量：**0 行**（workspace forbid 生效，演练 diff 无 unsafe）；
- 外部审计：不适用（M-1 无密码学面；ULID 非机密值）。

## 5. ADR 清单与债务登记

**ADR**：[0000 引导与治理](../adr/0000-bootstrap-and-governance.md)（含许可证临时决策）、
[0001 getrandom 熵源](../adr/0001-getrandom-for-ulid.md)。

**债务登记**（转入 M0 或 CI 首跑处理）：

| ID | 内容 | 处理窗口 |
|---|---|---|
| D1 | workflow 真实远端未验证（本仓库无 remote） | 远端建立后首 PR |
| D2 | criterion 基准基建 + baseline.json 机制（WP04 降级） | M0 第 1 周 |
| D3 | testcontainers 测试夹具（rustfs/MinIO/WebDAV/FTP）（WP03 顺延） | M0 第 1–2 周 |
| D4 | 发布管线（签名/SBOM/changelog）（WP06 顺延） | M0 末（v0.1 发布前） |
| D5 | 变异测试（cargo-mutants）抽检机制 | M0 第 1 周（I2 教训升级） |
| D6 | README「许可证临时」标注 + ADR-0000 人工签核 | 人工复核时 |

## 6. AI 使用披露（xtask 自动统计）

- 挂接 M-1-* 任务的提交数：7
- 任务数：7（WP01×1、WP02×1、WP05×3、WP07×2，另有 T03 收尾提交在本报告生成后）
- 工作包分布：M-1-WP01, M-1-WP02, M-1-WP05, M-1-WP07
- AI 辅助提交（AI-Assist）：7/7
- 人工终审提交（Reviewed-By）：0/7 —— **真实反映本次为 AI 全程演练**，
  G3 放行前置条件即人工签核（见 §8）。

## 7. 抽查审计记录

演练即抽查：[M-1-audit-rehearsal.md](M-1-audit-rehearsal.md) §5 以「外部审计者五问」
重建了 M-1-WP07 的完整故事（为什么做/要求什么/做了什么/如何证明/谁做的），全部可答；
人工签核位 ⏳。

## 8. 下一阶段建议与放行条件

1. **人工复核三件**：① ADR-0000/0001 签核；② M-1-WP07 人工终审（重点复核 AI 审查 F1 修复面）；
   ③ 本报告 G3 签字。完成即 M-1 正式放行。
2. M0 首周优先补债务 D2/D5（基准与变异基建——I2 教训：没有变异分数，测试质量无量化护栏），
   再进入 WP01 核心类型开发。
3. 建立远端仓库并首推（D1 随即消除），branch protection 按 ci.yml 五 job 配置。
4. M0 启动会按 SOP S1/S2 完成 WP01–WP02 的 SPEC 与任务卡分解（模板已就绪）。

## 放行签字（G3）

- [ ] 架构负责人：（待人工签核）
- [ ] 评审人：（待人工签核）
- [ ] 安全负责人（M2/M4）：不适用（无安全面）
