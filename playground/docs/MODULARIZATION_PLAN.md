# PartiSync Playground 全量模块化拆分方案与契约规范 (MODULARIZATION_PLAN.md)

本文档制定 PartiSync Playground 从巨石单文件到原子组件三级目录结构的模块化重构方案，确保 **275 个 Demo 源码提取及交互零破坏**，以及各级导出与消费契约的向前向后完全兼容。

---

## 一、 重构背景与核心目标

当前 `playground/src/showcase/` 下存在 5 个核心分类巨石文件：
- `basics.tsx` (~1308 行, 13 个 Section, 76 个 Demo)
- `forms.tsx` (~1082 行, 12 个 Section, 77 个 Demo)
- `overlays.tsx` (~1504 行, 8 个 Section, 41 个 Demo)
- `navigation.tsx` (~1742 行, 9 个 Section, 49 个 Demo)
- `data.tsx` (~1218 行, 7 个 Section, 32 个 Demo)

合计 5 个文件、49 个 Section、275 个 Demo，代码量达 6800+ 行。

### 核心目标（方案 A：按原子组件三级目录拆分）
1. **模块化物理拆分**：将 5 个巨石分类文件拆分为以组件/Section 为单元的子目录，每个 Section 独立成一个组件文件。
2. **275 个 Demo 源码提取零变更（0 破坏）**：
   `scripts/gen-demo-sources.mjs` 提取的键形如 `"${category}#${title}"`（如 `basics#Alert · Basic`）。重构后生成器需支持子目录递归扫描，并将所属分类（`basics` / `forms` 等）作为 category 前缀，保证提取出的 275 个键值对与原 `src/demo-sources.generated.ts` **100% 完全一致（0 diff）**。
3. **App.tsx 及全局调用无感知消费**：
   在各分类子目录下提供 `index.tsx` 聚合导出默认的 `XxxSection` 组件以及所有子组件，保持 `import BasicsSection from "./showcase/basics"` 等原有导入方式 100% 兼容。
4. **状态与辅助组件隔离**：
   对于有本地状态或复杂组件定义的 Demo（如 `ResponsiveProfileForm`、`ProgressControlledDemo`、`DataTableDemo` 等），收敛在各自的原子组件文件中，不再跨文件污染。

---

## 二、 目标目录结构规划

