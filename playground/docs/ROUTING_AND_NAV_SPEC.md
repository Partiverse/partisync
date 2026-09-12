# ROUTING_AND_NAV_SPEC.md: Playground 单组件独立页面与分类折叠侧栏架构设计规范

## 1. 背景与架构目标

当前 PartiSync Playground 页面（`App.tsx`）按大类（Basics、Forms、Overlays、Navigation、Data、Blocks、Dashboard、Theme）直接堆叠全量组件渲染，导致以下问题：
1. 页面过长：单个大分类（如 Basics 包含 13 个组件，Forms 包含 12 个组件）一次性渲染所有组件及其交互 Demo，DOM 体积庞大，难以快速聚焦定位单个组件。
2. 侧栏层级浅：左侧侧栏仅支持到大分类，无法在侧栏直接查看和直达 49 个原子组件。
3. 右侧 OnThisPage 目录分散：TOC 混合了大分类下的所有组件 Demo，无法针对单个组件进行精确的二级导航。
4. 路由与分享缺失：缺乏针对单组件的深层链接（URL Hash），无法复制链接直达指定组件。

**目标**：
- 升级侧边栏为 **Collapsible 分类折叠** 架构，可折叠/展开各分类并列出其所属原子组件。
- 引入 **单组件独立渲染页面**，支持精准直达 49 个原子组件，右侧 OnThisPage TOC 精准聚合该组件的 Demo 列表。
- **保留分类全览**（All 入口），兼容原有一览式体验。
- 建立统一的 **URL Hash 路由契约**（支持 `#basics/button`、`#button`、`#basics` 等形式），支持浏览器前进/后退与直接刷新定位。
- 底部 Pagination 支持跨组件线性跳转。

---

## 2. 49 个原子组件清单与全览分类注册表

Playground 覆盖全部 49 个原子组件与 3 个特别/块级分类，归类如下：

### 2.1 分类与组件映射表

| 分类 ID (`category`) | 分类标题 | 包含组件 ID (`componentId`) | 对应渲染组件 Section / 源码 |
| :--- | :--- | :--- | :--- |
| **basics** | Basics (13) | `button` | `ButtonSection` (`basics/button.tsx`) |
| | | `button-group` | `ButtonGroupSection` (`basics/button-group.tsx`) |
| | | `badge` | `BadgeSection` (`basics/badge.tsx`) |
| | | `card` | `CardSection` (`basics/card.tsx`) |
| | | `separator` | `SeparatorSection` (`basics/separator.tsx`) |
| | | `skeleton` | `SkeletonSection` (`basics/skeleton.tsx`) |
| | | `spinner` | `SpinnerSection` (`basics/spinner.tsx`) |
| | | `kbd` | `KbdSection` (`basics/kbd.tsx`) |
| | | `aspect-ratio` | `AspectRatioSection` (`basics/aspect-ratio.tsx`) |
| | | `empty` | `EmptySection` (`basics/empty.tsx`) |
| | | `attachment` | `AttachmentSection` (`basics/attachment.tsx`) |
| | | `avatar` | `AvatarSection` (`basics/avatar.tsx`) |
| | | `alert` | `AlertSection` (`basics/alert.tsx`) |
| **forms** | Forms (12) | `field` | `FieldSection` (`forms/field.tsx`) |
| | | `input` | `InputSection` (`forms/input.tsx`) |
| | | `input-group` | `InputGroupSection` (`forms/input-group.tsx`) |
| | | `select` | `SelectSection` (`forms/select.tsx`) |
| | | `checkbox` | `CheckboxSection` (`forms/checkbox.tsx`) |
| | | `radio-group` | `RadioGroupSection` (`forms/radio-group.tsx`) |
| | | `switch` | `SwitchSection` (`forms/switch.tsx`) |
| | | `slider` | `SliderSection` (`forms/slider.tsx`) |
| | | `textarea` | `TextareaSection` (`forms/textarea.tsx`) |
| | | `input-otp` | `InputOtpSection` (`forms/input-otp.tsx`) |
| | | `date-picker` | `DatePickerSection` (`forms/date-picker.tsx`) |
| | | `attachment-form` (或 `form-attachment`) | `AttachmentSection` (`forms/attachment.tsx`) |
| **overlays** | Overlays (8) | `dialog` | `DialogSection` (`overlays/dialog.tsx`) |
| | | `alert-dialog` | `AlertDialogSection` (`overlays/alert-dialog.tsx`) |
| | | `sheet` | `SheetSection` (`overlays/sheet.tsx`) |
| | | `drawer` | `DrawerSection` (`overlays/drawer.tsx`) |
| | | `popover` | `PopoverSection` (`overlays/popover.tsx`) |
| | | `hover-card` | `HoverCardSection` (`overlays/hover-card.tsx`) |
| | | `tooltip` | `TooltipSection` (`overlays/tooltip.tsx`) |
| | | `command` | `CommandSection` (`overlays/command.tsx`) |
| **navigation** | Navigation (9) | `tabs` | `TabsSection` (`navigation/tabs.tsx`) |
| | | `breadcrumb` | `BreadcrumbSection` (`navigation/breadcrumb.tsx`) |
| | | `pagination` | `PaginationSection` (`navigation/pagination.tsx`) |
| | | `menubar` | `MenubarSection` (`navigation/menubar.tsx`) |
| | | `navigation-menu` | `NavigationMenuSection` (`navigation/navigation-menu.tsx`) |
| | | `dropdown-menu` | `DropdownMenuSection` (`navigation/dropdown-menu.tsx`) |
| | | `context-menu` | `ContextMenuSection` (`navigation/context-menu.tsx`) |
| | | `scroll-area` | `ScrollAreaSection` (`navigation/scroll-area.tsx`) |
| | | `resizable` | `ResizableSection` (`navigation/resizable.tsx`) |
| **data** | Data (7) | `progress` | `ProgressSection` (`data/progress.tsx`) |
| | | `accordion` | `AccordionSection` (`data/accordion.tsx`) |
| | | `collapsible` | `CollapsibleSection` (`data/collapsible.tsx`) |
| | | `toggle` | `ToggleSection` (`data/toggle.tsx`) |
| | | `toggle-group` | `ToggleGroupSection` (`data/toggle-group.tsx`) |
| | | `table` | `TableSection` (`data/table.tsx`) |
| | | `sonner` | `SonnerSection` (`data/sonner.tsx`) |
| **blocks** | Blocks | `blocks` | `BlocksSection` (`blocks.tsx`) |
| **dashboard** | Dashboard | `dashboard` | `DashboardPreview` (`dashboard.tsx`) |
| **theme** | Theme | `theme` | `ThemeSection` (`theme.tsx`) |

