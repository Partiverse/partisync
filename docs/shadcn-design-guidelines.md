# shadcn/ui 设计规范与设计体系

> 本文档是对 [shadcn/ui 官方文档](https://ui.shadcn.com/) 的深度调研笔记，系统化整理其核心设计规范、CSS 变量体系、排版系统、主题机制及组件设计原则。所有规范均来自官方一手资料。
>
> **来源站点**：https://ui.shadcn.com/
> **调研时间**：2025年

---

## 目录

1. [概览与核心理念](#1-概览与核心理念)
2. [主题系统 Theming](#2-主题系统-theming)
3. [颜色规范 Colors](#3-颜色规范-colors)
4. [排版系统 Typography / Typeset](#4-排版系统-typography--typeset)
5. [布局规范 Layout](#5-布局规范-layout)
6. [组件系统 Components](#6-组件系统-components)
7. [页面级组件 Blocks](#7-页面级组件-blocks)
8. [无障碍 Accessibility](#8-无障碍-accessibility)
9. [设计细节规范](#9-设计细节规范)
10. [主题定制方法](#10-主题定制方法)

---

## 1. 概览与核心理念

**来源**：[Introduction - shadcn/ui](https://ui.shadcn.com/docs)

### 1.1 shadcn 是什么

shadcn/ui **不是一个传统意义上的组件库**，而是一个**代码分发系统**和**设计系统构建框架**。其核心理念是：

> "This is not a component library. It is how you build your component library."
> （这不是一个组件库，而是你构建自己组件库的方式。）

**与传统组件库的本质区别**：
| 传统组件库 | shadcn/ui |
|-----------|-----------|
| 从 NPM 安装包，import 即用 | 直接将组件代码**复制到你的项目**中 |
| 定制困难，需覆盖样式或包装组件 | **你拥有全部代码**，可随意修改 |
| 升级可能破坏自定义 | 组件代码在项目中，升级可控 |

**来源**：[Introduction](https://ui.shadcn.com/docs)

### 1.2 核心理念五原则

shadcn/ui 基于以下五个原则构建：

1. **Open Code（开放代码）**：组件源代码直接放在你的项目中，完全透明，可随意修改
2. **Composition（可组合性）**：所有组件共享统一的、可组合的接口，API 可预测
3. **Distribution（分发系统）**：通过 flat-file schema 和 CLI 工具分发组件代码
4. **Beautiful Defaults（精美默认值）**：精心设计的默认样式，开箱即用
5. **AI-Ready（AI 友好）**：开放代码便于 LLM 读取、理解甚至改进组件

### 1.3 技术栈

shadcn/ui 基于以下技术构建：

- **框架**：React（含 Next.js、RSC、TanStack 等集成）
- **样式**：Tailwind CSS
- **组件原语**：Radix UI（底层无样式组件）
- **样式组合**：CVA（Class Variance Authority）、clsx、Tailwind Merge
- **图标**：Lucide React
- **暗色模式**：next-themes（class 策略）

**来源**：[Introduction](https://ui.shadcn.com/docs)、[Theming](https://ui.shadcn.com/docs/theming)

### 1.4 components.json 配置

项目通过 `components.json` 文件配置：

```json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "new-york",
  "rsc": true,
  "tailwind": {
    "config": "",
    "css": "app/globals.css",
    "baseColor": "neutral",
    "cssVariables": true
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui",
    "lib": "@/lib",
    "hooks": "@/hooks"
  },
  "iconLibrary": "lucide"
}
```

**来源**：[components.json](https://ui.shadcn.com/docs/components-json)

---

## 2. 主题系统 Theming

**来源**：[Theming - shadcn/ui](https://ui.shadcn.com/docs/theming)

### 2.1 CSS 变量驱动的主题

shadcn/ui 使用 **CSS 变量**（推荐）或 Tailwind 工具类进行主题管理。

**工作原理**：
1. 在 `:root`（light）和 `.dark`（dark mode）选择器中定义 CSS 变量
2. Tailwind 将这些变量映射为工具类：`bg-primary`、`text-muted-foreground` 等
3. 组件使用这些工具类——修改变量即可全局改变外观

```tsx
// 使用 CSS 变量工具类
<div className="bg-background text-foreground" />
```

**来源**：[Theming](https://ui.shadcn.com/docs/theming)

### 2.2 Token 命名规范

shadcn/ui 采用简洁的 `background` / `foreground` 配对规范：

- **base token**（如 `primary`）：控制表面颜色
- **-foreground token**（如 `primary-foreground`）：控制该表面上文字和图标的颜色

> ⚠️ 注意：当变量用于组件背景色时，`background` 后缀被省略。例如 `primary` 配对 `primary-foreground`。

```css
/* 给定以下 CSS 变量 */
--primary: oklch(0.205 0 0);
--primary-foreground: oklch(0.985 0 0);

/* 以下组件的背景色为 var(--primary)，前景色为 var(--primary-foreground) */
<div className="bg-primary text-primary-foreground">Hello</div>
```

**来源**：[Theming](https://ui.shadcn.com/docs/theming)

### 2.3 OKLCH 色彩空间

**重要更新（Tailwind v4）**：shadcn/ui 将 **HSL 颜色转换为 OKLCH**。

OKLCH 的优势：
- 更符合人类视觉感知的色彩空间
- 支持更广的色域
- 亮度（Lightness）、色度（Chroma）、色相（Hue）分离，更易调整

格式：`oklch(L C H)`
- **L**：亮度，0-1（0 = 黑，1 = 白）
- **C**：色度，0 = 灰色，值越大颜色越鲜艳
- **H**：色相，0-360（色相角度）

```css
--primary: oklch(0.205 0 0); /* L=0.205, C=0(无色彩), H=0 */
--primary-foreground: oklch(0.985 0 0); /* L=0.985, C=0, H=0 */
```

**来源**：[Tailwind v4](https://ui.shadcn.com/docs/tailwind-v4)、[customization.md](https://github.com/shadcn-ui/ui/blob/main/skills/shadcn/customization.md)

### 2.4 完整 CSS 变量 Token 表

以下为 **New York 风格** 的完整默认 CSS 变量（Light 主题）：

#### Light 主题（`:root`）

```css
:root {
  /* 圆角 - New York: 0.625rem (10px) */
  --radius: 0.625rem;

  /* 背景与前景 */
  --background: oklch(1 0 0);
  --foreground: oklch(0.145 0 0);

  /* 卡片 */
  --card: oklch(1 0 0);
  --card-foreground: oklch(0.145 0 0);

  /* 弹出层 Popover */
  --popover: oklch(1 0 0);
  --popover-foreground: oklch(0.145 0 0);

  /* 主色 Primary */
  --primary: oklch(0.205 0 0);
  --primary-foreground: oklch(0.985 0 0);

  /* 次要色 Secondary */
  --secondary: oklch(0.97 0 0);
  --secondary-foreground: oklch(0.205 0 0);

  /* 静音色 Muted */
  --muted: oklch(0.97 0 0);
  --muted-foreground: oklch(0.556 0 0);

  /* 强调色 Accent */
  --accent: oklch(0.97 0 0);
  --accent-foreground: oklch(0.205 0 0);

  /* 破坏性色 Destructive */
  --destructive: oklch(0.577 0.245 27.325);
  --destructive-foreground: oklch(0.985 0 0);

  /* 边框与输入 */
  --border: oklch(0.922 0 0);
  --input: oklch(0.922 0 0);
  --ring: oklch(0.708 0 0);

  /* 图表颜色 */
  --chart-1: oklch(0.646 0.222 41.116);
  --chart-2: oklch(0.6 0.118 184.704);
  --chart-3: oklch(0.398 0.07 227.392);
  --chart-4: oklch(0.828 0.189 84.429);
  --chart-5: oklch(0.769 0.188 70.08);

  /* 侧边栏 */
  --sidebar: oklch(0.985 0 0);
  --sidebar-foreground: oklch(0.145 0 0);
  --sidebar-primary: oklch(0.205 0 0);
  --sidebar-primary-foreground: oklch(0.985 0 0);
  --sidebar-accent: oklch(0.97 0 0);
  --sidebar-accent-foreground: oklch(0.205 0 0);
  --sidebar-border: oklch(0.922 0 0);
  --sidebar-ring: oklch(0.708 0 0);

  /* 次要表面 */
  --surface: oklch(0.97 0 0);
  --surface-foreground: oklch(0.205 0 0);
}
```

#### Dark 主题（`.dark`）

```css
.dark {
  /* 背景与前景 */
  --background: oklch(0.145 0 0);
  --foreground: oklch(0.985 0 0);

  /* 卡片 */
  --card: oklch(0.205 0 0);
  --card-foreground: oklch(0.985 0 0);

  /* 弹出层 Popover */
  --popover: oklch(0.269 0 0);
  --popover-foreground: oklch(0.985 0 0);

  /* 主色 Primary */
  --primary: oklch(0.922 0 0);
  --primary-foreground: oklch(0.205 0 0);

  /* 次要色 Secondary */
  --secondary: oklch(0.269 0 0);
  --secondary-foreground: oklch(0.985 0 0);

  /* 静音色 Muted */
  --muted: oklch(0.269 0 0);
  --muted-foreground: oklch(0.708 0 0);

  /* 强调色 Accent */
  --accent: oklch(0.371 0 0);
  --accent-foreground: oklch(0.985 0 0);

  /* 破坏性色 Destructive */
  --destructive: oklch(0.704 0.191 22.216);
  --destructive-foreground: oklch(0.985 0 0);

  /* 边框与输入 */
  --border: oklch(1 0 0 / 10%);
  --input: oklch(1 0 0 / 15%);
  --ring: oklch(0.556 0 0);

  /* 图表颜色 */
  --chart-1: oklch(0.488 0.243 264.376);
  --chart-2: oklch(0.696 0.17 162.48);
  --chart-3: oklch(0.769 0.188 70.08);
  --chart-4: oklch(0.627 0.265 303.9);
  --chart-5: oklch(0.645 0.246 16.439);

  /* 侧边栏 */
  --sidebar: oklch(0.205 0 0);
  --sidebar-foreground: oklch(0.985 0 0);
  --sidebar-primary: oklch(0.488 0.243 264.376);
  --sidebar-primary-foreground: oklch(0.985 0 0);
  --sidebar-accent: oklch(0.269 0 0);
  --sidebar-accent-foreground: oklch(0.985 0 0);
  --sidebar-border: oklch(1 0 0 / 10%);
  --sidebar-ring: oklch(0.439 0 0);

  /* 次要表面 */
  --surface: oklch(0.269 0 0);
  --surface-foreground: oklch(0.985 0 0);
}
```

**来源**：[Theming](https://ui.shadcn.com/docs/theming)、[customization.md](https://github.com/shadcn-ui/ui/blob/main/skills/shadcn/customization.md)

### 2.5 New York vs Default 风格差异

shadcn/ui 有两种内置风格：**New York**（当前默认）和 **Default**（已弃用）。

| 特性 | New York | Default |
|------|----------|---------|
| **状态** | 当前默认，推荐使用 | 已弃用 |
| **圆角（--radius）** | `0.625rem` (10px) | `0.5rem` (8px) |
| **风格特点** | 更圆润、现代感更强 | 较方、传统 |
| **阴影** | 可能有轻微阴影变化 | 较平的卡片样式 |

> ⚠️ `default` 风格已被弃用，新项目应使用 `new-york` 风格。

**来源**：[components.json](https://ui.shadcn.com/docs/components-json)、[Tailwind v4](https://ui.shadcn.com/docs/tailwind-v4)

### 2.6 圆角系统（Radius Scale）

`--radius` 是全局圆角控制变量，组件从中派生具体值：

| Tailwind 类 | 对应值 |
|-------------|--------|
| `rounded-none` | `0` |
| `rounded-sm` | `calc(var(--radius) - 4px)` |
| `rounded-md` | `calc(var(--radius) - 2px)` |
| `rounded-lg` | `var(--radius)` |
| `rounded-xl` | `calc(var(--radius) + 2px)` |
| `rounded-2xl` | `calc(var(--radius) + 4px)` |
| `rounded-full` | `9999px` |

**New York 风格**：`--radius: 0.625rem`

**来源**：[customization.md](https://github.com/shadcn-ui/ui/blob/main/skills/shadcn/customization.md)

### 2.7 暗色模式实现

基于 **class 策略**的暗色模式，通过在根元素上添加 `.dark` 类来切换：

```tsx
// next-themes 配置
import { ThemeProvider } from "next-themes";

<ThemeProvider attribute="class" defaultTheme="system" enableSystem>
  {children}
</ThemeProvider>
```

原理：在 `.dark` 选择器中覆盖相同的 CSS 变量 token，实现主题切换。

**来源**：[Dark Mode](https://ui.shadcn.com/docs/dark-mode)、[customization.md](https://github.com/shadcn-ui/ui/blob/main/skills/shadcn/customization.md)

---

## 3. 颜色规范 Colors

**来源**：[Theming - shadcn/ui](https://ui.shadcn.com/docs/theming)

### 3.1 语义颜色 Token

shadcn/ui 定义了一套完整的语义化颜色 token：

| Token | 用途 | Light 默认值 (OKLCH) | Dark 默认值 (OKLCH) |
|-------|------|---------------------|---------------------|
| `--background` | 页面背景 | `1 0 0` | `0.145 0 0` |
| `--foreground` | 默认文字 | `0.145 0 0` | `0.985 0 0` |
| `--card` | 卡片表面 | `1 0 0` | `0.205 0 0` |
| `--card-foreground` | 卡片文字 | `0.145 0 0` | `0.985 0 0` |
| `--popover` | 弹出层 | `1 0 0` | `0.269 0 0` |
| `--popover-foreground` | 弹出层文字 | `0.145 0 0` | `0.985 0 0` |
| `--primary` | 主按钮/主要操作 | `0.205 0 0` | `0.922 0 0` |
| `--primary-foreground` | 主色上的文字 | `0.985 0 0` | `0.205 0 0` |
| `--secondary` | 次要操作 | `0.97 0 0` | `0.269 0 0` |
| `--secondary-foreground` | 次要色上的文字 | `0.205 0 0` | `0.985 0 0` |
| `--muted` | 静音/禁用状态 | `0.97 0 0` | `0.269 0 0` |
| `--muted-foreground` | 静音文字 | `0.556 0 0` | `0.708 0 0` |
| `--accent` | 悬停/强调状态 | `0.97 0 0` | `0.371 0 0` |
| `--accent-foreground` | 强调色上的文字 | `0.205 0 0` | `0.985 0 0` |
| `--destructive` | 错误/破坏性操作 | `0.577 0.245 27.325` | `0.704 0.191 22.216` |
| `--destructive-foreground` | 破坏性色上的文字 | `0.985 0 0` | `0.985 0 0` |
| `--border` | 默认边框颜色 | `0.922 0 0` | `1 0 0 / 10%` |
| `--input` | 表单输入边框 | `0.922 0 0` | `1 0 0 / 15%` |
| `--ring` | 焦点环颜色 | `0.708 0 0` | `0.556 0 0` |
| `--surface` | 次要表面 | `0.97 0 0` | `0.269 0 0` |
| `--surface-foreground` | 表面文字 | `0.205 0 0` | `0.985 0 0` |

### 3.2 图表颜色

| Token | 用途 | Light 默认值 (OKLCH) |
|-------|------|---------------------|
| `--chart-1` | 图表数据系列 1 | `0.646 0.222 41.116` |
| `--chart-2` | 图表数据系列 2 | `0.6 0.118 184.704` |
| `--chart-3` | 图表数据系列 3 | `0.398 0.07 227.392` |
| `--chart-4` | 图表数据系列 4 | `0.828 0.189 84.429` |
| `--chart-5` | 图表数据系列 5 | `0.769 0.188 70.08` |

### 3.3 侧边栏颜色

| Token | 用途 |
|-------|------|
| `--sidebar` | 侧边栏背景 |
| `--sidebar-foreground` | 侧边栏默认文字 |
| `--sidebar-primary` | 侧边栏主色 |
| `--sidebar-primary-foreground` | 侧边栏主色上的文字 |
| `--sidebar-accent` | 侧边栏强调色 |
| `--sidebar-accent-foreground` | 侧边栏强调色上的文字 |
| `--sidebar-border` | 侧边栏边框 |
| `--sidebar-ring` | 侧边栏焦点环 |

### 3.4 Tailwind 工具类映射

CSS 变量映射到以下 Tailwind 工具类：

| CSS 变量 | Tailwind 工具类 |
|----------|-----------------|
| `--background` | `bg-background`, `text-background` |
| `--foreground` | `bg-foreground`, `text-foreground` |
| `--border` | `border-border`, `bg-border` |
| `--input` | `border-input`, `bg-input` |
| `--ring` | `ring-ring`, `bg-ring` |
| `--primary` | `bg-primary`, `text-primary` |
| `--secondary` | `bg-secondary`, `text-secondary` |
| `--muted` | `bg-muted`, `text-muted` |
| `--accent` | `bg-accent`, `text-accent` |
| `--destructive` | `bg-destructive`, `text-destructive` |

**来源**：[Theming](https://ui.shadcn.com/docs/theming)、[customization.md](https://github.com/shadcn-ui/ui/blob/main/skills/shadcn/customization.md)

---

## 4. 排版系统 Typography / Typeset

**来源**：[Typeset - shadcn/ui](https://ui.shadcn.com/docs/typeset)

### 4.1 Typeset 概念

shadcn/ui 提供了 **shadcn/typeset** —— 一个用于渲染 HTML 和 Markdown 的排版系统，通过一个 CSS 文件样式化所有内容。

> "One CSS file you own."

**核心理念**：将排版压缩为**三个控制变量**：
1. **size**（字号）
2. **leading**（行高）
3. **flow**（段落间距）

其他所有元素（标题尺寸、列表缩进、标题下方间距等）都从这三个变量派生。

### 4.2 Typeset CSS 变量

```css
.typeset-docs {
  --typeset-font-body: var(--font-geist);
  --typeset-font-heading: var(--font-geist);
  --typeset-font-mono: var(--font-geist-mono);
  --typeset-size: 15px;
  --typeset-leading: 1.75;
  --typeset-flow: 1.25em;
}
```

### 4.3 字体系统

**默认字体**：
- **Body 字体**：Geist Sans（通过 `var(--font-geist)`）
- **Heading 字体**：Geist Sans（可自定义）
- **Mono 字体**：Geist Mono（通过 `var(--font-geist-mono)`）

**字体工具类**：

| 工具类 | 用途 |
|--------|------|
| `font-sans` | 无衬线字体栈 |
| `font-serif` | 衬线字体栈 |
| `font-mono` | 等宽字体栈 |

### 4.4 Typeset 特性

- **自适应容器**：放在聊天气泡中跟随小字号，放在文章中跟随大字号，小屏幕自动增加字号
- **主题集成**：颜色、字体、圆角来自你的应用主题，暗色模式遵循相同 token
- **Markdown 支持**：全面支持博客、文档、聊天等场景的渲染

**来源**：[Typeset](https://ui.shadcn.com/docs/typeset)

### 4.5 字体栈（Font Stack）

shadcn/ui 默认使用 Geist 字体系列。字体通过 CSS 变量引入：

```css
--font-geist: Geist, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
--font-geist-mono: GeistMono, 'SF Mono', Monaco, 'Cascadia Mono', 'Segoe UI Mono', monospace;
```

---

## 5. 布局规范 Layout

**来源**：[Introduction](https://ui.shadcn.com/docs)、[components](https://ui.shadcn.com/docs/components)

### 5.1 容器宽度

shadcn/ui 使用 Tailwind 的容器系统：

```css
.container {
  width: 100%;
  margin-left: auto;
  margin-right: auto;
  padding-left: 1rem;
  padding-right: 1rem;
}

/* 大屏幕增加内边距 */
@media (min-width: 640px) {
  .container {
    padding-left: 1.5rem;
    padding-right: 1.5rem;
  }
}
```

### 5.2 页面留白

| 断点 | 内边距 |
|------|--------|
| 默认（移动端） | `1rem` (16px) |
| `sm` (640px+) | `1.5rem` (24px) |
| `lg` (1024px+) | `2rem` (32px) |

### 5.3 组件间距

组件使用 4px 基准网格系统（Tailwind 默认）：

| Token | 值 | 用途 |
|-------|-----|------|
| `1` | `0.25rem` (4px) | 紧凑间距 |
| `2` | `0.5rem` (8px) | 小间距 |
| `3` | `0.75rem` (12px) | 中等间距 |
| `4` | `1rem` (16px) | 标准间距 |
| `6` | `1.5rem` (24px) | 大间距 |
| `8` | `2rem` (32px) | 特大间距 |

**来源**：[Tailwind CSS 官方间距规范](https://tailwindcss.com/docs spacing)

---

## 6. 组件系统 Components

**来源**：[Components](https://ui.shadcn.com/docs/components)

### 6.1 组件架构

shadcn/ui 组件基于以下架构：

1. **Radix UI 原语**：底层无样式的可访问组件
2. **样式层**：通过 Tailwind CSS 和 CSS 变量添加样式
3. **CVA（Class Variance Authority）**：管理组件变体

```
Radix UI 原语 → shadcn 样式 → CVA 变体 → 最终组件
```

### 6.2 组件列表

shadcn/ui 提供 70+ 组件，包括：

**基础组件**：
- Accordion、Alert、Alert Dialog、Aspect Ratio、Avatar、Badge、Breadcrumb、Button、Button Group、Calendar、Card、Carousel、Chart、Checkbox、Collapsible、Combobox、Command、Context Menu、Data Table、Date Picker、Dialog、Direction、Drawer、Dropdown Menu、Empty、Field、Hover Card、Input、Input Group、Input OTP、Item、Kbd、Label、Marker、Menubar、Message、Message Scroller、Native Select、Navigation Menu、Pagination、Popover、Progress、Questionnaire、Radio Group、Resizable、Scroll Area、Select、Separator、Sheet、Sidebar、Skeleton、Slider、Spinner、Switch、Table、Tabs、Textarea、Toast、Toggle、Toggle Group、Tooltip、Typography

### 6.3 Button 组件详解

**来源**：[Button - shadcn/ui](https://ui.shadcn.com/docs/components/button)

#### 变体（Variants）

| 变体 | 说明 | 样式特点 |
|------|------|----------|
| `default` | 默认按钮 | `bg-primary text-primary-foreground hover:bg-primary/90` |
| `destructive` | 破坏性操作 | `bg-destructive text-destructive-foreground hover:bg-destructive/90` |
| `outline` | 轮廓按钮 | `border border-input bg-background hover:bg-accent hover:text-accent-foreground` |
| `secondary` | 次要操作 | `bg-secondary text-secondary-foreground hover:bg-secondary/80` |
| `ghost` | 幽灵按钮 | `hover:bg-accent hover:text-accent-foreground` |
| `link` | 链接样式 | `text-primary underline-offset-4 hover:underline` |

#### 尺寸（Sizes）

| 尺寸 | 类 | 圆角 | 内边距 | 字体大小 |
|------|-----|------|--------|----------|
| `default` | `h-10 px-4 py-2` | `rounded-lg` | 水平 16px，垂直 8px | 14px |
| `sm` | `h-9 px-3` | `rounded-md` | 水平 12px | 14px |
| `lg` | `h-11 px-8` | `rounded-lg` | 水平 32px | 16px |
| `icon` | `h-10 w-10` | `rounded-lg` | 40x40px | - |

#### Cursor 行为（Tailwind v4 更新）

> ⚠️ Tailwind v4 将 button 的 cursor 从 `cursor: pointer` 改为 `cursor: default`。

如需保持 `cursor: pointer` 行为，在 CSS 中添加：

```css
@layer base {
  button:not(:disabled),
  [role="button"]:not(:disabled) {
    cursor: pointer;
  }
}
```

或使用 CLI 初始化：`npx shadcn@latest init --pointer`

**来源**：[Button](https://ui.shadcn.com/docs/components/button)

### 6.4 组件设计结构

#### 样式分离原则

shadcn/ui 遵循**样式与逻辑分离**原则：
- **逻辑层**：Radix UI 提供无障碍和行为逻辑
- **样式层**：Tailwind CSS 类提供视觉样式
- **变体管理**：CVA 管理组件变体

#### Focus Ring

组件使用统一的 focus ring 样式：

```css
/* 默认 focus ring */
focus-visible:ring-ring/50

/* 在 globals.css 中定义 */
--ring: oklch(0.708 0 0);
```

#### Shadow DOM

shadcn/ui 组件**不使用 Shadow DOM**，而是直接在你的项目 DOM 中渲染，便于样式覆盖和定制。

### 6.5 圆角规范

所有组件使用基于 `--radius` 变量的圆角系统：

```css
/* 基础圆角类 */
.rounded-none   { border-radius: 0; }
.rounded-sm     { border-radius: calc(var(--radius) - 4px); }
.rounded-md     { border-radius: calc(var(--radius) - 2px); }
.rounded-lg     { border-radius: var(--radius); }
.rounded-xl     { border-radius: calc(var(--radius) + 2px); }
.rounded-2xl    { border-radius: calc(var(--radius) + 4px); }
.rounded-full   { border-radius: 9999px; }
```

**New York 风格**：`--radius: 0.625rem`（10px）
- `rounded-lg` = `0.625rem`
- `rounded-md` = `0.5rem` (8px)

**来源**：[customization.md](https://github.com/shadcn-ui/ui/blob/main/skills/shadcn/customization.md)

### 6.6 自定义组件变体（CVA）

使用 Class Variance Authority 管理变体：

```tsx
// components/ui/button.tsx
import { cva, type VariantProps } from "class-variance-authority";

const buttonVariants = cva(
  "inline-flex items-center justify-center whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground shadow hover:bg-primary/90",
        destructive: "bg-destructive text-destructive-foreground shadow-sm hover:bg-destructive/90",
        outline: "border border-input bg-background shadow-sm hover:bg-accent hover:text-accent-foreground",
        secondary: "bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/80",
        ghost: "hover:bg-accent hover:text-accent-foreground",
        link: "text-primary underline-offset-4 hover:underline",
      },
      size: {
        default: "h-9 px-4 py-2",
        sm: "h-8 rounded-md px-3 text-xs",
        lg: "h-10 rounded-md px-8",
        icon: "h-9 w-9",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
);
```

**来源**：[Button](https://ui.shadcn.com/docs/components/button)

---

## 7. 页面级组件 Blocks

**来源**：[Blocks - shadcn/ui](https://ui.shadcn.com/blocks)

### 7.1 Blocks 概念

Blocks 是**页面级组件**，由多个 shadcn/ui 基础组件组合而成，用于快速构建完整页面。

### 7.2 可用 Blocks

| Block 名称 | 说明 |
|-----------|------|
| `dashboard-01` | 带侧边栏、图表和数据表的仪表板 |
| `sidebar` | 可折叠侧边栏布局 |
| `login` | 登录页面 |
| `signup` | 注册页面 |
| `cards` | 卡片集合 |
| `tables` | 数据表页面 |
| `forms` | 表单页面 |
| 等 | 更多Blocks持续更新 |

### 7.3 使用 Blocks

```bash
# 添加特定 Block
npx shadcn add dashboard-01

# 查看所有可用 Blocks
npx shadcn@latest blocks
```

### 7.4 Blocks 特点

- **可复制粘贴**：直接复制到你的应用中
- **框架无关**：适用于所有 React 框架
- **主题集成**：自动使用你的主题配置
- **开源免费**：MIT 许可证

**来源**：[Blocks](https://ui.shadcn.com/blocks)

---

## 8. 无障碍 Accessibility

**来源**：[Components](https://ui.shadcn.com/docs/components)

### 8.1 基于 Radix UI 的无障碍

shadcn/ui 组件底层使用 **Radix UI** 原语，因此继承了一流的无障碍支持：

- **WAI-ARIA 模式**：所有组件实现适当的 ARIA 角色和属性
- **键盘导航**：完整的键盘操作支持
- **屏幕阅读器支持**：语义化 HTML 和 aria 属性
- **焦点管理**：模态框、对话框等组件正确管理焦点

### 8.2 无障碍最佳实践

1. **使用语义化 HTML**：优先使用原生语义元素
2. **提供替代文本**：图像和图标应有替代文本
3. **确保颜色对比度**：文本与背景符合 WCAG 标准
4. **支持键盘导航**：所有交互可通过键盘完成
5. **焦点可见性**：使用 focus ring 指示焦点位置

### 8.3 Focus Ring 样式

统一使用 `--ring` CSS 变量：

```css
/* 默认样式 */
focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2

/* 禁用状态 */
disabled:opacity-50 disabled:pointer-events-none
```

### 8.4 aria-invalid 和 aria 属性

组件正确使用 aria 属性：

```tsx
// 示例：带有验证的输入框
<input
  aria-invalid="true"
  className="aria-invalid:border-destructive"
/>
```

**来源**：[Button](https://ui.shadcn.com/docs/components/button)

---

## 9. 设计细节规范

**来源**：[Theming](https://ui.shadcn.com/docs/theming)、[Button](https://ui.shadcn.com/docs/components/button)

### 9.1 阴影规范

shadcn/ui 使用轻量阴影，主要用于卡片和弹出层：

| 阴影类 | 用途 |
|--------|------|
| `shadow-sm` | 小的提升感，用于次要元素 |
| `shadow` | 标准阴影，用于卡片和面板 |
| 无阴影 | 默认/扁平元素 |

**注意**：New York 风格使用更现代的轻微阴影设计。

### 9.2 过渡/动画规范

组件使用标准过渡时间：

```css
/* 默认过渡 */
transition-colors duration-200 ease-in-out

/* 快速过渡 */
transition-all duration-150

/* 弹出层动画（Radix UI 提供）*/
data-[state=open]:animate-in data-[state=closed]:animate-out
```

**动画时长**：
- 快速交互：`150ms`
- 标准过渡：`200ms`
- 复杂动画：`300ms`

### 9.3 Focus-Visible 样式

仅在键盘导航时显示焦点环：

```css
/* 使用 focus-visible 而非 focus */
focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2
```

### 9.4 禁用状态

```css
disabled:opacity-50 disabled:pointer-events-none
```

### 9.5 组件状态速查表

| 状态 | 使用的类 |
|------|----------|
| Hover | `hover:` 前缀 |
| Active | `active:` 前缀 |
| Focus | `focus-visible:` 前缀 |
| Disabled | `disabled:` 前缀 |
| Loading | `aria-busy="true"` |
| Selected | `aria-selected="true"` |
| Expanded | `aria-expanded="true"` |

---

## 10. 主题定制方法

**来源**：[Theming](https://ui.shadcn.com/docs/theming)、[customization.md](https://github.com/shadcn-ui/ui/blob/main/skills/shadcn/customization.md)

### 10.1 修改主色

#### 方法 1：直接编辑 CSS 变量

在 `globals.css` 中修改变量：

```css
:root {
  /* 将主色改为蓝色 */
  --primary: oklch(0.55 0.2 250);
  --primary-foreground: oklch(0.98 0 0);
}
```

#### 方法 2：使用预设

```bash
# 应用预设
npx shadcn@latest apply --preset a2r6bw

# 使用位置简写
npx shadcn@latest apply a2r6bw

# 切换到命名预设
npx shadcn@latest apply --preset nova

# 使用自定义主题 URL
npx shadcn@latest apply --preset "https://ui.shadcn.com/init?base=radix&style=nova&theme=blue&..."
```

### 10.2 添加自定义颜色

#### 步骤 1：在 CSS 中定义

```css
:root {
  --warning: oklch(0.84 0.16 84);
  --warning-foreground: oklch(0.28 0.07 46);
}
.dark {
  --warning: oklch(0.41 0.11 46);
  --warning-foreground: oklch(0.99 0.02 95);
}
```

#### 步骤 2a：Tailwind v4 注册（@theme inline）

```css
@theme inline {
  --color-warning: var(--warning);
  --color-warning-foreground: var(--warning-foreground);
}
```

#### 步骤 2b：Tailwind v3 注册（tailwind.config.js）

```js
module.exports = {
  theme: {
    extend: {
      colors: {
        warning: "oklch(var(--warning) / <alpha-value>)",
        "warning-foreground": "oklch(var(--warning-foreground) / <alpha-value>)",
      },
    },
  },
};
```

#### 步骤 3：在组件中使用

```tsx
<div className="bg-warning text-warning-foreground">Warning</div>
```

### 10.3 暗色模式定制

#### 配置 next-themes

```tsx
// _app.tsx 或 providers.tsx
import { ThemeProvider } from "next-themes";

export function Providers({ children }: { children: React.ReactNode }) {
  return (
    <ThemeProvider attribute="class" defaultTheme="system" enableSystem>
      {children}
    </ThemeProvider>
  );
}
```

#### 手动切换

```tsx
import { useTheme } from "next-themes";
import { Moon, Sun } from "lucide-react";

export function ThemeToggle() {
  const { theme, setTheme } = useTheme();

  return (
    <button onClick={() => setTheme(theme === "dark" ? "light" : "dark")}>
      {theme === "dark" ? <Moon /> : <Sun />}
    </button>
  );
}
```

### 10.4 圆角定制

修改 `--radius` 变量：

```css
:root {
  /* 更圆润 */
  --radius: 1rem;

  /* 更方正 */
  --radius: 0.25rem;
}
```

### 10.5 字体定制

#### 添加 Google Fonts

```tsx
// app/layout.tsx
import { Inter } from "next/font/google";

const inter = Inter({
  subsets: ["latin"],
  variable: "--font-inter",
});

export default function RootLayout({ children }) {
  return (
    <html lang="en" className={inter.variable}>
      <body>{children}</body>
    </html>
  );
}
```

#### 在 CSS 中使用

```css
:root {
  --font-sans: var(--font-inter), system-ui, sans-serif;
}
```

### 10.6 Tailwind v4 迁移

**来源**：[Tailwind v4](https://ui.shadcn.com/docs/tailwind-v4)

shadcn/ui 现在支持 Tailwind v4，主要变化：

1. **HSL → OKLCH**：颜色现在使用 OKLCH 格式
2. **@theme 指令**：使用 `@theme inline` 替代 `tailwind.config.js`
3. **移除 forwardRef**：组件不再使用 forwardRef
4. **data-slot 属性**：所有原语现在有 `data-slot` 属性
5. **默认 cursor 改变**：button 使用 `cursor: default`

**迁移步骤**：

```bash
# 1. 升级到 Tailwind v4
# 参考官方升级指南：https://tailwindcss.com/docs/upgrade-guide

# 2. 使用 codemod
npx @tailwindcss/upgrade@next

# 3. 更新 CSS 变量（HSL → OKLCH）

# 4. 更新图表颜色

# 5. 使用新的 size-* 工具类

# 6. 移除 forwardRef（如果使用）
```

---

## 来源 URL 列表

以下为本调研访问的所有官方文档页面：

| # | 页面 | URL |
|---|------|-----|
| 1 | 首页/概览 | https://ui.shadcn.com/ |
| 2 | Introduction | https://ui.shadcn.com/docs |
| 3 | Theming | https://ui.shadcn.com/docs/theming |
| 4 | Dark Mode | https://ui.shadcn.com/docs/dark-mode |
| 5 | Typeset | https://ui.shadcn.com/docs/typeset |
| 6 | Components (Button) | https://ui.shadcn.com/docs/components/button |
| 7 | components.json | https://ui.shadcn.com/docs/components-json |
| 8 | Tailwind v4 | https://ui.shadcn.com/docs/tailwind-v4 |
| 9 | Blocks | https://ui.shadcn.com/blocks |
| 10 | GitHub customization.md | https://github.com/shadcn-ui/ui/blob/main/skills/shadcn/customization.md |

---

## 附录：关键规范速查

### CSS 变量速查

| 变量 | Light | Dark | 用途 |
|------|-------|------|------|
| `--background` | `oklch(1 0 0)` | `oklch(0.145 0 0)` | 页面背景 |
| `--foreground` | `oklch(0.145 0 0)` | `oklch(0.985 0 0)` | 默认文字 |
| `--primary` | `oklch(0.205 0 0)` | `oklch(0.922 0 0)` | 主色 |
| `--radius` | `0.625rem` | - | 圆角 (New York) |

### Tailwind 工具类速查

| 工具类 | CSS 变量 |
|--------|----------|
| `bg-background` | `--background` |
| `text-foreground` | `--foreground` |
| `bg-primary` | `--primary` |
| `text-primary-foreground` | `--primary-foreground` |
| `border-border` | `--border` |
| `ring-ring` | `--ring` |

### 组件变体速查（Button）

| Variant | 用途 |
|---------|------|
| `default` | 主要操作 |
| `destructive` | 危险操作 |
| `outline` | 次要操作 |
| `secondary` | 次次要操作 |
| `ghost` | 最小强调 |
| `link` | 链接样式 |

---

*文档版本：1.0*
*最后更新：2025年*
*来源：https://ui.shadcn.com/ 官方文档*