```
playground/src/showcase/
├── basics/
│   ├── index.tsx                         # 聚合导出 BasicsSection（作为 default）及各原子 Section
│   ├── button.tsx                        # Section id="button", 11 demos
│   ├── button-group.tsx                  # Section id="button-group", 13 demos
│   ├── badge.tsx                         # Section id="badge", 5 demos
│   ├── card.tsx                          # Section id="card", 4 demos
│   ├── separator.tsx                     # Section id="separator", 3 demos
│   ├── skeleton.tsx                      # Section id="skeleton", 5 demos
│   ├── spinner.tsx                       # Section id="spinner", 6 demos
│   ├── kbd.tsx                           # Section id="kbd", 5 demos
│   ├── aspect-ratio.tsx                  # Section id="aspect-ratio", 3 demos
│   ├── empty.tsx                         # Section id="empty", 6 demos
│   ├── attachment.tsx                    # Section id="attachment", 3 demos
│   ├── avatar.tsx                        # Section id="avatar", 8 demos
│   └── alert.tsx                         # Section id="alert", 4 demos
├── forms/
│   ├── index.tsx                         # 聚合导出 FormsSection 及各原子 Section
│   ├── field.tsx                         # Section id="field", 16 demos
│   ├── input.tsx                         # Section id="input", 10 demos
│   ├── input-group.tsx                   # Section id="input-group", 10 demos
│   ├── select.tsx                        # Section id="select", 5 demos
│   ├── checkbox.tsx                      # Section id="checkbox", 7 demos
│   ├── radio-group.tsx                   # Section id="radio-group", 6 demos
│   ├── switch.tsx                        # Section id="switch", 5 demos
│   ├── slider.tsx                        # Section id="slider", 5 demos
│   ├── textarea.tsx                      # Section id="textarea", 4 demos
│   ├── input-otp.tsx                     # Section id="input-otp", 5 demos
│   ├── date-picker.tsx                   # Section id="date-picker", 3 demos
│   └── attachment.tsx                    # Section id="attachment", 1 demo
├── overlays/
│   ├── index.tsx                         # 聚合导出 OverlaysSection 及各原子 Section
│   ├── dialog.tsx                        # Section id="dialog", 5 demos
│   ├── alert-dialog.tsx                  # Section id="alert-dialog", 6 demos
│   ├── sheet.tsx                         # Section id="sheet", 3 demos
│   ├── drawer.tsx                        # Section id="drawer" (含 ResponsiveProfileForm), 9 demos
│   ├── popover.tsx                       # Section id="popover", 4 demos
│   ├── hover-card.tsx                    # Section id="hover-card", 5 demos
│   ├── tooltip.tsx                       # Section id="tooltip", 4 demos
│   └── command.tsx                       # Section id="command", 5 demos
├── navigation/
│   ├── index.tsx                         # 聚合导出 NavigationSection 及各原子 Section
│   ├── tabs.tsx                          # Section id="tabs", 5 demos
│   ├── breadcrumb.tsx                    # Section id="breadcrumb", 6 demos
│   ├── pagination.tsx                    # Section id="pagination", 4 demos
│   ├── menubar.tsx                       # Section id="menubar", 5 demos
│   ├── navigation-menu.tsx               # Section id="navigation-menu", 2 demos
│   ├── dropdown-menu.tsx                 # Section id="dropdown-menu", 12 demos
│   ├── context-menu.tsx                  # Section id="context-menu", 10 demos
│   ├── scroll-area.tsx                   # Section id="scroll-area", 2 demos
│   └── resizable.tsx                     # Section id="resizable", 3 demos
├── data/
│   ├── index.tsx                         # 聚合导出 DataSection 及各原子 Section
│   ├── progress.tsx                      # Section id="progress" (含 ProgressControlledDemo), 3 demos
│   ├── accordion.tsx                     # Section id="accordion", 6 demos
│   ├── collapsible.tsx                   # Section id="collapsible" (含 CollapsibleControlledDemo, CollapsibleSettingsDemo, CollapsibleFileTreeDemo), 5 demos
│   ├── toggle.tsx                        # Section id="toggle", 4 demos
│   ├── toggle-group.tsx                  # Section id="toggle-group" (含 ToggleGroupCustomDemo), 7 demos
│   ├── table.tsx                         # Section id="table" (含 DataTableDemo), 4 demos
│   └── sonner.tsx                        # Section id="sonner", 3 demos
├── shared.tsx                            # 公共基础组件（Section, Demo, OnThisPage, Pagination 等保持不变）
├── blocks.tsx                            # Blocks 展示（保持不变）
├── dashboard.tsx                         # Dashboard 预览（保持不变）
└── theme.tsx                             # Theme 预览（保持不变）
```

---

## 三、 各模块详细组件拆分子文件路径与清单

### 1. basics 模块（13 个子组件文件，共 76 个 Demo）
目录：`playground/src/showcase/basics/`
- `button.tsx`: 导出 `ButtonSection`（Section id="button", title="Button", group="installation"）
  - 11 个 Demo: `Button · default`, `Button · outline`, `Button · secondary`, `Button · ghost`, `Button · destructive`, `Button · link`, `Button · 7 Sizes + Icon-only`, `Button · icon-only with label`, `Button · with icon`, `Button · Rounded`, `Button · spinner (loading)`
- `button-group.tsx`: 导出 `ButtonGroupSection`（Section id="button-group", title="Button Group", group="examples"）
  - 13 个 Demo: `ButtonGroup · Composition`, `ButtonGroup · Accessibility`, `ButtonGroup · Orientation`, `ButtonGroup · Size`, `ButtonGroup · Nested`, `ButtonGroup · Separator`, `ButtonGroup · Split`, `ButtonGroup · Input`, `ButtonGroup · Input Group`, `ButtonGroup · Dropdown Menu`, `ButtonGroup · Select`, `ButtonGroup · Popover`, `Button · As Link / Custom Element`
