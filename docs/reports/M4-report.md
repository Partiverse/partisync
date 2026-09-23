# M4 里程碑报告（终版：WP01–WP06 全量 · G3 评审材料）

生成：人工撰写（先例 M2-report.md）· 日期：2026-09-23 · 签字时点 HEAD：`d00c1ad`
落档任务：M4-WP99-T05（本报告为该任务唯一交付物）

## 1. 范围与结果（对照执行方案 M4 章；范围变更记录）

M4 主题「AI 管线与检索 + MCP 网关 + iroh 数据面 + C2PA + 安全评估」：6 个工作包全部交付关账，
无范围裁剪。M4 未设 WP00 总纲（以执行方案 M4 章为范围宪章）。

| WP | 交付 | 核心验收 |
|---|---|---|
| WP01 Sidecar 推理管线 | 摄取管线（缩略图/EXIF/OCR/转写/嵌入 + 编排状态机）+ ADR-0017 五依赖进场 | **15 轮随机 panic 崩溃恢复与全量直算严格等价**（L5 口径）；编排开销毫秒级（全驱动 8.2 ms/5 stage）；sidecar 去重节省 33.3% |
| WP02 混合检索 | tantivy 0.26 BM25 + usearch 向量 + RRF 精排全链路（ADR-0018） | IndexEngine 门面 + RRF 融合链路落地；**吞吐基准未实测**（占位待补，见 §2/§5-D5） |
| WP03 MCP 网关 | rmcp 3.4.0 五大工具（search/read/organize/export/status）（ADR-0019） | 工具级离线测试 10 例 + 质量门禁；开放项 6 闭合、1 项用户指令后置（真实 AI Agent e2e，§5-D8）；set_tag 单事务原子性 |
| WP04 iroh 数据面 | ChunkSink/ChunkSource trait + hub 侧 iroh 通道 + ChunkStore 接线（ADR-0015/0016） | **iroh 通道端到端集成测试 2 例 8/8 稳定**（loopback 离线）；iroh =1.2.0 精确锁定；**协议发现：push 为 fire-and-forget → M5 UploadAck 设计直接输入** |
| WP05 C2PA 摄取校验 | C2paStage（管线第六环节）+ schema v15 `content.c2pa` 回写 + asset_read 暴露（ADR-0020） | 校验语义矩阵 **8/8**（valid/absent/篡改保留/不支持容器跳过等） |
| WP06 安全与评估 | MCP/网关威胁模型（STRIDE）+ 攻击面清单 + 外部渗透 RFC；评估集 + 检索评估 runner + **18 个内部渗透探针** | bm25_only 基线 **nDCG@10=0.754**（合成库）；渗透 **18/18 服务不崩、0 高危、0 中危、3 P1**（未修，登记 §5） |

范围变更记录：无裁剪。WP06 内部任务编号微调（原 T06 拆 T06/T07），见 SPEC M4-WP06 注记。

## 2. KPI 达标表（基准报告链接）

