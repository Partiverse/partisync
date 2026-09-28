# M6 里程碑报告

生成：`cargo xtask report M6`（骨架，人工部分见各节注释）

## 1. 范围与结果（对照 SPEC 汇总；范围变更记录）
<!-- 人工填写：逐工作包对照 SPEC 验收标准 -->

## 2. KPI 达标表（基准报告链接）
<!-- 基准基建（M-1-WP04）落地后自动填充；此前手工附 criterion 输出 -->

## 3. 测试证据（覆盖率/属性测试/变异分数/模糊时长/混沌/互操作）
<!-- 附 CI run 链接与本地验证输出 -->

## 4. 安全（cargo audit / deny / unsafe 增量 / 外部审计）
<!-- -->

## 5. ADR 清单与债务登记
<!-- -->

### 5.1 安全扫描 advisory 人工终审登记

- Mimosa deep 静态扫描 `scan-2026-09-28T10-20-22.082Z-055c6f969933`
  （seal `sha256:e1ee766c12d2…`，2026-09-28，产物于
  `~/.mimosa/security-scans/project-513f89be40964bf2db88da7e/`）：
  唯一 HIGH advisory 指向 `xtask/src/main.rs:338` `fn git()`——
  命令行参数流入 `Command::new("git")` 的静态污点链（启发式，
  proof gap 自述需人工确认）。
- **人工终审（用户签收，2026-09-28）：接受/误报**。理由：xtask 为
  dev-only 开发工具不入产品依赖图，全部调用点参数为硬编码字面量，
  无外部可控数据可达该 sink；不改代码。扫描 run status=inconclusive
  （调用图部分不完整），本登记不构成里程碑放行结论。

## 6. AI 使用披露（自动统计）
- 挂接 M6-* 任务的提交数：31
- 任务数：14
- 工作包分布：M6-D67, M6-D68, M6-WP01, M6-WP03
- AI 辅助提交（AI-Assist）：31/31
- 人工终审提交（Reviewed-By）：25/31
  - M6-D67-T01: 1 commit(s)
  - M6-D67-T03: 1 commit(s)
  - M6-D68-T01: 1 commit(s)
  - M6-WP01-T01: 1 commit(s)
  - M6-WP01-T02: 1 commit(s)
  - M6-WP03-T01: 3 commit(s)
  - M6-WP03-T02: 2 commit(s)
  - M6-WP03-T03: 5 commit(s)
  - M6-WP03-T04: 5 commit(s)
  - M6-WP03-T05: 4 commit(s)
  - M6-WP03-T06: 2 commit(s)
  - M6-WP03-T07: 1 commit(s)
  - M6-WP03-T08: 2 commit(s)
  - M6-WP03-T09: 2 commit(s)

## 7. 抽查审计记录（随机 5 任务，仅凭工件重建故事）
<!-- 审计演练记录见 docs/reports/M-1-audit-rehearsal.md -->

## 8. 下一阶段建议
<!-- -->

## 放行签字（G3）

- [ ] 架构负责人：
- [ ] 评审人：
- [ ] 安全负责人（M2/M4）：