- `badge.tsx`: 导出 `BadgeSection`（Section id="badge", title="Badge", group="examples"）
  - 5 个 Demo: `Badge · default`, `Badge · secondary`, `Badge · outline`, `Badge · destructive`, `Badge · with icon`
- `card.tsx`: 导出 `CardSection`（Section id="card", title="Card", group="examples"）
  - 4 个 Demo: `Card · default (LoginForm)`, `Card · Simple`, `Card · Action`, `Card · Stats`
- `separator.tsx`: 导出 `SeparatorSection`（Section id="separator", title="Separator", group="examples"）
  - 3 个 Demo: `Separator · horizontal`, `Separator · vertical`, `Separator · with text`
- `skeleton.tsx`: 导出 `SkeletonSection`（Section id="skeleton", title="Skeleton", group="examples"）
  - 5 个 Demo: `Skeleton · Avatar + Lines`, `Skeleton · Card`, `Skeleton · List`, `Skeleton · Table Row`, `Skeleton · Form`
- `spinner.tsx`: 导出 `SpinnerSection`（Section id="spinner", title="Spinner", group="examples"）
  - 6 个 Demo: `Spinner · Sizes`, `Spinner · Custom Color`, `Spinner · Inside Button`, `Spinner · Full Page / Center`, `Spinner · Card Loading State`, `Spinner · Text + Spinner`
- `kbd.tsx`: 导出 `KbdSection`（Section id="kbd", title="Kbd", group="examples"）
  - 5 个 Demo: `Kbd · Single keys`, `Kbd · Combinations (shortcuts)`, `Kbd · Inside Text / Tooltip`, `Kbd · Sizes`, `Kbd · Group`
- `aspect-ratio.tsx`: 导出 `AspectRatioSection`（Section id="aspect-ratio", title="Aspect Ratio", group="examples"）
  - 3 个 Demo: `AspectRatio · Landscape（16:9）`, `AspectRatio · Portrait（4:5）`, `AspectRatio · Square（1:1）`
- `empty.tsx`: 导出 `EmptySection`（Section id="empty", title="Empty", group="examples"）
  - 6 个 Demo: `Empty · Composition`, `Empty · Outline`, `Empty · Background`, `Empty · Small`, `Empty · Action`, `Empty · Avatar`
- `attachment.tsx`: 导出 `AttachmentSection`（Section id="attachment", title="Attachment", group="examples"）
  - 3 个 Demo: `Attachment · Composition`, `Attachment · States（uploaded / uploading）`, `Attachment · Group`
- `avatar.tsx`: 导出 `AvatarSection`（Section id="avatar", title="Avatar", group="examples"）
  - 8 个 Demo: `Avatar · image`, `Avatar · fallback`, `Avatar · sizes`, `Avatar · badge/status`, `Avatar · Group`, `Avatar · Group Stacked`, `Avatar · Group Overflow`, `Avatar · Group with Tooltip`
- `alert.tsx`: 导出 `AlertSection`（Section id="alert", title="Alert", group="examples"）
  - 4 个 Demo: `Alert · Basic`, `Alert · Destructive`, `Alert · Action`, `Alert · Custom Colors`
- `index.tsx`: 聚合导出：
  ```tsx
  export { ButtonSection } from "./button"
  export { ButtonGroupSection } from "./button-group"
  export { BadgeSection } from "./badge"
  export { CardSection } from "./card"
  export { SeparatorSection } from "./separator"
  export { SkeletonSection } from "./skeleton"
  export { SpinnerSection } from "./spinner"
  export { KbdSection } from "./kbd"
  export { AspectRatioSection } from "./aspect-ratio"
  export { EmptySection } from "./empty"
  export { AttachmentSection } from "./attachment"
  export { AvatarSection } from "./avatar"
  export { AlertSection } from "./alert"

  export default function BasicsSection() {
    return (
      <div className="flex flex-col gap-12">
        <ButtonSection />
        <ButtonGroupSection />
        <BadgeSection />
        <CardSection />
        <SeparatorSection />
        <SkeletonSection />
        <SpinnerSection />
        <KbdSection />
        <AspectRatioSection />
        <EmptySection />
        <AttachmentSection />
        <AvatarSection />
        <AlertSection />
      </div>
    )
  }
  ```

