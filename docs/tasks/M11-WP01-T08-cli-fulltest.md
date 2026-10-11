# Task: M11-WP01-T08 GA 前 CLI 全量实测(8 域矩阵,J1-③ 执行件)

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M11-WP01-T08 |
| **类型** | 实测+docs(零产品代码 diff;全链 CLI/partisd/MCP 面实测,沿桌面端验收规则 2026-10-09 D19 修订:验证走 CLI/守护进程/MCP) |
| **来源** | M11-WP01-T07 GA 口径 §2 门槛③「窗口内 ≥1 次内部全链验收」;用户 2026-10-11 审批方案(全纳入 D/E/F + LCSTS 10⁴ 资产域规模面 + 通过即 J1-③ 闭合预授权) |
| **创建日期** | 2026-10-11 |
| **被测** | main@611e8f5(WP06 #241 后),release 构建,worktree 隔离(并行会话共用主树) |
| **语料** | LCSTS 资产域 12,000 文档 + memory 域 118,076 叶 + big.db 负控 |
| **判据精确定义** | docs/reviews/M11-WP01-ga-cadence.md §1 J1 行门槛③(0.1 补记 M12-WP02-T02:合入 commit trailer 的 Spec 字段所指 M10-WP06-release-cadence.md 为谱系母本,非判据条款所在——M11-report §5 债表行 8 清账) |

## 交付

docs/reviews/M11-WP01-t8-cli-fulltest-evidence.txt(新):8 域矩阵 **PASS 38 / DEGRADED-EXPLICIT 1
(CLI hybrid 无模型诚实回退) / FAIL 0**。要点:中断 resume(107591/120000 断点续扫)、规模
(12k reindex 0.52s/检索 3ms)、WP06 向量腿 e2e(写入→伴生嵌入→artifact 持久化→重启后语义
rank 1)、降级标记、GC 生命周期 P25(c) 用户面、mcp-http fail-closed 双层(JWKS 拒启 + 401
A-2 形状)、FUSE 真挂载(写回 unlink + by-hash chunk 粒度)、big 负控诚实上浮。限制新增
登记 5 项(CLI 库×FUSE 指纹缺口/by-hash 小文件 ENOENT/T04 语料库 device 缺口/loopback
无鉴权设计内/--help 微债)、既有清单更新 1 项(③语义向量检索已清偿)。

## 涉及文件清单(Iron Rule 9)

docs/reviews/M11-WP01-t8-cli-fulltest-evidence.txt(新)·
docs/tasks/M11-WP01-T08-cli-fulltest.md(本卡)

## 验收

- [x] 8 域矩阵逐项执行且证据归档(56 个命令日志 + 证据件十节)
- [x] 判据满足:全绿或偏差如实登记(1 项 DEGRADED-EXPLICIT,零 FAIL,零假绿)
- [x] 域 H 已知限制逐项复核并更新(③清偿 + 5 项新增登记)
- [x] J1-③ 闭合(用户 2026-10-11 预授权:通过即闭合)
- [x] 环境事件如实登记(共享 target 被并行 cargo clean 两项未复采;语料库 +1 叶合法变更)
- [x] 测试守护进程全清理;零产品代码 diff;零新依赖;提交挂 Task-ID
