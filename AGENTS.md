# AGENTS.md — AI 代理作业规范（根上下文）

> 本文件是所有 AI 编码代理的第一上下文。每个 crate 可有自己的局部 AGENTS.md 补充契约。
> 治理总纲见 [docs/partisync-AI开发执行方案.md](docs/partisync-AI开发执行方案.md)（下称「执行方案」）；
> 技术蓝图见 [docs/partisync-调研与构建方案.md](docs/partisync-调研与构建方案.md)。

## 十条铁律（摘要，全文见执行方案 §1.1）

1. 规格先行：无已批准 SPEC 不写实现。2. 追溯唯一：提交必须挂 Task-ID。3. 原子交付：单 PR ≤400 行 diff。
4. 测试即规格：行为契约先于实现。5. 门禁前置：CI 红灯禁止合入，覆盖率只升不降。
6. 双通道审查：AI 对抗审查 + 人工终审。7. 地基人工：schema/密码学/unsafe/公共 API 双人评审。
8. 无聊依赖：新依赖需 ADR + cargo deny。9. 会话即弃：一任务一会话，不越任务文件清单。
10. 审计即工件：里程碑不出规定工件 = 未完成。

## 提交与追溯格式（强制）

```
<type>(<scope>): <subject> [M{m}-WP{nn}-T{nn}]

Task-ID: M{m}-WP{nn}-T{nn}
Spec: docs/specs/M{m}-WP{nn}.md
AI-Assist: <agent/model> (做了什么)
AI-Review: <agent/model>            # 有 AI 对抗审查时
Reviewed-By: <human-id>             # 人工终审后
```

编号速查：任务 `M2-WP03-T07` · 分支 `m2/wp03-t07-slug` · 规格 `docs/specs/M2-WP03.md`。

## 常用命令

```bash
cargo fmt --all --check        # 格式门禁
cargo clippy --workspace --all-targets -- -D warnings   # lint 门禁
cargo test --workspace         # 测试门禁
cargo xtask trace M-1-WP07-T01 # 追溯某任务全链
cargo xtask report M0          # 生成里程碑报告骨架（已存在不覆盖）
cargo run -p partisync-cli     # CLI（index/ui/watch/ls/find/dedupe 等子命令）
```

## 禁止事项（红线）

- 禁止新增顶层依赖（须 ADR + cargo deny 通过，先问）。
- 禁止修改 CI 门禁阈值、deny.toml 白名单、rust-toolchain.toml（均需 ADR）。
- 禁止删除/跳过测试、放宽断言来「让测试变绿」——放宽断言本身需要独立 PR 与理由。
- 禁止凭记忆编写第三方 crate API——必须查证该版本文档/源码后使用。
- 禁止在本仓库引用或逐字复制任何许可证不明的代码片段。
- 禁止 unsafe（workspace 已 forbid；需要时先提 ADR）。
- 禁止改动任务卡声明文件清单之外的文件（「顺手修」一律另开任务）。

## crate 地图与依赖方向（自上而下只允许向下依赖）

```
partisync-cli / partisd        # 壳：组装与入口
  → gateway, ai, index, sync, transfer   # 能力层
      → provider, cas, graph             # 领域层
          → core                          # 基础类型（被所有人依赖，不依赖任何人）
partisync-hub                  # 独立服务（依赖 graph/cas/core）
xtask                          # 开发工具（不参与产品依赖图）
```

跨层反向依赖、能力层互相依赖（如 transfer → sync）需 ADR。

## 桌面端验收规则（2026-10-02 用户指令，硬性；2026-10-09 D19 修订）

> **2026-10-09 修订（用户指令，partiverse D19 定位裁决落地）**：
> partisync-desktop 进入维护冻结（停新功能、仅必需维护；废弃条件 =
> Partiverse V2 完成 crate 级集成，届时 GUI 由 Partiverse 接替）。
> 自本日起：①**不再新增桌面 GUI 功能任务**（冻结令，M11 提案 v0.3
> §4-4a 已拍板）；②**测试与验证一律走 CLI / partisd 守护进程 /
> MCP 面**，**无需再实际操控 GUI 界面**（下述 GUI 实操要求仅对冻结
> 前已挂「待 GUI 验证」的存量项残留适用，此类项已全部清账）；
③强化 **CLI 交互能力**为 M11 主线组成部分（partisd 守护进程的 CLI
消费面）。

- **凡涉及桌面端（`partisync-desktop`）功能/界面的任务，AI 必须在申请
  PR 前，实际操控电脑（GUI 实操/截图验证）确认实现情况符合预期**——
  纯单测绿不算桌面端验收通过；（**2026-10-09 冻结令后不再产生此类
  任务**，本条留作历史判例与存量回归参照）
- 验证证据（截图/操作记录）随 PR 正文或 docs/screenshots/ 归档；
  CLI/守护进程/MCP 面的验证证据以命令输出/基准数字归档；
- 无法实际操控时（环境不可得），PR 保持 OPEN 并显式标注「待 GUI 验证」，
  不得自行合入。

## 风险分级（评审深度）

- R0 纯内部有测试 → 抽审 30%；R1 跨 crate/IO 密集 → 全审；R2 钉子清单 → 双人全审。