### 2. forms 模块（12 个子组件文件，共 77 个 Demo）
目录：`playground/src/showcase/forms/`
- `field.tsx`: 导出 `FieldSection`（16 demos: `Field · Composition`, `Field · With Label + Description`, `Field · Error Message`, `Field · Required Indicator`, `Field · Optional Indicator`, `Field · With Input`, `Field · With Textarea`, `Field · With Select`, `Field · With Checkbox`, `Field · With Switch`, `Field · With Slider`, `Field · With RadioGroup`, `Field · Group`, `Field · Set (Fieldset)`, `Field · Separator / Legend`, `Field · Responsive / Grid Layout`）
- `input.tsx`: 导出 `InputSection`（10 demos: `Input · default`, `Input · disabled`, `Input · with placeholder`, `Input · type password`, `Input · type file`, `Input · rounded full`, `Input · with label and helper text`, `Input · form validation error state`, `Input · clearable`, `Input · search`）
- `input-group.tsx`: 导出 `InputGroupSection`（10 demos: `InputGroup · Composition`, `InputGroup · Left Icon`, `InputGroup · Right Icon`, `InputGroup · Prefix / Suffix Text`, `InputGroup · Action Button`, `InputGroup · Password Toggle`, `InputGroup · Search with Clear`, `InputGroup · Select Prefix`, `InputGroup · Copy Button`, `InputGroup · Sizes`）
- `select.tsx`: 导出 `SelectSection`（5 demos: `Select · default`, `Select · with groups`, `Select · scrollable / long list`, `Select · disabled`, `Select · disabled items`）
- `checkbox.tsx`: 导出 `CheckboxSection`（7 demos: `Checkbox · default (unchecked / checked)`, `Checkbox · disabled`, `Checkbox · disabled checked`, `Checkbox · with label`, `Checkbox · with description`, `Checkbox · card (interactive item)`, `Checkbox · group`）
- `radio-group.tsx`: 导出 `RadioGroupSection`（6 demos: `RadioGroup · default`, `RadioGroup · with label & description`, `RadioGroup · horizontal`, `RadioGroup · disabled`, `RadioGroup · card style`, `RadioGroup · compact`）
- `switch.tsx`: 导出 `SwitchSection`（5 demos: `Switch · default`, `Switch · disabled`, `Switch · with label`, `Switch · with description`, `Switch · in a form card`）
- `slider.tsx`: 导出 `SliderSection`（5 demos: `Slider · default`, `Slider · range (dual thumb)`, `Slider · with steps & marks`, `Slider · disabled`, `Slider · vertical`）
- `textarea.tsx`: 导出 `TextareaSection`（4 demos: `Textarea · default`, `Textarea · disabled`, `Textarea · with label & description`, `Textarea · character count / max length`）
- `input-otp.tsx`: 导出 `InputOtpSection`（5 demos: `InputOTP · 6 digits`, `InputOTP · 4 digits`, `InputOTP · with separator`, `InputOTP · alphanumeric`, `InputOTP · disabled`）
- `date-picker.tsx`: 导出 `DatePickerSection`（3 demos: `DatePicker · default`, `DatePicker · with presets`, `DatePicker · disabled`）
- `attachment.tsx`: 导出 `AttachmentSection`（1 demo: `Attachment · Field Integration`）
- `index.tsx`: 聚合导出 `FormsSection` 默认组件及所有子 Section。

