import React from "react"

// Basics components
import BasicsSection, {
  ButtonSection,
  ButtonGroupSection,
  BadgeSection,
  CardSection,
  SeparatorSection,
  SkeletonSection,
  SpinnerSection,
  KbdSection,
  AspectRatioSection,
  EmptySection,
  AvatarSection,
  AlertSection,
  CarouselSection,
  TypographySection,
  ItemSection,
} from "./basics"

// Forms components
import FormsSection, {
  FieldSection,
  InputSection,
  InputGroupSection,
  SelectSection,
  CheckboxSection,
  RadioGroupSection,
  SwitchSection,
  SliderSection,
  TextareaSection,
  InputOtpSection,
  ComboboxSection,
  FormSection,
  FormZodSection,
  NativeSelectSection,
} from "./forms"

// Overlays components
import OverlaysSection, {
  DialogSection,
  AlertDialogSection,
  SheetSection,
  DrawerSection,
  PopoverSection,
  HoverCardSection,
  TooltipSection,
  CommandSection,
} from "./overlays"

// Navigation components
import NavigationSection, {
  TabsSection,
  BreadcrumbSection,
  PaginationSection,
  MenubarSection,
  NavigationMenuSection,
  DropdownMenuSection,
  ContextMenuSection,
  ScrollAreaSection,
  ResizableSection,
} from "./navigation"

// Data components
import DataSection, {
  ProgressSection,
  AccordionSection,
  CollapsibleSection,
  ToggleSection,
  ToggleGroupSection,
  TableSection,
  SonnerSection,
  ChartSection,
  TimelineSection,
} from "./data"

// Top-level blocks and special views
import BlocksSection from "./blocks"
import DashboardPreview from "./dashboard"
import ThemeSection from "./theme"

export type CategoryId =
  | "basics"
  | "forms"
  | "overlays"
  | "navigation"
  | "data"
  | "blocks"
  | "dashboard"
  | "theme"

export interface ComponentItem {
  id: string
  title: string
  category: CategoryId
  render: () => React.ReactNode
}

export interface CategoryDefinition {
  id: CategoryId
  title: string
  description?: string
  collapsible: boolean
  allRender?: () => React.ReactNode
  components: ComponentItem[]
}

export interface RouteState {
  category: CategoryId
  componentId?: string
}

/**
 * 全局分类与 49 个原子组件统一注册表
 */
