# M7-WP02-T02 复核报告：OSCP（M4-D2）——M4 渗透探针 HEAD 重跑 + 攻击面增量映射（AI 执行）

> **报告编号**: SEC-AI-RECHECK-M7-WP02-OSCP
> **复核对象**: SEC-AI-PENTEST-M4-WP06-001（M4 LLM 多模型对抗渗透，18 探针，
> P0=1/P1=1/P2=17/P3=13）对当前 HEAD 的持续有效性
> **复核基线**: main = `241cb6a`（2026-09-30）
> **上游**: M4-report §5-D2（OSCP 0.5 天复核修订路径：LLM-only 报告 +
> 0.5 天复核即满足执行方案 §7.1.10）· SPEC M7-WP02 §3 T02 · M6-report §9
> **执行方式**: AI 执行（GLM-5.3-Flash，探针全量重跑 + 逐项处置核验）
> **复核结论**: **Conditional Pass（AI 执行，不构成外部审计；OSCP 持证人
> 0.5 天人工复核义务未清偿，挂资金回笼）**

---

## 1. 结论速览

| 项 | 结果 |
|---|---|
| ① 18 原始探针 HEAD 重跑 | **全绿**（`cargo test -p partisync-gateway --test pen_test`：**24/24** = 18 原始 + 6 条 M5/M6 期新增整改回归探针，0 失败 0 忽略，4.76s） |
| ② P0×1 处置有效性 | **已修复**（include_vectors 默认 false + 回归探针双保险，§3） |
| ② P1×1 处置有效性 | **处置可接受**（重建代价已降为常数级纯函数 + stdio 本地通道无远程暴露；频率限制未实施，诚实登记 §3） |
| ③ M4→M7 攻击面增量 | 四类新面全部有既有验证覆盖（§4 映射表），**覆盖缺口 0 项** |
| 新发现 P0/P1 | **0 项** |

## 2. 复核方法与局限

- **方法**：探针测试套件原样重跑（不放宽断言，铁律 5）；P0/P1 处置到
  代码证据；攻击面增量对照 M4 威胁模型四组（注入/鉴权/DoS/边界）逐类
  映射到 M5–M7 的既有验证工件（探针/property/对抗审查）。
- **局限（R4 自审自证）**：探针与被测代码均为 AI 产物；本复核为同源
  AI 执行——**不满足执行方案 §7.1.10 的人工 vendor 签字**，OSCP 持证人
  0.5 天复核义务未清偿（延续义务见 §5）。

## 3. P0/P1 处置有效性（HEAD 代码证据）

### P0 T3.4.I（dataset_export include_vectors 默认导出向量）

- **处置核验**：`mcp.rs:430` schema 声明 `"include_vectors": {"default": false}`；
  参数结构默认反序列化为 false（bool 默认）。原始攻击载荷
  `params: {include_vectors: true}` 现为**显式 opt-in**，非默认行为。
- **回归探针**：`pen_dataset_export_vectors_default_off`（默认不含向量）+
  `pen_dataset_export_vectors_explicit_off`（显式关断）双探针绿。
- **判定：已修复且带回归防线。**

### P1 T-NEW.7（tools/list 重复请求触发 schema 重建）

- **原始修复建议**（频率限制 / 惰性加载 / 缓存 TTL）**均未逐条实施**——
  但 HEAD 形态使威胁消解：`list_tools` → `all_tools()`（`mcp.rs:371`）为
  纯函数常数级构造（JSON 字面量，无 DB/IO），重复调用代价可忽略；通道为
  本地 stdio 侧车（桌面壳/CLI 本地客户端），**无远程匿名暴露面**，频率
  攻击需要本机执行权（威胁模型升级）。
- **判定：处置可接受（威胁已因架构演化消解）**。诚实登记：若未来
  gateway 暴露网络端点（联邦路由场景），需补频率限制——登记观察项 O-1。

## 4. M4→M7 攻击面增量映射（SPEC T02 第 ③ 项）

| 增量面 | 引入时点 | 验证覆盖（引用，不重做） | 缺口 |
|---|---|---|---|
| **ext-host WASM 沙箱**（最大新面） | M7-WP01 T01–T04 | [P13] per-call 拒绝全量探针 + [P14] 注权 fail-closed（30+ 测试）；T01/T02/T03 三轮 R2 对抗审查（含**变异测试**手法：绕 trait/篡改入参/错误 JSON 退化）；PR #34 加固（IndexRead→Result 消除 panic 越界、preflight fail-closed、`MAX_QUERY_BYTES=8KiB`、RFC 8259 转义）；R8 guest 无终止保障→`spawn_blocking`+10s timeout 兜底 | 0（epoch/fuel 正解留后续 WP，WP01 SPEC R8 已登记） |
| **gateway 扩展工具面**（ext_list/ext_*） | M7-WP01 T04 | `ext_` 前缀 + `source:"extension"` 标注（扩展结果不隐式获得宿主信任，WP01 SPEC R4）；扫描 fail-closed；撞名加载期拒绝 | 0 |
| **桌面壳 IPC/CSP 面** | M6-WP03 → M7-WP01 热修链 | CSP `script-src 'self'` 严格不放宽；inline onclick 全仓 grep=0 判例（#40）；IPC 参数绑定修正（#38）；sidecar handshake + 每请求 `_meta` 协议合规（#39）；stdout 协议通道纯净化（#43）；面包屑/状态渲染修正（#38/#43） | 0（WKWebView 缓存击穿判例=工程债非安全债） |
| **partisync-mcp 可用性面**（graph.db 首启） | M7-WP01 | PR #42 `mode=rwc` + 父目录创建（回归探针 + 真机 e2e）——可用性修复，无权限面扩大 | 0 |
| **侧车生命周期**（McpSidecar 懒 spawn + oneshot pending map） | M6-WP03-T05 | bash stub 单测 + 真二进制 e2e（`mcp_call_real_sidecar_ext_list`） | 0 |

**结论**：M4→M7 四类增量面均有入仓验证覆盖且各自由对应 WP 的对抗审查
背书；探针套件对既有 MCP 工具面的行为契约在 HEAD 完整保持（24/24）。

## 5. 延续义务登记（M7-WP00 §5 R4 格式）

- **义务**：OSCP 持证人 0.5 天复核本报告 + SEC-AI-PENTEST-M4-WP06-001 +
  内部 18 探针（M4-report §5-D2 修订路径的「一页签」收尾）→ 闭合 M4-D2。
- **状态**：**未清偿**，挂「资金回笼」事件触发（与 M2-D1 外部审计同窗口，
  RFP-OSCP-SECURITY-AUDIT-M5 采购路径保留待用）。
- **复核留痕**：本报告 + T01 报告 = M7 关账（WP99）债务表回填素材。

## 6. 观察项与复核日志

| # | 级别 | 项 | 去向 |
|---|---|---|---|
| O-1 | 观察 | gateway 若暴露网络端点（联邦路由演进），tools/list 需补频率限制（P1 T-NEW.7 原建议的剩余项） | 债登记，触发条件=网络暴露面出现 |

- 复核时间：2026-09-30 · 基线 main=`241cb6a`
- 执行：GLM-5.3-Flash（ZCode 会话，M7-WP02-T02）
- 测试：pen_test **24/24 绿**（18 原始 + 6 整改回归；macOS 26.6.2 arm64，1.94 pin）
