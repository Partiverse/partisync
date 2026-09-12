# TASK-CARD: partisync MCD 首版工程落地

## 1. 任务基础信息

- **任务名称**: partisync MCD 核心闭环开发与压测基准（当前执行切片：第一梯队 T1+T2 基础设施与核心引擎骨架）
- **关联路线**: `docs/SESSION.md` (v2) 及 `docs/decisions/ADR-001-mcd-architecture.md`
- **执行模式**: 智能模式（执行级硬门）
- **架构形态**: 双模同构（Go Headless 服务端/REST API + 预留 Wails 桌面端入口）
- **预设领域**: 通用开发 / Web全栈
- **高风险标识**: **是**（跨后端Go、中间件Postgres+Meilisearch容器、路径防护与高吞吐架构）

---

## 2. Agent 团队与模型档位分配（显式指定）

根据智能模式 Q10A 及当前会话 modelTiers 配置显式指定：
- **fast**: `minimax-cn/MiniMax-M2.7-highspeed`（机械/脚手架任务）
- **standard**: `vectide/glm-5.3`（普通实现任务）
- **strong**: `vectide/glm-5.3`（架构规划、高风险、代码审计、性能审查）
- **vision**: `zai/glm-5.3-flash`（图像任务；本任务暂不涉及图模直接分析）

| 角色 / Agent 标识 | 负责模块 | 指定 Provider / Model | 档位 | 职责说明 |
|---|---|---|---|---|
| **Lead / Orchestrator** | 全局协调与验收 | `vectide/glm-5.3` | strong | 驱动 DAG 状态流转、目标防漂移与最终 L3 验收核查 |
| **Backend-Coder** | Go 后端、API、DB 与任务队列 | `vectide/glm-5.3` | standard | 实现资产上传、元数据入库、Meilisearch 同步与慢标注 Worker |
| **Frontend-Coder** | Web 检索与标注 UI 对接 | `vectide/glm-5.3` | standard | 基于 playground 现有 shadcn 资产改造为资产库与标注工作流界面 |
| **Audit-Reviewer** | 质量门禁与安全审计 | `vectide/glm-5.3` | strong | **审计成员（独立强档）**：检查 SQL 注入、路径遍历、数据竞争、错误处理与断言 |
| **Perf-Tester** | 百万基准测试与 L3 脚本 | `vectide/glm-5.3` | strong | 生成百万元数据、执行并发压测、断言 p95 < 100ms 检索指标 |

---

## 3. MCD 交付物范围（边界与非目标）

### MCD 交付物（Scope）
1. **Docker Compose 单机编排**：
   - `server`: Go 核心 API 服务
   - `postgres`: PostgreSQL 16 关系数据库与任务队列
   - `meilisearch`: v1.8+ 纯 Rust 倒排检索引擎
2. **Go 核心服务端**：
   - 资产上传 API（单文件/批量上传，SHA256 去重与存储落盘）
   - 资产同步与元数据入库逻辑
   - Meilisearch 自动同步索引通道
   - 慢标注任务队列（基于 Postgres `FOR UPDATE SKIP LOCKED`）与模拟 AI 标注 Worker
3. **Web 交互前端**：
   - 资产库瀑布流 / 虚拟滚动网格
   - 毫秒级即时搜索栏（拼写容错、多维属性过滤）
   - 资产详情弹窗与分类/文本标注面板
   - 异步慢标注任务提交与状态看板
4. **性能验证产物**：
   - `scripts/benchmark-1m.sh`：自动生成 100 万资产元数据并压测搜索接口的自动化脚本
   - 性能测试报告：证明检索延迟在 100 万数据集下稳定处于 `p95 < 100ms`

### 非目标（Explicit Non-Goals）
- ❌ 视觉框选/多边形标注编辑器
- ❌ 复杂的 SaaS 云盘 OAuth 连接器
- ❌ 多人协同与细粒度 RBAC 权限
- ❌ 客户端端到端零知识加密 Vault
- ❌ 实时 GPU 嵌入模型推理

---

## 4. 任务分解 DAG

```text
[T1: 容器编排与基础设施] (Docker Compose + DB Schema)
       │
       ▼
[T2: Go 服务端资产与检索核心] ◄───┐
       │                          │
       ▼                          │
[T3: 慢标注异步任务系统]           │ (依赖与解耦)
       │                          │
       ▼                          │
[T4: Web 端资产库与标注联调] ─────┘
       │
       ▼
[T5: 100 万资产生成与压测基准验证]
       │
       ▼
[T6: 独立架构与安全审查 (Audit)]
       │
       ▼
[T7: L3 真实入口验收与发布]
```

---

## 5. 验收标准与证据分级

| 级别 | 验收方式 | 必须包含的证据产物 |
|---|---|---|
| **L1 (单元测试)** | `go test ./...` 与前端 lint | Go 核心逻辑测试通过率 100%，无数据竞争（`-race`） |
| **L2 (集成测试)** | 真实 Docker 环境中测试上传→索引→检索闭环 | API 自动化集成脚本执行日志与返回值断言 |
| **L3 (真实入口验收)** | 浏览器端完整走通业务链路 + 百万压测实测 | 1. 自动化截图与 DOM 断言写入仓库；<br>2. 100 万资产压测输出的直方图，`p95 < 100ms` 明确通过 |

---

## 6. 审计与质量门禁（Audit Gate）

- 审计角色必须由 **strong 档模型**（`vectide/glm-5.3`）独立执行；
- 审计检查项：
  1. **安全性**：上传文件路径遍历（Path Traversal）、恶意思维扩展名防护、SQL 注入；
  2. **并发与健壮性**：Go channel/goroutine 泄漏、Context 超时传递、DB 连接池耗尽防护；
  3. **指标真实性**：百万测试数据是否具备随机性与真实文本分布，杜绝虚假缓存命中；
- **硬性要求**：审计报告结论必须为 `verdict=pass` 且无 open blocker/high findings 才能交付。