| KPI | 口径 | 结果 | 证据 |
|---|---|---|---|
| WP01 管线吞吐/编排开销/崩溃恢复/去重 | criterion + 随机 kill 压力（fake 模型口径：真实推理栈未冒烟，HF 不可达，§5-D6） | ✅ | `docs/reports/bench/m4-wp01-kpi.md` |
| WP02 检索吞吐（BM25/向量 10⁶ 目标 <100 ms） | criterion——**基准文件未建，全表占位**；SIMD 禁用环境（numkong + Apple Clang 16）+ reranker 模型未启用 | ⚠️ **待补** | `docs/reports/bench/m4-wp02-kpi.md`（§5-D5） |
| WP03 五大工具质量门禁 + 开放项闭合 | 工具级离线测试 10 例（真实 agent e2e 后置，§5-D8） | ✅ | `docs/reports/bench/m4-wp03-kpi.md` |
| WP04 iroh 通道端到端 | loopback 直连（离线不依赖 relay）push→CAS 落库 + CAS→fsm 下发 | ✅ 8/8 稳定 | `docs/reports/bench/m3-wp04-kpi.md` §6（M4-WP04 KPI 填实于该底稿） |
| WP05 C2PA 校验语义 | 测试实证矩阵（wp05.rs 8/8） | ✅ | `docs/reports/bench/m4-wp05-kpi.md` |
| WP06 检索质量基线 | **合成库**（50 语料/25 查询/30 qrels，单人标注，3 项偏离披露 vs 计划 200/40 双人盲标）；hybrid 两档留 M5+ | ✅（基线非承诺） | `docs/reports/bench/m4-wp06-kpi.md`：nDCG@10=0.754 / recall@10=0.780 / MRR=0.747 |
| WP06 网关渗透 | 18 内部探针（注入 8/鉴权 4/DoS 3/C2PA 3），真实 partisync-mcp 子进程 | ✅ 0 高危/0 中危；**3 P1 未修** | `docs/reports/security/wp06-pen-test-internal.md`（PEN-M4-WP06-001） |

## 3. 测试证据（门禁复核：签字时点本地实测 + CI 最近一次运行）

- **本地签字复核（2026-09-23，HEAD `d00c1ad`，`NK_TARGET_*=0` 屏蔽 numkong SIMD——Apple Clang 16
  环境问题非代码）**：fmt ✅；`cargo deny --offline check` **4/4 ✅**；
  `cargo clippy --workspace --all-targets -D warnings` **❌ 2 处已知**（cli `main.rs:658` unused store /
  `:712` wildcard_in_or_patterns——即 M4-WP99-T01 登记之债，签字放行接受并限 M5 开局清偿）；
  `cargo test --workspace` **70 passed / 0 failed / 4 ignored**（ignored 为 #[ignore] KPI 基准；
  含 `wp06_eval` 4 + `pen_test` 18；CI 红过的 `iroh_keyspace` 2 测试经 `47276ca` 修复后本地复绿）。
  **退出码 0，测试门禁绿。**
- **CI 最近一次运行（`12f3053`，2026-09-21）**：fmt ✅ / clippy ✅ / test(ubuntu) ✅ / interop ✅；
  test(macos) ❌（`iroh_keyspace` 2 测试——**其后 `47276ca` keyspace 并行隔离修复（2026-09-22）落地，
  本地复核见上行**）；deny ❌（cargo-deny-action 容器 musl 工具链未装 + 当时 licenses 项——其后
  ADR-0020 deny 基线进场，本地现四段绿）。**`12f3053` 之后 47 个提交未推送 origin、未触发 CI**
  （CI 完整性披露，§5-D9）。
- **渗透探针**：18 探针全部「服务端不崩溃」通过（真实子进程模式，与 mcp_e2e 同源）。
- **崩溃恢复**：WP01 随机 kill 15 轮恢复等价性压力测试（M4 DoD）。
- **互操作**：interop job 绿（`12f3053`）；M4 协议面新增 iroh 通道由端到端集成测试承载，rclone 面未动。
- **模糊/混沌/夜间套件**：M4 未安排（网关面以 18 内部渗透探针替代）；M5 建议立项（§8）。
- **变异测试**：沿用 M0 基线（`mutants-m0-wp00.md`），M4 行为契约以验收测试先行承载。

## 4. 安全（cargo audit / deny / unsafe 增量 / 渗透）

- **cargo deny --offline**：licenses/bans/sources/advisories **4/4 绿**（HEAD 实测 2026-09-23）。
- **cargo audit**：1 vulnerability = **RUSTSEC-2023-0071（rsa Marvin Attack，medium 5.9）——即
  ADR-0020/deny.toml 已豁免之同一项**（本仓库仅公钥验证路径，私钥解密侧信道不可达；上游无修复版；
  撤销条件已登记）。audit 不读 deny 豁免故单独报出，**非新发现**。另 7 条 unmaintained 警告同属
  已允许级别（iroh 传递树）。
