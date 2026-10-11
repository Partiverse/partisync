# Task: M12-WP00-T01 M12 拍板落纲（五线 charter）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M12-WP00-T01 |
| **类型** | docs-only（里程碑换轨落纲件） |
| **来源** | 用户会话指令「确认，G3 三签确认；M12 = GA 序列执行 + SPEC 回填小尾巴 + install-demo-ext.sh 修复 + 候选 4 复裁 + 强化 CLI 交互能力」（2026-10-11，M11 G3 关账同窗口）；判例：M11-WP00-T01（PR #208，母本） |
| **创建日期** | 2026-10-11 |

## 交付

docs/reviews/M12-roadmap-proposal.md（新）v1.0：用户拍板五线结构化
落纲——①WP01 GA 序列执行（cadence §3.2 清单化：release.yml 去 rc 段
+ CHANGELOG [0.2.0] + rc-feedback 渠道登记可先行；J1 计满最早
2026-10-24 + J4 B1–B8 复跑 + 用户终审门 ×2 后 tag/publish）；②WP02
SPEC 回填小尾巴（WP06 §3 十一项 + WP02 T06 自指框/trailer 指针）；
③WP03 install-demo-ext.sh 验签腿（三方案评估 + SPEC 先行）；④WP04
**CLI 交互能力强化**（新增主线：partisd 消费面动词/输出管道友好/ga
式 DX 反哺/watch 交互；SPEC 先行，不破坏 rc 锁面）；⑤WP05 候选 4 复
裁（只更新判定，实施仍待 partiverse S4 窗口 ≈2027-03 前）+ **GA 先
行排序推荐案**（WP04 实施落 v0.3.0 线，SPEC 起草并行不阻塞 GA）+ 非
目标四项（桌面冻结令延续等）。

## 涉及文件清单（Iron Rule 9）

docs/reviews/M12-roadmap-proposal.md（新）· docs/tasks/M12-WP00-T01-charter.md（本卡）

## 验收

- [x] 五线与用户拍板原文逐项对应，无增删方向（CLI 强化按用户指令立为主线，细节留 SPEC 轮）；
- [x] GA 执行纪律如实承接（cadence v1.1 红线：J1 计满 + J4 复跑 + 用户终审门，未提前任何 GA 动作）；
- [x] 排序推荐案显式标注「待用户修订权」（WP04 实施时点 = v0.2.0 tag 后属推荐非既决）；
- [x] docs-only；零代码 diff；零既有文件修改；纯新增两文件；
- [x] 提交挂 Task-ID `M12-WP00-T01`，CI 全绿后 squash 合入。
