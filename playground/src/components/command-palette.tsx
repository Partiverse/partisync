"use client"

import * as React from "react"
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui/command"
import {
  CATEGORY_REGISTRY,
  LINEAR_NAV_ITEMS,
  type CategoryId,
  type RouteState,
} from "@/showcase/navigation-registry"
import { CornerDownLeftIcon } from "lucide-react"

interface CommandPaletteProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onNavigate: (route: RouteState) => void
}

function groupByCategory(items: typeof LINEAR_NAV_ITEMS) {
  const groups: Record<string, typeof LINEAR_NAV_ITEMS> = {}
  for (const item of items) {
    if (!groups[item.category]) {
      groups[item.category] = []
    }
    groups[item.category].push(item)
  }
  return groups
}

const CATEGORY_LABELS: Record<CategoryId, string> = {
  basics: "Basics",
  forms: "Forms",
  overlays: "Overlays",
  navigation: "Navigation",
  data: "Data",
  blocks: "Blocks",
  dashboard: "Dashboard",
  theme: "Theme",
}

export function CommandPalette({
  open,
  onOpenChange,
  onNavigate,
}: CommandPaletteProps) {
  const [value, setValue] = React.useState("")

  // Keyboard shortcut: Cmd+K or Ctrl+K
  React.useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault()
        onOpenChange(!open)
      }
    }

    document.addEventListener("keydown", handleKeyDown)
    return () => document.removeEventListener("keydown", handleKeyDown)
  }, [open, onOpenChange])

  // Reset input when dialog closes
  React.useEffect(() => {
    if (!open) {
      setValue("")
    }
  }, [open])

  const handleSelect = (route: RouteState) => {
    onOpenChange(false)
    setValue("")
    onNavigate(route)
  }

  // Group linear nav items by category
  const groupedItems = groupByCategory(LINEAR_NAV_ITEMS)

  return (
    <CommandDialog open={open} onOpenChange={onOpenChange}>
      <CommandInput
        placeholder="Search components..."
        value={value}
        onValueChange={setValue}
      />
      <CommandList>
        <CommandEmpty>No component found.</CommandEmpty>

        {Object.entries(groupedItems).map(([categoryId, items]) => (
          <CommandGroup
            key={categoryId}
            heading={CATEGORY_LABELS[categoryId as CategoryId] ?? categoryId}
          >
            {items.map((item) => (
              <CommandItem
                key={item.url}
                value={item.title}
                onSelect={() =>
                  handleSelect({
                    category: item.category,
                    componentId: item.componentId,
                  })
                }
              >
                <span className="flex-1">{item.title}</span>
                <CornerDownLeftIcon className="size-3 opacity-50" />
              </CommandItem>
            ))}
          </CommandGroup>
        ))}

        <CommandSeparator />

        <CommandGroup heading="Navigation">
          {CATEGORY_REGISTRY.filter((c) => c.id !== "blocks" && c.id !== "dashboard" && c.id !== "theme").map((cat) => (
            <CommandItem
              key={cat.id}
              value={`${cat.title} overview`}
              onSelect={() =>
                handleSelect({ category: cat.id })
              }
            >
              <span className="flex-1">{cat.title} Overview</span>
              <CornerDownLeftIcon className="size-3 opacity-50" />
            </CommandItem>
          ))}
        </CommandGroup>
      </CommandList>
    </CommandDialog>
  )
}