- **unsafe**：workspace `unsafe_code = "forbid"` 全程未放宽（partisync-cas 仅复述 forbid）；M4 零 unsafe 增量。
- **内部渗透**（PEN-M4-WP06-001）：0 高危 / 0 中危 / **3 P1 未修**——① asset_organize 巨大批次不拒收（DoS）、
  ② 对不存在 content_id 静默插入（跨库越权面）、③ 缺失 c2pa 字段形态不一致（低风险）。
  全部登记 M4-WP99-T02/03/04 预留编号，铁律 9 不顺手修。
- **外部渗透（§7.2 M4 触发点）**：**未执行**——RFC 已就绪（`docs/security/pentest-rfc.md`，4 家 vendor 候选），
  @lead 2026-09-23 同意关账放行并知悉 vendor 委托挂 M5 开局（§5-D2）；内部 18 探针将移交 vendor 独立复核。
- **跨里程碑债务**：M2-D1 外部密码学审计仍未补做（延续挂账，§5-D1）。

## 5. ADR 清单与债务登记

ADR（全部已接受）：0015（iroh 设备通道 hub 侧）· 0016（iroh-blobs 传递依赖 deny 基线，人工终审 `32c6753b`）·
0017（sidecar 推理/图像栈五依赖）· 0018（usearch/tantivy/bge-reranker 检索依赖）· 0019（rmcp 3.4.0）·
0020（c2pa 0.90.22，deny ignore RUSTSEC-2023-0071）。

债务登记（M4 段编号，按偿还窗口排序）：

| # | 债务 | 偿还窗口 |
|---|---|---|
| D1 | （承接 M2-D1）外部密码学审计补做 | M5，外部审计公司委托 |
| D2 | **外部 MCP/网关渗透 vendor 委托执行**（RFC + 4 候选就绪） | **M5 开局 P0** |
| D3 | cli clippy 2 处清偿（M4-WP99-T01，当前 clippy 门禁红之唯一来源） | **M5 开局 P0** |
| D4 | 渗透 3 项 P1 加固（M4-WP99-T02/03/04 预留：批次上限/跨库校验/c2pa 形态） | M5 |
| D5 | WP02 检索吞吐基准实测（建 bench 文件 + reranker 真模型冒烟 + SIMD 环境解决） | M5 |
| D6 | WP01 三推理栈真模型冒烟（HF 可达后补录真实吞吐） | M5 |
| D7 | 真实评估集（200 语料/40 查询、双人盲标、真实 INBOX 分层）+ hybrid 两档评测 | M5+（真实数据面后） |
| D8 | MCP e2e 真实 AI Agent 集成测试（WP03 用户指令后置项） | M5 |
| D9 | **CI 完整性**：47 提交（`12f3053..d00c1ad`）未推送、未触发 CI；恢复推送节奏 | **关账后立即** |
| — | 观察：M3 无里程碑级终版报告（以逐 WP 验收报告承载关门）；如外部审计需要可补档 | 待定 |

## 6. AI 使用披露（自动统计）

- 挂 M4-* 任务的提交数：44（`12f3053..d00c1ad` 全窗口，commit-msg hook 强制 Task-ID）
- AI 辅助提交（AI-Assist）：44/44（多模型：glm-5.3 / MiniMax-M3 / ZCode，trace 可查）
- 人工终审提交（Reviewed-By）：11/44（其余随本 G3 签字一并补认——先例 M2 §6）

## 7. 抽查审计记录（随机 5 任务，仅凭工件重建）

`cargo xtask trace` 抽 5 任务（跨 WP 抽样），全部 commit→spec→AI→files 链完整、无断链：

