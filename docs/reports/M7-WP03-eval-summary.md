# M7-WP03 评估汇总（T04）—— SMB 桥接评估 WP 收官

> 任务：M7-WP03-T04（SPEC M7-WP03 §3 T04，2026-09-30 批准 PR #47）
> 日期：2026-09-30 · 基线 main=`940fb5c`
> 结论：**评估完成**——FUSE 路线技术可行，Samba 桥接实测通；**是否进
> 产品依赖图待用户拍板**（ADR-0026 草案已出）

---

## 1. 三任务结论汇总

| 任务 | 产物 | 结论 | 证据性质 |
|---|---|---|---|
| T01 | [FUSE spike](../reviews/M7-WP03-fuse-spike.md) | **fuser 0.18.0 线位成立**；mountpoint-s3 式语义完整可实现；拒绝面真挂载 2/2 绿；Linux pure-rust 免 C 库 | **实测**（容器真挂载） |
| T02 | [Samba 桥接](../reviews/M7-WP03-samba-bridge-eval.md) | **smbd 可 export FUSE 挂载点**（SMB2 往返通）；oplock/lease 无对象可锁；**鉴权约束捕获未闭环** | **实测 + 纸面分栏** |
| T03 | [企业特性议题](../reviews/M7-WP03-enterprise-topics.md) | 扩展**来源侧**供应链面空白（无签名/registry）；运行时侧已闭合（P13/P14） | **纸面**（只登记不设计） |
| T04 | [ADR-0026 草案](../adr/0026-fuse-posix-gateway.md) + 本汇总 | 推荐采纳 fuser `>=0.18.0, <0.19` 入 gateway 层；**4 项决策前置条件未完成** | 决策建议 |

## 2. SPEC §3 验收项对照

| 验收项 | 状态 | 证据 |
|---|---|---|
| 产品依赖图零改动 | ✅ | 根 Cargo.toml / deny.toml / toolchain 未动；spike 独立 workspace |
| API 零凭记忆 | ✅ | T01 报告 §2 查证表（15 方法签名 / Config non_exhaustive / build.rs 分支） |
| SEMANTICS.md 语义先行 | ✅ | `crates/partisync-fuse-spike/SEMANTICS.md` |
| P15 候选登记 | ✅ | properties.md（spike 期登记，实施 WP 转正） |
| Samba 桥接评估（实测/纸面分栏） | ✅ | T02 报告 §4 |
| 企业特性议题登记 + M8+ 建议表 | ✅ | T03 报告 §3/§4 |
| ADR-0026 草案 | ✅ | 状态「草案」未接受 |
| **是否立项实施 WP 由用户拍板** | ⏸ **待拍板** | ADR-0026 §决策前置条件（4 项未闭环） |

## 3. 关键决策点（供拍板）

1. **fuser 进产品图？** 推荐采纳（线位成立 + 语义可行 + Samba 桥接通），
   但须先闭环 4 项前置：鉴权映射方案、冷缓存+macOS 复测、写面二期范围、
   SMB 桥接是否随 FUSE 落地。
2. **SMB 桥接是否随 FUSE 落地？** 可选——局域网场景亦可直挂 FUSE；
   桥接引入属主身份映射约束。
3. **扩展签名何时立项？** T03 建议 P1（生态开放前必做），须先经威胁模型
   拍板。

## 4. 诚实声明

- T02 的 `NT_STATUS_ACCESS_DENIED` **根因未闭环**（假设已登记，复测触发
  条件已列）——不作为「已解决」计入结论；
- T03 全纸面（威胁模型为设计期推断）；
- T01 冷缓存读基准与 macOS FUSE-T 本机路径**未测**（R1 环境前置未满足）；
- 本 WP **未做任何产品代码改动**，全部结论止于「可行 + 约束清单 + 拍板建议」。

## 5. 复核日志

- 执行：GLM-5.3-Flash（ZCode 会话，M7-WP03-T01..T04）
- 提交序列：T01 PR（spike）→ T02（评估）→ T03（议题）→ T04（汇总+ADR）
- 并行说明：T03/T04 在独立 git worktree 完成（协议 §3，避并行会话抢工作树）