export const CATEGORY_REGISTRY: CategoryDefinition[] = [
  {
    id: "basics",
    title: "Basics",
    collapsible: true,
    allRender: () => <BasicsSection />,
    components: [
      { id: "button", title: "Button", category: "basics", render: () => <ButtonSection /> },
      { id: "button-group", title: "Button Group", category: "basics", render: () => <ButtonGroupSection /> },
      { id: "badge", title: "Badge", category: "basics", render: () => <BadgeSection /> },
      { id: "card", title: "Card", category: "basics", render: () => <CardSection /> },
      { id: "separator", title: "Separator", category: "basics", render: () => <SeparatorSection /> },
      { id: "skeleton", title: "Skeleton", category: "basics", render: () => <SkeletonSection /> },
      { id: "spinner", title: "Spinner", category: "basics", render: () => <SpinnerSection /> },
      { id: "kbd", title: "Kbd", category: "basics", render: () => <KbdSection /> },
      { id: "aspect-ratio", title: "Aspect Ratio", category: "basics", render: () => <AspectRatioSection /> },
      { id: "empty", title: "Empty", category: "basics", render: () => <EmptySection /> },
      { id: "avatar", title: "Avatar", category: "basics", render: () => <AvatarSection /> },
      { id: "alert", title: "Alert", category: "basics", render: () => <AlertSection /> },
      { id: "carousel", title: "Carousel", category: "basics", render: () => <CarouselSection /> },
      { id: "typography", title: "Typography", category: "basics", render: () => <TypographySection /> },
      { id: "item", title: "Item", category: "basics", render: () => <ItemSection /> },
    ],
  },
  {
    id: "forms",
    title: "Forms",
    collapsible: true,
    allRender: () => <FormsSection />,
    components: [
      { id: "field", title: "Field", category: "forms", render: () => <FieldSection /> },
      { id: "input", title: "Input", category: "forms", render: () => <InputSection /> },
      { id: "input-group", title: "Input Group", category: "forms", render: () => <InputGroupSection /> },
      { id: "select", title: "Select", category: "forms", render: () => <SelectSection /> },
      { id: "checkbox", title: "Checkbox", category: "forms", render: () => <CheckboxSection /> },
      { id: "radio-group", title: "Radio Group", category: "forms", render: () => <RadioGroupSection /> },
      { id: "switch", title: "Switch", category: "forms", render: () => <SwitchSection /> },
      { id: "slider", title: "Slider", category: "forms", render: () => <SliderSection /> },
      { id: "textarea", title: "Textarea", category: "forms", render: () => <TextareaSection /> },
      { id: "input-otp", title: "Input OTP", category: "forms", render: () => <InputOtpSection /> },
      { id: "combobox", title: "Combobox", category: "forms", render: () => <ComboboxSection /> },
      { id: "form", title: "Form", category: "forms", render: () => <FormSection /> },
      { id: "form-zod", title: "Form + Zod", category: "forms", render: () => <FormZodSection /> },
      { id: "native-select", title: "Native Select", category: "forms", render: () => <NativeSelectSection /> },
    ],
  },
  {
    id: "overlays",
    title: "Overlays",
    collapsible: true,
    allRender: () => <OverlaysSection />,
    components: [
      { id: "dialog", title: "Dialog", category: "overlays", render: () => <DialogSection /> },
      { id: "alert-dialog", title: "Alert Dialog", category: "overlays", render: () => <AlertDialogSection /> },
      { id: "sheet", title: "Sheet", category: "overlays", render: () => <SheetSection /> },
      { id: "drawer", title: "Drawer", category: "overlays", render: () => <DrawerSection /> },
      { id: "popover", title: "Popover", category: "overlays", render: () => <PopoverSection /> },
      { id: "hover-card", title: "Hover Card", category: "overlays", render: () => <HoverCardSection /> },
      { id: "tooltip", title: "Tooltip", category: "overlays", render: () => <TooltipSection /> },
      { id: "command", title: "Command", category: "overlays", render: () => <CommandSection /> },
    ],
  },
  {
    id: "navigation",
    title: "Navigation",
    collapsible: true,
    allRender: () => <NavigationSection />,
    components: [
      { id: "tabs", title: "Tabs", category: "navigation", render: () => <TabsSection /> },
      { id: "breadcrumb", title: "Breadcrumb", category: "navigation", render: () => <BreadcrumbSection /> },
      { id: "pagination", title: "Pagination", category: "navigation", render: () => <PaginationSection /> },
      { id: "menubar", title: "Menubar", category: "navigation", render: () => <MenubarSection /> },
      { id: "navigation-menu", title: "Navigation Menu", category: "navigation", render: () => <NavigationMenuSection /> },
      { id: "dropdown-menu", title: "Dropdown Menu", category: "navigation", render: () => <DropdownMenuSection /> },
      { id: "context-menu", title: "Context Menu", category: "navigation", render: () => <ContextMenuSection /> },
      { id: "scroll-area", title: "Scroll Area", category: "navigation", render: () => <ScrollAreaSection /> },
      { id: "resizable", title: "Resizable", category: "navigation", render: () => <ResizableSection /> },
    ],
  },
  {
    id: "data",
    title: "Data",
    collapsible: true,
    allRender: () => <DataSection />,
    components: [
      { id: "progress", title: "Progress", category: "data", render: () => <ProgressSection /> },
      { id: "accordion", title: "Accordion", category: "data", render: () => <AccordionSection /> },
      { id: "collapsible", title: "Collapsible", category: "data", render: () => <CollapsibleSection /> },
      { id: "toggle", title: "Toggle", category: "data", render: () => <ToggleSection /> },
      { id: "toggle-group", title: "Toggle Group", category: "data", render: () => <ToggleGroupSection /> },
      { id: "table", title: "Table", category: "data", render: () => <TableSection /> },
      { id: "sonner", title: "Sonner", category: "data", render: () => <SonnerSection /> },
      { id: "chart", title: "Chart", category: "data", render: () => <ChartSection /> },
      { id: "timeline", title: "Timeline", category: "data", render: () => <TimelineSection /> },
    ],
  },
  {
    id: "blocks",
    title: "Blocks",
    collapsible: false,
    allRender: () => <BlocksSection />,
    components: [],
  },
  {
    id: "dashboard",
    title: "Dashboard",
    collapsible: false,
    allRender: () => <DashboardPreview />,
    components: [],
  },
  {
    id: "theme",
    title: "Theme",
    collapsible: false,
    allRender: () => <ThemeSection />,
    components: [],
  },
]