- M4-WP01-T07 → `f361d614` 基准报告，与 SPEC M4-WP01 验收 + m4-wp01-kpi.md 三方互证；
- M4-WP02-T03 → `f18ab763` 向量索引深化（RRF 精排全链路），spec 对应；
- M4-WP03-T05 → `68309240` KPI 底稿（质量门禁 + 开放项登记），报告与 spec 对应；
- M4-WP04-T06 → 4 提交链（ADR-0016 起草→deny 基线→e2e 测试→人工终审翻批准），**含显式 human 签核记录**；
- M4-WP06-T05 → `9bf997fe` 18 探针，覆盖威胁模型 STRIDE 矩阵，T06 报告汇总行为发现。

## 8. 执行方案 §7.1 G3 检查单逐项裁定

**自动部分**：
1. WP 全关闭、无孤儿提交 —— ✅ WP01–WP06 全关账；44 提交全挂 Task-ID（hook 强制 + 抽查 §7）
2. 覆盖率/变异分数 —— ⚠️ 沿用 M0 变异基线；M4 以验收测试先行承载，覆盖率趋势未见回退红灯
3. 基准报告齐全、KPI 达标 —— ⚠️ 5/6 WP 底稿实测达标；WP02 吞吐占位（§5-D5）；WP06 为合成库基线（3 偏离披露）
4. 模糊时长/崩溃清零 —— ⚠️ M4 未安排模糊；渗透 18/18 无崩溃；WP01 崩溃恢复等价 ✅
5. cargo audit/deny 干净、unsafe 增量已审 —— ✅ deny 4/4；audit 唯一漏洞即已豁免项（§4）；零 unsafe
6. 互操作矩阵全绿 —— ✅ interop 绿（`12f3053`）；iroh 新面由 e2e 承载
7. 混沌/夜间无未分类红灯 —— ⚠️ 无夜间套件（M5 立项建议）；CI 红 2 job 已分类归因（§3）

**人工部分**：
8. ADR 完整、债务登记现实 —— ✅ 6 ADR 全接受；债务 D1–D9 如实登记（含 clippy 红与 CI 缺位）
9. 抽查审计 5 任务重建 —— ✅ §7 全链无断链
10. M2/M4 外部审计无未关闭高危 —— ⚠️ **外部渗透未执行（D2，用户知悉挂 M5 开局）**；内部 0 高危/0 中危；
    3 P1 未修已登记；M2-D1 密码学外部审计延续挂账
11. AI 披露统计合理、无 AI 独断钉子清单 —— ✅ §6；R2 钉子项（KDF/密码学面）M4 未触碰

豁免承载方式：评估集规模/标注口径、外部渗透延后等偏离，按 M2 先例以本报告 §2/§4/§5 登记 +
放行签字知情确认承载，不另立 ADR（非架构决策）。

## 9. 下一阶段建议（M5）

1. **UploadAck push 语义设计**——WP04 协议发现（iroh-blobs push fire-and-forget、早关连接丢数据）是现成输入；
2. M5 开局双 P0：外部渗透 vendor 委托（D2）+ cli clippy 清偿（D3），随后 D4 加固；
3. 真实数据面评估（10⁶ 条目 + 真实 OCR/向量），回填 D5/D6/D7；
4. 恢复 CI 推送节奏（D9）+ 立项夜间/模糊套件；
5. 补 MCP e2e 真实 agent 测试（D8）。

## 放行签字（G3）

- [x] 架构负责人：@lead（用户签收，2026-09-23 会话指令「同意 M4 G3 关账放行」）
- [x] 评审人：@lead（一人团队：同一位承担双角色，见执行方案 D6 注记）
- [x] 安全负责人（M2/M4）：@lead——**附条件放行**：内部渗透 0 高危/0 中危；3 P1 未修登记
      M4-WP99-T02/03/04；外部渗透 vendor 委托挂 M5 开局 P0（RFC 已就绪，用户知情确认）；
      M2-D1 外部密码学审计延续挂账

**M4 附条件正式放行。** 条件即 §5 债务登记（D2/D3/D9 为关账后立即项）。
