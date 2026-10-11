# Task: M12-WP03-T01 示例扩展签名腿方案对证 + WP03 SPEC 起草

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M12-WP03-T01 |
| **类型** | docs-only（评估件 + SPEC，T02 实施前置件） |
| **来源** | M12-WP00-T01 charter WP03-T01 + M11-report §5 债表行 4 + EXT-SIGNING 示例扩展签名窗口（M10 债） |
| **创建日期** | 2026-10-11 |

## 交付

①docs/reviews/M12-WP03-demo-ext-signing-eval.md（新）：锚定钥现场事实（ANCHOR_PUBKEYS 双钥表，CI 子钥已是锚定钥且仅存 secret）+ 四方案对照（A 主钥仪式/否、B 仅守卫/部分采纳、**C′ CI 子钥签名 minisig 随仓/推荐**、D 新锚钥/否）+ 红线核对（P21 零语义变化、ADR-0027 不变、钥不经手）；②docs/specs/M12-WP03.md（新）：T02 四步契约（签名工作流 workflow_dispatch + minisig 经 PR 入仓、脚本加拷贝与显式告警、e2e 三态守卫含 skip-with-reason、防漂移 minisign -V 校验）+ 六项可执行验收 + 非目标四项 + R1 分级。

## 涉及文件清单（Iron Rule 9）

docs/reviews/M12-WP03-demo-ext-signing-eval.md（新）· docs/specs/M12-WP03.md（新）· docs/tasks/M12-WP03-T01-eval-spec.md（本卡）

## 验收

- [x] 方案对证全部基于仓内现场事实（signature.rs:17-24 / release.yml:178-199 / EXT-SIGNING.md:48-57 / registry.rs:65-89，行号随文）；
- [x] 推荐方案零密钥面变更（CI 子钥已锚定，零 ANCHOR_PUBKEYS 触碰）——不触发 R2/ADR；
- [x] SPEC 验收六项全部可执行可归档，e2e 从「跳过假绿」升格「真装载链验证」路径明确；
- [x] docs-only；纯三文件新增；零代码 diff；
- [x] 提交挂 Task-ID `M12-WP03-T01`，CI 全绿后 squash 合入。