/** 分类 ID 快速集合 */
const CATEGORY_MAP = new Map<CategoryId, CategoryDefinition>(
  CATEGORY_REGISTRY.map((c) => [c.id, c])
)

/** 全局所有 49 个原子组件拍平列表 */
export const ALL_COMPONENTS: ComponentItem[] = CATEGORY_REGISTRY.flatMap(
  (cat) => cat.components
)

/** 线性翻页顺序结构 */
export interface NavTarget {
  category: CategoryId
  componentId?: string
  title: string
  breadcrumb: string
  url: string
}

export const LINEAR_NAV_ITEMS: NavTarget[] = CATEGORY_REGISTRY.flatMap((cat) => {
  const items: NavTarget[] = []
  if (cat.allRender) {
    items.push({
      category: cat.id,
      title: `${cat.title} (Overview)`,
      breadcrumb: cat.title,
      url: `#${cat.id}`,
    })
  }
  for (const comp of cat.components) {
    items.push({
      category: cat.id,
      componentId: comp.id,
      title: comp.title,
      breadcrumb: `${cat.title} > ${comp.title}`,
      url: `#${cat.id}/${comp.id}`,
    })
  }
  return items
})

/**
 * 判断是否为合法分类
 */
export function isCategoryId(id: string): id is CategoryId {
  return CATEGORY_MAP.has(id as CategoryId)
}

/**
 * 根据 ID 查找组件
 */
export function findComponent(
  category: CategoryId,
  componentId: string
): ComponentItem | undefined {
  const cat = CATEGORY_MAP.get(category)
  return cat?.components.find((c) => c.id === componentId)
}

/**
 * 根据简写查找组件
 */
export function findComponentByShortId(componentId: string): ComponentItem | undefined {
  return ALL_COMPONENTS.find((c) => c.id === componentId)
}

/**
 * 解析 URL Hash 返回规范化的 RouteState
 * 支持:
 * - #basics/button
 * - #/basics/button
 * - #button
 * - #basics
 * - #basics/all
 */
export function parseHash(hash: string): RouteState {
  const clean = hash.replace(/^#\/?/, "").trim()
  if (!clean) {
    return { category: "basics" }
  }

  const parts = clean.split("/")
  if (parts.length >= 2) {
    const [cat, comp] = parts
    if (isCategoryId(cat)) {
      if (comp === "all" || !comp) {
        return { category: cat }
      }
      if (findComponent(cat, comp)) {
        return { category: cat, componentId: comp }
      }
      // 容错：如果分类下无此组件但该组件在其他分类中
      const compInOther = findComponentByShortId(comp)
      if (compInOther) {
        return { category: compInOther.category, componentId: compInOther.id }
      }
      return { category: cat }
    }
  }

  // 单段情况
  const single = parts[0]
  if (isCategoryId(single)) {
    return { category: single }
  }

  const foundComp = findComponentByShortId(single)
  if (foundComp) {
    return { category: foundComp.category, componentId: foundComp.id }
  }

  // 默认 fallback
  return { category: "basics" }
}

/**
 * 生成规范的 Hash URL
 */
export function stringifyRoute(route: RouteState): string {
  if (!route.componentId || route.componentId === "all") {
    return `#${route.category}`
  }
  return `#${route.category}/${route.componentId}`
}

/**
 * 获取当前路由的渲染内容与面包屑信息
 */
export function getRouteView(route: RouteState): {
  breadcrumbCategory: string
  breadcrumbComponent?: string
  render: () => React.ReactNode
} {
  const cat = CATEGORY_MAP.get(route.category) ?? CATEGORY_MAP.get("basics")!
  if (route.componentId && route.componentId !== "all") {
    const comp = findComponent(cat.id, route.componentId)
    if (comp) {
      return {
        breadcrumbCategory: cat.title,
        breadcrumbComponent: comp.title,
        render: comp.render,
      }
    }
  }

  return {
    breadcrumbCategory: cat.title,
    render: cat.allRender ?? (() => null),
  }
}