### 3. overlays 模块（8 个子组件文件，共 41 个 Demo）
目录：`playground/src/showcase/overlays/`
- `dialog.tsx`: 导出 `DialogSection`（5 demos: `Dialog · default`, `Dialog · with form`, `Dialog · custom close`, `Dialog · scrollable content`, `Dialog · sticky header / footer`）
- `alert-dialog.tsx`: 导出 `AlertDialogSection`（6 demos: `AlertDialog · default`, `AlertDialog · destructive confirm`, `AlertDialog · cancel with custom text`, `AlertDialog · form in alert dialog`, `AlertDialog · multi-step confirmation`, `AlertDialog · custom trigger button`）
- `sheet.tsx`: 导出 `SheetSection`（3 demos: `Sheet · 4 sides (top / right / bottom / left)`, `Sheet · with form`, `Sheet · custom width / size`）
- `drawer.tsx`: 导出 `DrawerSection`（含内部 helper `ResponsiveProfileForm`，9 demos: `Drawer · default (bottom sheet)`, `Drawer · right side`, `Drawer · left side`, `Drawer · top side`, `Drawer · snap points`, `Drawer · scrollable content`, `Drawer · with form`, `Drawer · non-dismissible`, `Drawer · Responsive`）
- `popover.tsx`: 导出 `PopoverSection`（4 demos: `Popover · default`, `Popover · with form`, `Popover · positioning (top / right / bottom / left)`, `Popover · custom width`）
- `hover-card.tsx`: 导出 `HoverCardSection`（5 demos: `HoverCard · default`, `HoverCard · user profile preview`, `HoverCard · instant open (no delay)`, `HoverCard · custom positioning`, `HoverCard · rich media`）
- `tooltip.tsx`: 导出 `TooltipSection`（4 demos: `Tooltip · default`, `Tooltip · 4 sides (top / right / bottom / left)`, `Tooltip · with delay`, `Tooltip · custom content`）
- `command.tsx`: 导出 `CommandSection`（5 demos: `Command · default (inline)`, `Command · in Dialog (palette)`, `Command · with groups and shortcuts`, `Command · empty state`, `Command · loading state`）
- `index.tsx`: 聚合导出 `OverlaysSection` 默认组件及所有子 Section。

### 4. navigation 模块（9 个子组件文件，共 49 个 Demo）
目录：`playground/src/showcase/navigation/`
- `tabs.tsx`: 导出 `TabsSection`（5 demos: `Tabs · default`, `Tabs · vertical`, `Tabs · line variant`, `Tabs · disabled tab`, `Tabs · with badge on trigger`）
- `breadcrumb.tsx`: 导出 `BreadcrumbSection`（6 demos: `Breadcrumb · default`, `Breadcrumb · custom separator`, `Breadcrumb · with dropdown / collapsed`, `Breadcrumb · with icons`, `Breadcrumb · router Link integration`, `Breadcrumb · responsive`）
- `pagination.tsx`: 导出 `PaginationSection`（4 demos: `Pagination · default`, `Pagination · with ellipsis`, `Pagination · compact / simple`, `Pagination · disabled states`）
- `menubar.tsx`: 导出 `MenubarSection`（5 demos: `Menubar · default`, `Menubar · with shortcuts`, `Menubar · with checkboxes & radios`, `Menubar · nested submenus`, `Menubar · disabled items`）
- `navigation-menu.tsx`: 导出 `NavigationMenuSection`（2 demos: `NavigationMenu · default`, `NavigationMenu · simple list`）
- `dropdown-menu.tsx`: 导出 `DropdownMenuSection`（12 demos: `DropdownMenu · default`, `DropdownMenu · with shortcuts`, `DropdownMenu · with checkbox items`, `DropdownMenu · with radio group`, `DropdownMenu · with submenus`, `DropdownMenu · with separators and labels`, `DropdownMenu · destructive item`, `DropdownMenu · custom trigger (avatar)`, `DropdownMenu · positioning`, `DropdownMenu · complex user profile menu`, `DropdownMenu · quick actions palette`, `DropdownMenu · notification list with unread badges`）
- `context-menu.tsx`: 导出 `ContextMenuSection`（10 demos: `ContextMenu · default`, `ContextMenu · with shortcuts`, `ContextMenu · with checkboxes`, `ContextMenu · with radio items`, `ContextMenu · with submenus`, `ContextMenu · disabled items`, `ContextMenu · file manager context`, `ContextMenu · code editor context`, `ContextMenu · table row context`, `ContextMenu · canvas element context`）
- `scroll-area.tsx`: 导出 `ScrollAreaSection`（2 demos: `ScrollArea · vertical`, `ScrollArea · horizontal / tags`）
- `resizable.tsx`: 导出 `ResizableSection`（3 demos: `Resizable · horizontal`, `Resizable · vertical`, `Resizable · nested / 3 panels`）
- `index.tsx`: 聚合导出 `NavigationSection` 默认组件及所有子 Section。

