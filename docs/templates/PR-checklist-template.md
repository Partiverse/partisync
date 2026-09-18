# PR 检查单（随 PR 描述粘贴）

- [ ] 链接任务卡与 SPEC；提交 trailer 完整（Task-ID/AI-Assist/Reviewed-By）
- [ ] diff ≤400 行（锁文件/生成物除外）；改动在任务文件清单内
- [ ] S3 测试规格先行且已批准；本 PR 未放宽任何断言
- [ ] CI 全绿：fmt/clippy/test/覆盖率差值/基准差值/deny/audit
- [ ] AI 对抗审查报告已附（docs/reviews/），发现项已修复或驳回（写明理由）
- [ ] 人工终审意见已记录；R2 级有第二名人类评审
- [ ] 新依赖：ADR + cargo deny 通过（无则勾 N/A）
- [ ] 文档/CHANGELOG 已更新
