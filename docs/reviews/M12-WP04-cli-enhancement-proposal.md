# M12-WP04 提案 v0.1——CLI 交互能力强化（方向细化，征求意见稿）

> Task-ID: M12-WP04-T01 · 日期: 2026-10-11 · 性质: **细化提案（待用户
> 拍板范围与排序后才立 SPEC，铁律 1）** · 现状盘点证据全部 path:line
> 在案（盘点快照 §0）· 上游: M12 charter WP04（用户拍板主线）+
> AGENTS.md 桌面端验收规则 2026-10-09 D19 修订（CLI = 产品主交互面）

## 0. 现状快照（盘点结论，2026-10-11 main）

- **16 个动词**，手写 `match` 分发（无 clap，main.rs:35-51）；**零
  `--json`、零 `--help`**（全库 grep 零命中）；未知 flag **静默忽略**
  （main.rs:68-72）；usage 仅裸调用可见（main.rs:54-59）。
- 输出全为人读中文文本；stderr/stdout 分离干净；退出码隐含
  （0/1/2/130）且**内部不一致**：`ls` 空结果返 1（main.rs:613-616）
  vs `find` 空结果返 0（main.rs:654-655）；watch Ctrl-C 恒返 0
  （main.rs:791-800）。
- **CLI ↔ partisd 完全脱节**：CLI 无任何 HTTP 客户端依赖（Cargo.toml
  全表核），16 动词全部直连库；`search` 在 CLI 恒降级 BM25
  （main.rs:954-962），而 partisd `--memory-embed` 有 hybrid 三腿——
  同一查询两个入口能力不同。无 `status/ping/stop` 管理动词（pidfile
  TCP 探活已有，partisd main.rs:57-97，仅服务启动互斥）。
- T08 登记微债 ⑩：partisync-fuse usage 文案与解析不符、hub-demo-web
  不识 `--help` 直起服（t8 证据件:134-135）。

## 1. 方向包（四枚，可独立立项）

### P1 机器可读契约层

`--json` 全动词覆盖（增量旗标，默认输出不变）+ 退出码契约文档化并
统一（`ls`/`find` 空结果语义对齐、watch Ctrl-C 130）+ 未知 flag 显式
报错（exit 2）。
价值：脚本/AI 消费者的地基（也是 P2/P4 的前置）；依赖面 = serde/
serde_json（workspace 既有 pin，crate 级新边非新顶层）。工作量：中
（16 动词逐一 + 快照测试）。

### P2 partisd 客户端打通

管理动词 `partisd status/ping/stop`（复用 pidfile 探活逻辑暴露成客户
端命令）+ `search` 守护进程感知（在位走 loopback MCP hybrid 三腿，
不在位回落直连现行为）。
价值：消除「同一查询两个入口能力不同」；GA 已把 loopback 无鉴权列
为设计内（ADR-0032 决策 5）。**依赖面前置**：CLI 需 HTTP 客户端能力
——实现路径二选一：A = 引入 HTTP 客户端 crate（**新顶层依赖 → ADR +
cargo deny 前置**）；B = 复用 gateway 既有 rmcp/axum 客户端面（T 轮
细化时对证可行性）。工作量：大。

### P3 UX 硬化

每动词 `--help`（文案与实际行为一致）+ 裸调用分动词 usage + T08 ⑩
微债清偿（fuse usage / hub-demo-web --help）+ flag 拼错显式报错。
价值：小而确定，立刻提升手感并清登记债。工作量：小-中，R0-R1。

### P4 ga 式 DX 反哺

`partisync doctor` 环境体检一键化（db/cas/index/守护进程/扩展签名状
态先行——partisync-tour `ga` 助手哲学的仓内化）+ 动词命名一致性审查。
价值：D19 后新手引导面（与 TOUR.md 体验对齐）；doctor 与 P2 status
天然协同。工作量：中。

## 2. 推荐排期（待用户修订权）

**T 轮 1 = P3 → T 轮 2 = P1 → T 轮 3 = P2（ADR 前置）→ T 轮 4 =
P4**。理由：P3 清登记债且零风险先行；P1 是 P2/P4 的契约地基；P2 依
赖面最重放后；P4 收尾整合。每轮独立 SPEC + 独立 PR 串（铁律 1/3）。
GA 先行纪律不变：实施合入落在 v0.2.0 tag 之后（charter v1.0 推荐案），
本提案不产码。

## 3. 红线

rc 锁面公共 API 零破坏（`--json`/新动词均为增量面）；桌面冻结令延
续；P2 若走新 HTTP 客户端依赖须 ADR + cargo deny 通过（铁律 8）；
验证一律 CLI/partisd/MCP 面实录归档（D19 修订）。

## 4. 待用户拍板项

1. 四包**范围取舍与排序**（§2 推荐案可整体采纳或重排）；
2. P2 实现路径倾向（A 新依赖 + ADR，或 B 复用 gateway 面先对证）；
3. T 轮粒度（每轮一 SPEC，或 P1+P3 合并一 SPEC）。

## 5. 修订记录

| 版本 | 日期 | 内容 |
|---|---|---|
| 0.1 | 2026-10-11 | 初稿：现状盘点快照（16 动词/零 --json/CLI↔partisd 脱节/退出码不一致）+ 四方向包 + 排期推荐案，待用户拍板 |