### 5. data 模块（7 个子组件文件，共 32 个 Demo）
目录：`playground/src/showcase/data/`
- `progress.tsx`: 导出 `ProgressSection`（含 helper `ProgressControlledDemo`，3 demos: `Progress · Composition`, `Progress · Label`, `Progress · Controlled`）
- `accordion.tsx`: 导出 `AccordionSection`（6 demos: `Accordion · Composition`, `Accordion · Basic`, `Accordion · Multiple`, `Accordion · Disabled`, `Accordion · Borders`, `Accordion · Card`）
- `collapsible.tsx`: 导出 `CollapsibleSection`（含 helpers `CollapsibleControlledDemo`, `CollapsibleSettingsDemo`, `FileTreeNode`, `CollapsibleFileTreeDemo`，5 demos: `Collapsible · Composition`, `Collapsible · Controlled State`, `Collapsible · Basic`, `Collapsible · Settings Panel`, `Collapsible · File Tree`）
- `toggle.tsx`: 导出 `ToggleSection`（4 demos: `Toggle · Outline`, `Toggle · With Text`, `Toggle · Size`, `Toggle · Disabled`）
- `toggle-group.tsx`: 导出 `ToggleGroupSection`（含 helper `ToggleGroupCustomDemo`，7 demos: `ToggleGroup · Composition`, `ToggleGroup · Outline`, `ToggleGroup · Size`, `ToggleGroup · Spacing`, `ToggleGroup · Vertical`, `ToggleGroup · Disabled`, `ToggleGroup · Custom`）
- `table.tsx`: 导出 `TableSection`（含 helper `DataTableDemo`，4 demos: `Table · Composition`, `Table · Footer`, `Table · Actions`, `Table · Data Table`）
- `sonner.tsx`: 导出 `SonnerSection`（3 demos: `Sonner · Types`, `Sonner · Action`, `Sonner · Promise`）
- `index.tsx`: 聚合导出 `DataSection` 默认组件及所有子 Section。

---

## 四、 gen-demo-sources.mjs 扫描适配契约设计

### 1. 核心契约与不变式
`demo-sources.generated.ts` 中的键规范定义为：
```typescript
"${fileCategory}#${demoTitle}"
```
其中：
- `${fileCategory}` 必须严格保持为原有的 5 个分类基名：`"basics"`, `"forms"`, `"overlays"`, `"navigation"`, `"data"`。
- `${demoTitle}` 保持完全不变。

### 2. 生成器扫描规则调整实现策略
在当前 `scripts/gen-demo-sources.mjs` 中：
```javascript
const TARGET_FILES = ["basics", "forms", "overlays", "navigation", "data"];
```
原逻辑是直接拼成 `${showcaseDir}/${name}.tsx`。

**平滑兼容改造方案**：
生成器应同时支持**子目录化结构**与**单文件结构**（防回归与渐进式重构兼容）：
```javascript
for (const category of TARGET_FILES) {
  const dirPath = path.join(showcaseDir, category);
  const singleFilePath = path.join(showcaseDir, `${category}.tsx`);

  if (fs.existsSync(dirPath) && fs.statSync(dirPath).isDirectory()) {
    // 递归或遍历扫描子目录下的所有 .tsx 文件（排除 index.tsx）
    const entries = fs.readdirSync(dirPath, { withFileTypes: true })
      .filter((ent) => ent.isFile() && ent.name.endsWith(".tsx") && ent.name !== "index.tsx")
      .sort((a, b) => a.name.localeCompare(b.name));

    for (const entry of entries) {
      const subFilePath = path.join(dirPath, entry.name);
      // 注意：传入 category 作为 key 的基名前缀，而非子文件名！
      processFile(subFilePath, sources, errors, category);
    }
  } else if (fs.existsSync(singleFilePath)) {
    processFile(singleFilePath, sources, errors, category);
  } else {
    errors.push(`缺少目标模块: ${category}`);
  }
}
```

