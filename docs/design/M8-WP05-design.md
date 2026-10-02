# M8-WP05 桌面壳设计语言（GUI 实施规范）

> 任务: M8-WP05-T00 · 日期: 2026-10-02 · 适用: 桌面壳全部界面
> （T1–T4 及后续）· 设计原则与 tokens 为**实施规范**——实现样式以此
> 为准；高保真原型见 `docs/design/mockups/*.html`（浏览器直接打开），
> 截图见 `docs/screenshots/M8-WP05-design-*.png` ·
> 执行: GLM-5.3-Flash (ZCode)（frontend-design 流程：计划→自审→构建
> →截图自查）
>
> **v4.3（2026-10-02 borgbackup 源码仓库深度调研：borgbackup.github.io
> clone 实测）**——重大发现：官网新版 index3.html 已有 **three.js 粒子
> 宇宙**（6³ InstancedMesh 方块格 = 存储块、900 星星空、FogExp2 深度
> 雾、数据块螺旋吸入、GSAP ScrollTrigger 编排）——与「微观宇宙」构想
> 同源，验证方向正确。**颜色搭配可取**：三档色分布（90% base / 16%
> mid / 10% hot 荧光）+ 深度雾层次；**交互可取**：滚动编排粒子吸入。
> **v4.3 CSS 化吸收**（three.js 3D 登记探针，不阻塞桌面壳）：①星点
> twinkle 双层交替闪烁（活星空，body::before/::after 双层）；②中央
> 绿核 radial glow（深度层次，FogExp2 的 CSS 化）；③检索结果**聚拢
> 入场**（gather 0.32s stagger——「粒子从星域聚拢到位」，动效唯一
> 升级，截图实证中间帧）。已知微瑕：扫描线层与 twinkle-b 共层（轻微
> 闪烁），后续拆层。
>
> **v4（2026-10-02 用户三次补充：微观宇宙粒子隐喻）**——核心设计
> 逻辑升级：**数据文件/设备 = 粒子（particle）= 微观宇宙中的尘埃/
> 星球/天体**。落地：①多层 radial-gradient 星点底（星域）；②条目
> 指纹 = 圆粒星点 + hover 光晕（`::after` 粒子）；③**去重星图组件**
> （browse 详情：v1/v2 两行粒子，亮 = 新增存储 · 暗 = 复用已有——
> borgbackup 去重可视化的**叙事借鉴 + 原创粒子执行，非样式抄袭**）；
> ④同步大数字 **data-count 滚动入场**（counters 逻辑原创实现，
> reduced-motion 直落终值）。「指纹即视觉」「全等宽/直角/扫描线」
> 不变。
>
> **v3（2026-10-02 用户二次补充：直角 + 赛博朋克感 + 参考
> borgbackup.org 官网）**——官网 tokens **实测提取**（curl 源码）：
> `--bg #020503` / `--green #22D045` / `--panel rgba(8,18,10,.82)` /
> `--panel-line rgba(34,208,69,.22)` / `--text #d7e8da` / **正文全等
> 宽** / **全直角**。赛博朋克增强：CRT 扫描线 overlay、终端提示符
> （`partisync@local:~$`）、`//` 注释式小节标、glow（标题/得分条）。
> v2 shadcn HSL 变量体系被 v3 取代；v1 亮色存档。「指纹即视觉」不变。
> v2（同日）：shadcn × Borg 首轮（存档）。

## 1. 定位与原则

PartiSync 桌面壳是**本地优先引擎的档案工具**：用户的核心对象是「有
内容身份的文件」。设计语言 = **指纹即视觉**——

1. **内容身份是一等公民**：blake3 摘要前 8 位以等宽徽章呈现，且条目
   的指纹色相由摘要派生（暗底荧光 HSL，同内容在任何视图颜色相同）——
   颜色是产品语义，不是装饰；
2. **Borg 集合体 × 赛博朋克**：绿黑底 + 网格 + CRT 扫描线 + 终端
   提示符——「SSH 进本地引擎」的操作感；荧光绿 glow 仅标注活的状态与
   关键数据（克制使用，非霓虹城）；
3. **shadcn 纪律**：全 tokens 走 HSL 变量体系；1px subtle border 驱动
   分隔（无投影）；radius 6px 统一；focus = 2px ring（主色 15% halo）；
4. **动效唯一**：仅检索结果指纹条生长一处入场动画；`prefers-reduced-
   motion` 全局尊重；
5. **文案即指路**：错误说明怎么修、空态给出动作邀请、同一动作全流程
   同名（「检索」按钮 → 「N hits」计数）。

## 2. Tokens