*注：Basics(13) + Forms(12) + Overlays(8) + Navigation(9) + Data(7) = 49 个原子组件，加 3 个特别分类视图。*

---

## 3. URL Hash 路由规范与解析契约

### 3.1 路由格式规范

系统采用纯前端 **URL Hash** 路由（不依赖后端服务器 rewrite 配置），支持以下几种标准格式：

1. **单组件全路径格式**（推荐标准格式）：
   - 格式：`#<category>/<componentId>` 或 `#/category/componentId`
   - 示例：`#basics/button`、`#forms/input-otp`、`#navigation/dropdown-menu`
   - 行为：选中并仅渲染该组件的独立 Section。

2. **单组件简写别名**（兼容直达）：
   - 格式：`#<componentId>`
   - 示例：`#button`、`#tabs`、`#dialog`
   - 行为：根据全局组件注册表反查所属分类，自动归一化到对应的单组件视图。若存在同名组件（例如 basics 的 attachment 与 forms 的 attachment），优先匹配全路径；简写时按注册表首项或明确的别名（如 `forms-attachment`）区分。

3. **分类全览模式**（保留兼容）：
   - 格式：`#<category>` 或 `#<category>/all`
   - 示例：`#basics`、`#forms`、`#basics/all`、`#blocks`、`#dashboard`、`#theme`
   - 行为：渲染该分类下的全量组件列表（即原有的 All Section 模式）。

4. **缺省与容错回退**：
   - 缺省路由：`#` 或空 → 回退至 `#basics`（或 `#basics/button`）。推荐默认定向到 `#basics`。
   - 无法识别的 Hash → 回退至 `#basics` 并修正当前视图。

### 3.2 路由状态类型定义

```typescript
export type CategoryId =
  | "basics"
  | "forms"
  | "overlays"
  | "navigation"
  | "data"
  | "blocks"
  | "dashboard"
  | "theme"

export interface RouteState {
  category: CategoryId
  /**
   * undefined 或 "all" 代表分类全览视图；
   * 具体 componentId（如 "button"）代表单组件独立页面。
   */
  componentId?: string
}
```

### 3.3 路由解析与序列化算法