并在 `processFile(filePath, sources, errors, category)` 中：
```javascript
function processFile(filePath, sources, errors, category) {
  // keyPrefix 优先使用传入的 category，回退为 basename
  const base = category || path.basename(filePath, ".tsx");
  ...
  // 提取同文件组件定义时，各独立子组件文件内的 helper（如 DataTableDemo 等）
  // 仅在当前文件内搜索，完美匹配 singleComponentChild(compName)
  ...
  sources.set(`${base}#${title}`, code);
}
```

### 3. 同名 Title 冲突检测作用域
原生成器中 `seenTitles` 是以单个文件为作用域。当一个分类被拆成多个子文件后，同一个 `category` 下依然**严禁出现同名 Title**。
因此 `seenTitles` 的 map 应该以 `category` 为作用域（在扫描每个 category 时共享一个 `categorySeenTitles`），这样如果在 `basics/` 的不同子文件中不小心写了同名 title，能第一时间报错拦截。

---

## 五、 App.tsx 与 shared.tsx 兼容性契约

### 1. Section 组件传参契约
在各子组件（例如 `basics/alert.tsx`）中，`Section` 的 `fileKey` 必须显式或隐式传递所属的大分类：
```tsx
<Section
  id="alert"
  title="Alert"
  group="examples"
  fileKey="basics"  // 必须保持为 "basics"！
  description="Displays a callout for user attention."
>
```
由于 `shared.tsx` 中的 `Demo` 依赖：
```tsx
const fileKey = React.useContext(FileKeyContext)
const source = (fileKey ? demoSources[`${fileKey}#${title}`] : undefined) ?? ...
```
只要 `Section` 的 `fileKey` 仍然是 `"basics"`、`"forms"`、`"overlays"`、`"navigation"`、`"data"`，`Demo` 组件就能无缝从 `demoSources["basics#Alert · Basic"]` 取得源码，完全无需修改 `shared.tsx` 的接口契约！

### 2. App.tsx 导入契约
在 `App.tsx` 中原本的导入为：
```tsx
import BasicsSection from "./showcase/basics"
import FormsSection from "./showcase/forms"
import OverlaysSection from "./showcase/overlays"
import NavigationSection from "./showcase/navigation"
import DataSection from "./showcase/data"
```
当 `src/showcase/basics/index.tsx` 等存在时，TypeScript 与 Vite 解析 `./showcase/basics` 会自动寻址到 `./showcase/basics/index.tsx`（当移除原 `basics.tsx` 之后）。
因此 `App.tsx` 的代码**无需任何改动**，完全实现向前向后平滑兼容。

---

## 六、 实施步骤与工程师任务拆解（给 Engineer / Reviewer 的执行指南）

### Step 1: 更新 `scripts/gen-demo-sources.mjs`
- 改造 `gen-demo-sources.mjs` 支持目录扫描（排除 `index.tsx`，按分类名称作为 prefix 生成 key）。
- 确保同 category 跨文件 title 唯一性检测。

### Step 2: 逐个模块拆分实施（建议顺序：basics → forms → overlays → navigation → data）
对每个分类：
1. 创建 `playground/src/showcase/${category}/` 目录。
2. 逐一将各个 Section 及其所需 import 和本地 helper 提取到独立的 `${id}.tsx`（注意：`fileKey="${category}"`）。
3. 编写 `${category}/index.tsx`，聚合导入并默认导出 `${Category}Section`。
4. 删除或重命名原 `src/showcase/${category}.tsx`。
5. 运行 `node scripts/gen-demo-sources.mjs`，比对生成的 `src/demo-sources.generated.ts`。

### Step 3: 全局验证与质量门禁（验收标准）
1. **源码零差异检查**：
   运行 `node scripts/gen-demo-sources.mjs`，执行 `git diff src/demo-sources.generated.ts`，要求 **完全无 diff（0 行差异，275 个 key 100% 保持一致）**。
2. **TypeScript 编译检查**：
   在 `playground/` 下运行 `npm run build`（或 `npx tsc -b`），要求 **0 error**。
3. **Vite 打包检查**：
   运行 `npm run build` 完成产物构建。
4. **运行态验证**：
   所有 Demo 的预览与源码切换面板（Code 按钮）正常展示源码，TOC 锚点跳转无异常。
