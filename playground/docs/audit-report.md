# Playground 组件审计报告（对照 ui.shadcn.com）

审计范围：`playground/src/components/ui/*`（59 个）、`playground/src/showcase/*`
参考：https://ui.shadcn.com/docs/components

## 组件审计报告

### 差异组件列表

| 组件 | 官网有 | 当前状态 | 问题 |
|------|--------|---------|------|
| button.tsx | ✓ | 6 variants + 8 sizes | 与官网差异明显（见下） |
| button-group | ✓ | 存在 | 官方组件，保留 |
| field / input-group / empty / kbd / spinner / input-otp | ✓ | 存在 | 官方新组件，保留 |
| direction.tsx | ✓（Direction Provider） | 仅 6 行 re-export | 可保留，但过度薄 |
| attachment.tsx | ✗ | 自定义 207 行 | 非官方组件 |
| bubble.tsx | ✗ | 自定义 50 行 | 非官方组件 |
| marker.tsx | ✗ | 自定义（Radix Slot + cva） | 非官方组件 |
| message.tsx | ✗ | 自定义 106 行 | 非官方组件 |
| message-scroller.tsx | ✗ | 自定义 161 行 | 非官方组件 |
| questionnaire.tsx | ✗ | 305 行，依赖 `@shadcn/react/questionnaire`（可疑依赖） | 非官方组件 |
| date-picker.tsx | △ | 官网仅作为 Calendar+Popover 组合示例，非 ui 原语 | 可降级为 showcase 示例 |
| item.tsx / form.tsx | ✓ 官网有 | **缺失** | 官网有而本地没有 |

其余 accordion / alert / alert-dialog / avatar / badge / breadcrumb / calendar / card / carousel / chart / checkbox / collapsible / command / context-menu / dialog / drawer / dropdown-menu / hover-card / input / label / menubar / navigation-menu / pagination / popover / progress / radio-group / resizable / scroll-area / select / separator / sheet / sidebar / skeleton / slider / sonner / switch / table / tabs / textarea / toggle / toggle-group / tooltip / native-select / aspect-ratio 均为官方组件，API 齐全，保留。

### Button 部分问题

`components/ui/button.tsx`（基于 `@base-ui/react/button`，官网基于 Radix Slot）：

1. **变体数与官网一致（default/outline/secondary/ghost/destructive/link），无多余变体** —— 不算问题。
2. **尺寸过度扩展**：官网只有 `default / sm / lg / icon`；本项目新增 `xs / icon-xs / icon-sm / icon-lg`，共 8 个 size。xs/icon-xs 是额外添加。
3. **样式类与官网不一致**：
   - `rounded-lg`（官网 `rounded-md`）；hover 用 `hover:bg-primary/80`（官网 `bg-primary/90`）；
   - secondary hover 用 `color-mix(in oklch,…)` 私有技巧；destructive 采用软色 `bg-destructive/10`（官网为实色 `bg-destructive`）；
   - `active:translate-y-px` 按压位移为自定义动效，官网无。
4. **showcase/basics/button.tsx 展示 11 个 Demo**：default/outline/secondary/ghost/destructive/link、7 Sizes+Icon-only、icon-only with label、with icon、Rounded、spinner(loading)。
   - 官网 Button 页示例为：默认、Outline、Secondary、Ghost、Destructive、Link、With Icon、Loading、按钮组等。
   - 「7 Sizes + Icon-only」「icon-only with label」是**额外添加**（xs/icon-xs/icon-sm/icon-lg 为本地自定义 size）。
   - **重复**：loading Demo 中第二个按钮手写内联 SVG spinner，与第一个用的 `<Spinner>` 组件功能重复，应统一用 Spinner。
   - Rounded Demo 通过 `className="rounded-full"` 覆盖，属样式覆写式展示，非官方 API。
5. 重复/类似组件：Spinner 组件与内联 SVG spinner 两套实现并存。

### 建议删除的组件

| 组件 | 原因 |
|------|------|
| ui/attachment.tsx | 非官方，207 行自定义；如需保留应移到 showcase/demo 而非 ui |
| ui/bubble.tsx | 非官方聊天气泡 |
| ui/marker.tsx | 非官方标记组件 |
| ui/message.tsx | 非官方消息组件（与 bubble 功能重叠） |
| ui/message-scroller.tsx | 非官方，且依赖 message |
| ui/questionnaire.tsx | 非官方，且依赖来源可疑的 `@shadcn/react/questionnaire` |
| ui/date-picker.tsx | 非官方 ui 原语；官网定位是 Calendar+Popover 组合示例，可移到 showcase/forms |
| showcase 内上述组件对应 demo（basics/attachment、bubble、marker、message、message-scroller 等） | 随组件一并清理 |
| button.tsx 中 xs / icon-xs size（可选） | 官网无此尺寸；若删除需同步清理 7 Sizes Demo |

### 建议保留的官方组件

accordion, alert, alert-dialog, aspect-ratio, avatar, badge, breadcrumb, button（建议对齐官网样式/尺寸）, button-group, calendar, card, carousel, chart, checkbox, collapsible, command, context-menu, dialog, direction, drawer, dropdown-menu, empty, field, hover-card, input, input-group, input-otp, kbd, label, menubar, navigation-menu, pagination, popover, progress, radio-group, resizable, scroll-area, select, separator, sheet, sidebar, skeleton, slider, sonner, spinner, switch, table, tabs, textarea, toggle, toggle-group, tooltip。

### 其他发现

- 官网有而本地缺失：`item.tsx`（Item）、`form.tsx`（Form），建议补齐。
- 组件基座为 `@base-ui/react`（官网当前默认 Radix），多数组件 API 靠近但细节 prop 可能不同，如需 1:1 对齐官网应切回 Radix 封装。

### 改进方案摘要

1. 删除 6 个非官方 ui 组件及对应 showcase demo；date-picker 降级为示例。
2. Button：删除 xs/icon-xs（或明确标注为本地扩展），对齐官网 rounded-md、hover:bg-primary/90、destructive 实色；loading Demo 统一使用 Spinner。
3. 补齐官方 item、form 组件。
4. 决策点：是否整体从 base-ui 迁回 radix-ui 以对齐官网（工作量最大，需队长/用户拍板）。