```typescript
export function parseHash(hash: string): RouteState {
  const clean = hash.replace(/^#\/?/, "").trim()
  if (!clean) {
    return { category: "basics" }
  }

  const parts = clean.split("/")
  if (parts.length === 2) {
    const [cat, comp] = parts
    if (isValidCategory(cat)) {
      if (comp === "all") return { category: cat }
      if (isValidComponent(cat, comp)) return { category: cat, componentId: comp }
    }
  }

  // 单段匹配：可能是 category 或者是 component 简写
  const single = parts[0]
  if (isValidCategory(single)) {
    return { category: single }
  }

  const found = findComponentById(single)
  if (found) {
    return { category: found.category, componentId: found.id }
  }

  return { category: "basics" }
}

export function stringifyRoute(route: RouteState): string {
  if (!route.componentId || route.componentId === "all") {
    return `#${route.category}`
  }
  return `#${route.category}/${route.componentId}`
}
```

### 3.4 路由监听与同步规则
- **`hashchange` 监听**：在 `App.tsx` 的 `useEffect` 中挂载 `window.addEventListener("hashchange", ...)`，并在组件卸载时移除。
- **状态驱动 URL 更新**：点击侧边栏菜单项或 Pagination 时，调用 `window.location.hash = stringifyRoute(newRoute)`，由 `hashchange` 统一分发状态，避免双向更新导致的事件死循环。
- **页面切换滚动复位**：路由变更且发生页面切换时，自动使主视口（或 contentRef）平滑或即时滚动至顶部 `window.scrollTo({ top: 0, behavior: "instant" })`。

---

## 4. 侧边栏 Collapsible 折叠数据结构与交互设计

### 4.1 数据结构接口

```typescript
export interface NavComponentItem {
  id: string
  title: string
  url: string // "#basics/button"
  category: CategoryId
}

export interface NavCategoryGroup {
  id: CategoryId
  title: string
  allUrl: string // "#basics"
  components: NavComponentItem[]
  collapsible: boolean // 是否支持折叠（如 basics/forms/overlays/navigation/data 为 true，blocks/dashboard/theme 为 false）
}

export interface SidebarNavSection {
  groupTitle: string // 如 "Getting Started", "Components", "Blocks & Views"
  categories: NavCategoryGroup[]
}
```

### 4.2 Collapsible 交互规范
- **默认展开逻辑**：
  - 当前活动组件所在的分类**自动展开**。
  - 用户可自由点击分类头部右侧的折叠箭头或分类标题展开/收起其他分类。
- **高亮与活动态（Active State）**：
  - **分类“All / 全览”**：当 `route.category === group.id && !route.componentId` 时高亮。
  - **子项组件**：当 `route.category === group.id && route.componentId === item.id` 时高亮（`isActive={true}`）。
- **组件层次结构展示**：
  ```
  ▼ Basics
     ├ 📄 All Basics (全览)
     ├ Button
     ├ Button Group
     ├ Badge
     ├ Card
     └ ...
  ▼ Forms
     ├ 📄 All Forms (全览)
     ├ Field
     ├ Input
     └ ...
  ▶ Overlays
  ▶ Navigation
  ▶ Data
  ──────────────────
  Blocks (直接链接)
  Dashboard (直接链接)
  Theme (直接链接)
  ```

---

## 5. Breadcrumb 与 OnThisPage TOC 适配规范

### 5.1 面包屑 (Breadcrumb) 联动契约

- **分类全览模式**：
  - 面包屑链路：`PartiSync Playground > {Category Title} (全览)`
  - 示例：`PartiSync Playground > Basics`
- **单组件页面模式**：
  - 面包屑链路：`PartiSync Playground > {Category Title} > {Component Title}`
  - 示例：`PartiSync Playground > Basics > Button`
  - 点击分类链接可跳转至该分类的全览页面（`#basics`）。

### 5.2 右侧 OnThisPage TOC 聚合机制

- **全览模式**：DOM 中包含所有 Section，`collectToc(contentRef.current)` 收集所有 Section 的 `h2` 与 Demo 的 `h3`。
- **单组件页面模式**：
  - 内容区仅渲染目标原子 Section（例如 `<ButtonSection />`）。
  - `collectToc` 会精准且自然地仅遍历该组件内的 `section[id]` 及所属 `h3[id]` Demo 锚点（如 `Default`、`Outline`、`Destructive`、`Ghost` 等）。
  - 完美解决以往 TOC 列表过长、滚动追踪跨组件错乱的痛点。

---

## 6. Pagination 跨组件顺序翻页规范

### 6.1 线性跳转序列设计

为了提供无缝顺畅的组件文档浏览体验，建立扁平的全局线性导航序列 `GLOBAL_NAV_ORDER`：
1. `basics` (全览)
2. `basics/button`
3. `basics/button-group`
4. ... (13 个 Basics 组件)
5. `forms` (全览)
6. `forms/field`
7. ... (12 个 Forms 组件)
8. `overlays` (全览)
9. ... (8 个 Overlays 组件)
10. `navigation` (全览)
11. ... (9 个 Navigation 组件)
12. `data` (全览)
13. ... (7 个 Data 组件)
14. `blocks`
15. `dashboard`
16. `theme`

### 6.2 翻页契约
- 在单组件视图底部，`Pagination` 自动计算上一项（`previous`）和下一项（`next`），包括：
  - 标题（Title，如 `← Button Group` / `Badge →`）
  - 目标 Hash（`href`）
  - 点击回调切换路由。
- 允许用户像翻阅组件手册一样持续使用底部 Next 浏览完所有 49 个组件。
