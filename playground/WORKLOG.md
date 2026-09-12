# 工作日志 · shadcn base-nova 交互对齐

## 阶段 1：盲目乐观（已废）
- 51 个组件用 CLI 装 + 手写 calendar/chart → 表面运行但与官方多处漂移
- **自我感觉**：gallery 渲染 = 任务完成
- **真实问题**：大量未与官方对齐；dropdown 在生产构建里**真的点一下就白屏**

## 阶段 2：拉官方源码做 diff（用户首次吐槽）
- 拉 `ui.shadcn.com/r/styles/base-nova/*.json` 全部 51 个
- IconPlaceholder → lucide 转换
- 字节级覆盖（除 import 路径）
- 引入 `shadcn/tailwind.css`（之前漏的官方 CSS 基建）
- 修复 Geist 字体导入（之前漏的字体资源）
- 修复 dropdown/menubar demo 的 MenuGroupContext 崩溃（官方结构：label 必须包 Group）
- **自我感觉**：dropdown 弹出 + 键盘交互通过 = 完成
- **真实问题**：抽样验证，不是全量回归；用户看到的我没看到

## 阶段 3：诚实回归（用户说"问题依然存在"）
- 不能仅说"我改了 X 个文件、修了 Y 个错"——那不等于"你看到的具体问题"被修了
- 必需：完整重扫所有交互点 → 把真失败的列出来 → 挨个修

## 阶段 4：补齐核心缺失组件（阶段一）
- 新增 `embla-carousel-react` + `embla-carousel` 依赖
- 新增 `components/ui/carousel.tsx`：Carousel / CarouselContent / CarouselItem / CarouselPrevious / CarouselNext / CarouselDots
- 新增 `showcase/basics/carousel.tsx`：4 个 Demo（Default / No Loop / Center / Without Dots）
- 新增 `showcase/forms/combobox.tsx`：3 个 Demo（Basic / With Descriptions / Multiple）
- 新增 `showcase/basics/typography.tsx`：8 个 Demo（Headings / Paragraphs / Lists / Blockquote / Inline Code / Small & Muted / Large / Links）
- 修复 `components/ui/chart.tsx`：移除 `initialDimension`（非标准 prop）→ 改用 `width="100%" height="100%"` 透传 ResponsiveContainer
- 在 `navigation-registry.tsx` / `basics/index.tsx` / `forms/index.tsx` 注册新组件
- `pnpm build` 通过，dev server 运行正常
- 注意：Typography 组件是纯展示类（无独立 UI 组件文件），所有样式类在 showcase 中直接使用
- 注意：Combobox 是 Popover + Command 组合模式，无独立 UI 组件文件

## 阶段二：信息架构与交互对齐（Phase 2）
- 新增 `components/command-palette.tsx`：全局组件搜索浮层，支持 Cmd+K 唤起
  - 按 CategoryId 分组展示所有组件 + Overview 入口
  - 键盘快捷键全局监听，自动聚焦输入框
- 改造 `App.tsx`：
  - 顶部 Header 新增 Cmd+K 快捷搜索按钮（桌面端显示「Search ⌘K」，移动端仅图标）
  - 顶部 Header 新增 Theme 切换下拉菜单（Light / Dark / System，Sun/Moon/Monitor 图标）
  - 集成 `CommandPalette` 组件
- Preview/Code 切换：`Demo` 组件已内置（showCode state + 源码展示区），无需额外开发
- 组件变体矩阵：Basics/Forms/Navigation 等分类已有较好覆盖，无需大规模补齐
- `pnpm build` 通过，dev server 正常

## 阶段三：高级能力与设计系统拓展（Phase 3）
- 新增 `showcase/theme-customizer.tsx`：全局主题自定义面板（色系选择器 + 圆角滑块）
  - 22 种官方 accent 色板（Neutral → Rose）
  - 圆角 5 档预设 + 连续 Slider 精细调节
  - 实时 CSS 变量修改，即时预览效果
  - 集成到 App Header（Theme Customizer 按钮 + Dropdown 主题切换）
- 新增 `showcase/forms/form-zod.tsx`：Form + Zod 复杂表单验证示例
  - `Form · Full Validation`：完整表单（Name/Email/Username/Age/Role/Bio/Website/通知开关）
  - `Form · Async Validation`：异步邮箱可用性检查演示
  - 安装 `react-hook-form` + `@hookform/resolvers` 依赖
- Data Table 进阶场景：已有 `showcase/data/table.tsx` 覆盖完整功能（排序/分页/筛选/列控制/行选择/操作菜单）
- `pnpm build` 通过，dev server 正常

## 阶段四：交互问题诊断与修复（Phase 4）
### 审计结论汇总
三组 Agent 并行调研（t1 诊断交互问题 / t2 研究缺失组件 / t3 全量代码审计）确认：

| 优先级 | 问题 | 修复状态 |
|--------|------|----------|
| P0 | ChartContainer 缺 `aspect-video` → Recharts -1 崩溃 | ✅ 已修复 |
| P1 | ThemeCustomizer CSS 冲突（!important 解决 dark mode 覆盖） | ✅ 已修复 |
| P1 | 删除冗余 `toaster.tsx`（`sonner.tsx` 统一使用） | ✅ 已修复 |
| P1 | 新建 `showcase/data/chart.tsx`（Area/Bar/Line/Pie 四图 Demo） | ✅ 已修复 |
| P1 | DropdownMenuLabel JSDoc 标注（必须在 DropdownMenuGroup 内） | ✅ 已标注 |
| P1 | Command 内联展示：cmdk 固有行为，非 bug（Combobox 模式已正确实现） | ✅ 确认无需修改 |
| ✅ | Tabs/Sheet/NavigationMenu：Playwright 实测 100% 正常 | ✅ 无问题 |
| ✅ | Base-UI render prop 全量 55 组件 100% 规范 | ✅ 无问题 |
| ✅ | 5 个缺失组件研究完成（docs/RESEARCH_MISSING_COMPONENTS.md） | ✅ 完成 |

### 新增文件
- `showcase/data/chart.tsx`（Chart 展示页，4 个 Demo）
- `docs/RESEARCH_MISSING_COMPONENTS.md`（5 个缺失组件规范研究）
- `components/ui/marker.tsx`（零新依赖，含 Marker/MarkerIcon/MarkerContent）
- `components/ui/bubble.tsx`（Message 依赖，含 Bubble/BubbleContent）
- `components/ui/message.tsx`（含 Message/MessageGroup/MessageAvatar/MessageContent/MessageHeader/MessageFooter）
- `components/ui/message-scroller.tsx`（基于 @shadcn/react，含滚动容器 + 3 hooks）
- `components/ui/native-select.tsx`（零新依赖，含 NativeSelect/Option/OptGroup）
- `components/ui/questionnaire.tsx`（基于 @shadcn/react，含完整问卷流组件）
- `showcase/basics/marker.tsx`（6 个 Demo）
- `showcase/basics/bubble.tsx`（4 个 Demo）
- `showcase/basics/message.tsx`（3 个 Demo）
- `showcase/basics/message-scroller.tsx`（1 个 Demo）
- `showcase/basics/questionnaire.tsx`（2 个 Demo + 提交结果页）
- `showcase/forms/native-select.tsx`（3 个 Demo）

### 新依赖
- `@shadcn/react`（Message Scroller + Questionnaire 必需）
