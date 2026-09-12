"use client"

import * as React from "react"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuAction,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
  SidebarRail,
} from "@/components/ui/sidebar"
import { ChevronRight, GalleryVerticalEnd } from "lucide-react"
import {
  CATEGORY_REGISTRY,
  type CategoryId,
  type RouteState,
  stringifyRoute,
} from "@/showcase/navigation-registry"

export interface AppSidebarProps
  extends React.ComponentProps<typeof Sidebar> {
  currentRoute: RouteState
  onNavigate?: (route: RouteState) => void
}

export function AppSidebar({
  currentRoute,
  onNavigate,
  ...props
}: AppSidebarProps) {
  // 维护分类的展开/收起状态，默认当前分类展开
  const [openCategories, setOpenCategories] = React.useState<
    Record<string, boolean>
  >(() => {
    const initial: Record<string, boolean> = {
      basics: true,
      forms: false,
      overlays: false,
      navigation: false,
      data: false,
    }
    if (currentRoute.category) {
      initial[currentRoute.category] = true
    }
    return initial
  })

  // 当外部路由变更到某个分类时，确保该分类保持展开状态
  React.useEffect(() => {
    if (currentRoute.category) {
      setOpenCategories((prev) => {
        if (prev[currentRoute.category]) return prev
        return {
          ...prev,
          [currentRoute.category]: true,
        }
      })
    }
  }, [currentRoute.category])

  const toggleCategory = (catId: string) => {
    setOpenCategories((prev) => ({
      ...prev,
      [catId]: !prev[catId],
    }))
  }

  const handleLinkClick = (
    e: React.MouseEvent<HTMLAnchorElement>,
    route: RouteState
  ) => {
    // 允许通过 onNavigate 或默认 Hash 触发
    if (onNavigate) {
      e.preventDefault()
      onNavigate(route)
    }
  }

  // 分类归组
  const componentCategories = CATEGORY_REGISTRY.filter((c) => c.collapsible)
  const directCategories = CATEGORY_REGISTRY.filter((c) => !c.collapsible)

  return (
    <Sidebar {...props}>
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton render={<a href="#basics" />} size="lg">
              <div className="flex aspect-square size-8 items-center justify-center rounded-lg bg-sidebar-primary text-sidebar-primary-foreground">
                <GalleryVerticalEnd className="size-4" />
              </div>
              <div className="flex flex-col gap-0.5 leading-none">
                <span className="font-semibold text-sm">PartiSync UI</span>
                <span className="text-xs text-muted-foreground">v1.0.0 Playground</span>
              </div>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        {/* 组件分类组（Collapsible 折叠与子组件直达） */}
        <SidebarGroup>
          <SidebarGroupLabel>Components</SidebarGroupLabel>
          <SidebarMenu>
            {componentCategories.map((category) => {
              const isCurrentCat = currentRoute.category === category.id
              const isAllActive = isCurrentCat && !currentRoute.componentId
              const isOpen = !!openCategories[category.id]

              return (
                <SidebarMenuItem key={category.id}>
                  <Collapsible
                    open={isOpen}
                    onOpenChange={() => toggleCategory(category.id)}
                    className="group/collapsible"
                  >
                    <div className="relative flex items-center">
                      <SidebarMenuButton
                        isActive={isAllActive}
                        tooltip={category.title}
                        className="pr-7 font-medium"
                        render={
                          <a
                            href={`#${category.id}`}
                            onClick={(e) =>
                              handleLinkClick(e, {
                                category: category.id as CategoryId,
                              })
                            }
                          />
                        }
                      >
                        <span>{category.title}</span>
                        <span className="ml-auto mr-1 text-[11px] text-muted-foreground/70 tabular-nums">
                          {category.components.length}
                        </span>
                      </SidebarMenuButton>

                      <CollapsibleTrigger
                        render={
                          <SidebarMenuAction
                            showOnHover={false}
                            className="hover:bg-sidebar-accent"
                            aria-label={`Toggle ${category.title}`}
                          />
                        }
                      >
                        <ChevronRight className="size-3.5 transition-transform duration-200 group-data-open/collapsible:rotate-90" />
                      </CollapsibleTrigger>
                    </div>

                    <CollapsibleContent>
                      <SidebarMenuSub>
                        {/* 全览入口 */}
                        <SidebarMenuSubItem>
                          <SidebarMenuSubButton
                            isActive={isAllActive}
                            render={
                              <a
                                href={`#${category.id}`}
                                onClick={(e) =>
                                  handleLinkClick(e, {
                                    category: category.id as CategoryId,
                                  })
                                }
                              />
                            }
                          >
                            <span className="text-xs text-muted-foreground font-medium">
                              All {category.title} (Overview)
                            </span>
                          </SidebarMenuSubButton>
                        </SidebarMenuSubItem>

                        {/* 原子组件列表 */}
                        {category.components.map((component) => {
                          const isCompActive =
                            isCurrentCat &&
                            currentRoute.componentId === component.id
                          const compRoute: RouteState = {
                            category: category.id as CategoryId,
                            componentId: component.id,
                          }

                          return (
                            <SidebarMenuSubItem key={component.id}>
                              <SidebarMenuSubButton
                                isActive={isCompActive}
                                render={
                                  <a
                                    href={stringifyRoute(compRoute)}
                                    onClick={(e) =>
                                      handleLinkClick(e, compRoute)
                                    }
                                  />
                                }
                              >
                                <span>{component.title}</span>
                              </SidebarMenuSubButton>
                            </SidebarMenuSubItem>
                          )
                        })}
                      </SidebarMenuSub>
                    </CollapsibleContent>
                  </Collapsible>
                </SidebarMenuItem>
              )
            })}
          </SidebarMenu>
        </SidebarGroup>

        {/* 特别视图组 (Blocks, Dashboard, Theme) */}
        {directCategories.length > 0 && (
          <SidebarGroup>
            <SidebarGroupLabel>Views &amp; Demos</SidebarGroupLabel>
            <SidebarMenu>
              {directCategories.map((item) => {
                const isActive =
                  currentRoute.category === item.id && !currentRoute.componentId
                const itemRoute: RouteState = {
                  category: item.id as CategoryId,
                }
                return (
                  <SidebarMenuItem key={item.id}>
                    <SidebarMenuButton
                      isActive={isActive}
                      render={
                        <a
                          href={`#${item.id}`}
                          onClick={(e) => handleLinkClick(e, itemRoute)}
                        />
                      }
                    >
                      <span>{item.title}</span>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                )
              })}
            </SidebarMenu>
          </SidebarGroup>
        )}
      </SidebarContent>
      <SidebarRail />
    </Sidebar>
  )
}
