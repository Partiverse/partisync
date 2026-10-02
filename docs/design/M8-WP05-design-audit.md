# M8-WP05 设计审计——功能覆盖 × shadcn 标准偏离（v4 原型评估）

> 任务: M8-WP05-T00 · 日期: 2026-10-02 · 对象: 设计 v4 原型
> （mockups/*.html）× 当前已开发功能全集 · 方法: 功能→组件覆盖矩阵
> 逐项核对 + shadcn 设计标准逐项对照 · 执行: GLM-5.3-Flash (ZCode)

## 1. 功能 × 组件覆盖矩阵

| 功能（function-map 域） | 所需组件 | v4 原型 | 裁定 |
|---|---|---|---|
| A 统计概览 | 统计展示 | tally 仪表阵列（sync 页形态） | ✅ 可复用 |
| B1 目录浏览 | 表格 + 面包屑 + 行选中 | ✅ browse 双栏 | ✅ |
| B2 条目详情 | 详情面板 + 键值 dl + 指纹色带 + 转写引用 | ✅ + **去重星图**（v4 新增） | ✅ |
| C1/C2 检索 | 检索框 + 模式开关 + 结果行 + 评分 + 高亮 | ✅ search | ✅ |
| C 检索**加载中** | Skeleton / 加载指示 | ❌ **无** | **补**（states 页） |
| C 检索**无结果** | 空态（动作邀请） | ❌ **无** | **补** |
| E/F 作业/同步**错误** | 错误警示（destructive 语义） | ⚠️ 仅全局错误条，amber 与冲突混用 | **补** destructive 语义分离 |
| G 扩展调用 | 表单（textarea + 按钮） + 输出区 | ⚠️ v1 有雏形未入 v4 | **补**（states 页） |
| 全局**破坏性操作确认** | Dialog / 确认模态 | ❌ 无 | **补**（后续写路径前置件） |
| 全局**操作反馈** | Toast | ❌ 无 | **补** |
| 任意列表**分页/续读** | cursor 分页控件 | ❌ 无（list_children cursor 后端已支持） | **补** |
| 面板内**次级按钮/幽灵按钮** | Button variants | ⚠️ 仅 primary 实心 | **补** variants |

## 2. shadcn 标准偏离评估

| shadcn 标准 | v4 现状 | 裁定 |
|---|---|---|
| HSL token 体系（background/foreground/card/popover/primary/secondary/muted/accent/destructive/border/input/ring） | 有大半，**缺 `--destructive` 与 `--secondary`/`--input`** | ⚠️ **补 token**（destructive 与 amber 分离：amber=冲突警示数据态，destructive=操作错误态） |
| radius 0.5rem | radius 0（全直角） | ✅ **有意偏离**（用户硬性要求直角 + borgbackup 判例）——登记不回改 |
| sans 正文 | 全 mono 正文 | ✅ **有意偏离**（borgbackup 判例：mono 制造终端感；信息密度工具正当） |
| dark mode 一等公民 | 仅 dark | ✅（本地工具无 light 需求；登记） |
| focus ring（ring token + halo） | ✅ query-row focus-within | ✅ 对齐 |
| 组件边框驱动（borderCard，无投影） | ✅ panel/border 驱动 | ✅ 对齐 |
| RadioGroup/Table/Alert 形态 | 自制实现，token 对齐 | ✅ 形态对齐 |
| Toast/Dialog/Skeleton 官方组件 | 无 | **补**（自制最小形态，token 对齐——桌面壳不引 React 生态，形态照抄 token 自管） |

## 3. 补充裁定（v4.1）

1. **token 补齐**：`--destructive`（错误操作语义，独立于 amber）、
   `--secondary`（次级按钮底）、`--input`（表单边框）；
2. **组件状态画廊**新增 `mockup-states.html`：Skeleton 加载（检索中）、
   检索空态（动作邀请）、Toast（操作成功/失败）、确认 Dialog（破坏性
   操作）、 destructive 按钮/错误警示、cursor 分页控件、按钮 variants
   （primary/secondary/ghost/destructive）；
3. 全局错误条（H1）与 destructive 语义对齐（`{kind}` 分类映射）；
4. 实施约束：T1–T4 前端落地必须覆盖「加载/空/错」三态（shadcn 惯例
   + 本审计裁定），不允许只做 happy path。

## 4. 图标审计（v4.2，2026-10-02 用户补充）

### 4.1 现用符号清单（8 种，全 Unicode 几何系/CSS 绘制，零图标库）

| 符号 | 语义 | 位置 | 平台风险 | 裁定 |
|---|---|---|---|---|
| `▸` | 目录展开指示 | browse 表 | 安全（文本渲染） | ✅ 语义单一化：sync tally cap 前缀 `▸` 已删除（此前与目录指示混用） |
| `·` | 文件行指示（低调粒子） | browse 表 | 安全 | ✅ 维持 |
| `⌖` | 检索空态符号 | states | 低（冷门字形豆腐风险） | ✅ 维持；登记观察 |
| `■` | 成功 toast / 错误警示前缀 | states | 安全 | ✅ |
| `✕` | 失败 toast（v4.2 改：原与成功同 `■` 仅靠色分，色盲不友好 → **形状区分**） | states | 安全 | ✅ 修正 |
| `◆` | 冲突警示前缀 | states | 安全 | ✅ v4.2 结构化进 HTML（CSS content → span aria-hidden） |
| `⚠` | 破坏性 Dialog 标题前缀 | states | **高：Windows/部分平台回落彩色 emoji** ⚠️ | ✅ v4.2 修正：`&#xFE0E;` 文本变体锁定 + aria-hidden |
| `→` `←` | 分页方向 | states | 安全 | ✅ 维持 |
| CSS 绘制 | 三方块集结符号 / 指纹圆粒 / 指纹条 / score 条 | 全局 | 无 | ✅ 维持 |

### 4.2 审计裁定（v4.2 修正 + 常设规则）

1. **可访问性修正**：全部装饰性符号补 `aria-hidden="true"`（此前零覆
   盖——屏幕阅读器朗读「黑方块黑方块」噪音）；CSS `content` 内符号
   改 HTML `span` 结构化（多数读屏器朗读 CSS content）；
2. **形状冗余**：成功/失败语义必须「形状 + 色」双通道区分（`■`/`✕`），
   禁止仅靠色分（色盲可达性）；
3. **emoji 回落防御**：`⚠` 类有 emoji 变体的符号加 `&#xFE0E;` 文本变
   体锁定；新符号选型优先 U+25A0–U+25FF 几何区块（无 emoji 变体带）；
4. **符号语义单一化**：一个符号一种含义（`▸` 只作目录指示）；
5. **路线登记**：维持「零图标库 + Unicode 几何系 + CSS 绘制」（borg-
   backup 判例 + 零依赖 + CSP `script-src 'self'` 友好）；shadcn 的
   lucide 图标库路线为**自觉偏离**（引入需评估 React 生态前提）；
6. **文件类型图标**：browse 表维持目录 `▸` / 文件 `·` 双态极简（类型
   详情在详情面板「类型」字段），不扩符号集。

### 4.3 修正执行

`states.html`（toast 形状区分 + aria 结构化）、`sync.html`（cap 前缀
删除）、`browse.html`（行指示 aria-hidden）；states 截图刷新。

## 5. 复核日志

- 功能全集对照 function-map §1（A–H 域）；shadcn 标准依据其公开
  稳定规范（token 语义色清单 + 组件形态惯例）；有意偏离三处均登记
  理由（直角/全 mono/仅 dark——用户要求或工具正当性）。