| Token（borgbackup.org 实测 + 赛博增强） | 值 | 用途 |
|---|---|---|
| `--bg` | `#020503` | 页面基底（官网实测） |
| `--green` | `#22D045` | 主色（官网实测；标题/指纹主色带 glow） |
| `--green-dim` / `--green-dark` | `#14803a` / `#0a3d1c` | 次级/深底 |
| `--panel` | `rgba(8,18,10,0.82)` | 面板底（官网实测） |
| `--panel-line` | `rgba(34,208,69,0.22)` | 全部分隔边框（官网实测） |
| `--text` / `--text-dim` | `#d7e8da` / `#8aa892` | 主/次文字（官网实测） |
| `--amber` | `#d7a015` | 冲突/警示（含 glow） |
| 指纹色 | `#22D045 #d7a015 #4ab8d8 #c85fd4 #7ac03a…` | 摘要派生散布（黄金角） |
| 字体 | **全等宽** `ui-monospace → Fira Mono` 栈 | 正文/标题/数据（官网判例：mono 制造终端感） |
| 直角 | `border-radius: 0 !important` | 全组件（用户硬性要求 + 官网判例） |
| 扫描线 | 3px 周期 rgba(0,0,0,.12) fixed overlay | CRT 质感（subtle） |
| glow | `text/box-shadow rgba(34,208,69,.35~.6)` | 标题/得分条/当前 tab（赛博霓虹） |
| 星点底 | 多层 radial-gradient 1–2px 粒子（12 颗静态 + 5 颗 twinkle 交替闪烁） | body 背景（活星空；网格底纹之上） |
| 中央绿核 | radial-gradient 600×400 hsla(152,60%,30%,0.12) | 深度层次（borgbackup FogExp2 的 CSS 化） |
| 聚拢入场 | gather 0.32s stagger（blur(2px)+translateY(-6px)→就位） | 检索结果唯一动效（粒子从星域聚拢） |
| 去重星图 | 两行圆粒（10px，lit=primary glow / dim=border） | browse 详情（讲「同内容只存一次」） |
| 计数入场 | data-count + rAF ease-out 900ms | 同步大数字（reduced-motion 直落终值） |
| 网格底纹 | 32px repeating-linear（3.5% 绿） | body 背景（星域坐标网格） |
| 终端提示符 | `partisync@local:~$` + 闪烁光标 | 检索区/状态横幅引导 |

字号：30 mono（同步大数字）/ 14（wordmark/检索输入/条目名）/ 13 正文 /
12.5 辅助 / 10.5–11（指纹徽章/时间戳）。

## 3. 三大界面结构

### 检索（旗舰）
```
┌────────────────────────────────────┐
│  [ 搜文件名、内容、转写文本……  ][检索] │  ← hero 框（全宽 720px，墨色描边）
│  ○关键词 ●语义 ○含转写文本          │  ← 分段单选（非卡片）
│  14 个命中 · 语义模式 · 耗时 41 ms   │
│  ▌[9f3a2c1d] 名称                  │  ← 每行：指纹条+徽章+摘要高亮
│  ▌路径  片段……        ▂▂▂ 0.92     │     + score 细条（数据可视化）
└────────────────────────────────────┘
```

### 浏览
```
┌─────────────────────────┬──────────────┐
│ 面包屑                   │ 详情面板（右， │
│ 表：▸名称 大小 mtime 指纹 │ 320px sticky）：│
│ （行选中=绿底）           │ 标题/元数据 dl │
│                         │ blake3 色带    │
│                         │ 转写前 4 行     │
└─────────────────────────┴──────────────┘
```

### 同步
```
┌────────────────────────────────────┐
│ [已与 2 台设备保持一致 · 最近对账 4 分钟前] │ ← 一句话状态横幅
│  1,284      96       31       2(琥珀)  │ ← 大数字行（宋体，左对齐）
│  已应用     本机跳过  落选     冲突待处理  │
│  最近活动                              │
│  ▌文件名 来自 X · 路径        14:22     │ ← 按日分组时间线（指纹条延续）
└────────────────────────────────────┘
```

## 4. 实现对照（T1–T4 落地要求）

- styles.css 重写为 tokens 变量体系（现状样式整体替换）；
- 指纹色派生函数进 app-core.js（`fingerprintHue(hex8)`），三处视图
  共用；真实摘要数据处才用等宽；
- 新 tab/面板 DOM 结构对照本文档 wireframe；交互即时反馈不加过渡动画；
- 空态文案：「本机还没有索引文件——运行 `partisync index <路径>` 开始」
  （给动作，不给情绪）。

## 5. 设计审计（v4.1，2026-10-02）

**审计文档**: `docs/design/M8-WP05-design-audit.md`（功能×组件覆盖矩阵 +
shadcn 偏离评估）。要点：

- **token 补齐**：`--destructive`（操作错误，独立于 amber 冲突警示）、
  `--secondary`、`--input`——shadcn 语义色清单补全；
- **组件状态画廊** `mockup-states.html`（截图
  `docs/screenshots/M8-WP05-design-states.png`）：Skeleton 检索加载 /
  Empty 动作邀请 / Button 四变体 / Toast 成败 / 破坏性确认 Dialog /
  conflict≠error 语义分离 / cursor 分页控件；
- **实施硬约束（T1–T4）**：不允许只做 happy path——「加载/空/错」三态
  全覆盖；
- **有意偏离登记**（不回改）：radius 0（用户直角要求）、全 mono 正文
  （borgbackup 判例）、仅 dark 主题——三处均 shadcn 标准的自觉偏离。

## 6. 复核日志

- 三页原型截图自查（headless Chrome 1280 宽）：指纹语言三页贯通、
  唯一警示色仅同步冲突、无模板俗套（对照 frontend-design 校准清单
  逐项核对）；原型为静态示意，数据为真实感占位。
