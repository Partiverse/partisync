import { useEffect, useState } from "react"
import { Toaster } from "@/components/ui/sonner"
import { TooltipProvider } from "@/components/ui/tooltip"
import { DirectionProvider } from "@/components/ui/direction"
import { Separator } from "@/components/ui/separator"
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar"
import { useTheme } from "next-themes"
import { Moon, Sun } from "lucide-react"
import { Button } from "@/components/ui/button"
import { AssetLibraryPage } from "@/pages/assets"

function ComponentShowcasePage() {
  return (
    <div className="flex flex-col gap-6">
      <h1 className="text-2xl font-semibold">Component Showcase</h1>
      <p className="text-muted-foreground">
        Components disabled in this build — see original playground setup.
      </p>
    </div>
  )
}

export default function App() {
  const [hash, setHash] = useState(() => {
    if (typeof window !== "undefined") {
      return window.location.hash.replace(/^#\/?/, "") || "assets"
    }
    return "assets"
  })
  const { theme, setTheme } = useTheme()

  useEffect(() => {
    const handleHashChange = () => {
      setHash(window.location.hash.replace(/^#\/?/, "") || "assets")
    }
    window.addEventListener("hashchange", handleHashChange)
    return () => window.removeEventListener("hashchange", handleHashChange)
  }, [])

  const isAssets = hash === "assets"

  return (
    <DirectionProvider direction="ltr">
      <TooltipProvider>
        <Toaster position="top-center" richColors />
        <SidebarProvider>
          <Sidebar collapsible="icon">
            <SidebarHeader>
              <div className="flex items-center gap-2 px-2">
                <span className="font-semibold text-sm">PartiSync</span>
              </div>
            </SidebarHeader>
            <SidebarContent>
              <SidebarMenu>
                <SidebarMenuItem>
                  <SidebarMenuButton
                    isActive={isAssets}
                    onClick={() => {
                      window.location.hash = "#assets"
                    }}
                  >
                    <span>资产管理</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
                <SidebarMenuItem>
                  <SidebarMenuButton
                    isActive={!isAssets}
                    onClick={() => {
                      window.location.hash = "#components"
                    }}
                  >
                    <span>组件</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarContent>
            <SidebarFooter>
              <div className="px-2">
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-8"
                  aria-label="Toggle theme"
                  onClick={() => {
                    const themes = ["light", "dark"]
                    const current = theme ?? "light"
                    const next = themes[(themes.indexOf(current) + 1) % themes.length]
                    setTheme(next)
                  }}
                >
                  <Sun className="size-4 rotate-0 scale-100 transition-all dark:-rotate-90 dark:scale-0" />
                  <Moon className="absolute size-4 rotate-90 scale-0 transition-all dark:rotate-0 dark:scale-100" />
                </Button>
              </div>
            </SidebarFooter>
          </Sidebar>

          <SidebarInset>
            <header className="sticky top-0 z-40 flex h-14 shrink-0 items-center gap-2 border-b bg-background px-4">
              <SidebarTrigger />
              <Separator orientation="vertical" className="mr-2 h-4" />
              <span className="text-sm font-medium">
                {isAssets ? "资产管理" : "组件展示"}
              </span>
              <div className="ml-auto">
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-8"
                  aria-label="Toggle theme"
                  onClick={() => {
                    const themes = ["light", "dark"]
                    const current = theme ?? "light"
                    const next = themes[(themes.indexOf(current) + 1) % themes.length]
                    setTheme(next)
                  }}
                >
                  <Sun className="size-4 rotate-0 scale-100 transition-all dark:-rotate-90 dark:scale-0" />
                  <Moon className="absolute size-4 rotate-90 scale-0 transition-all dark:rotate-0 dark:scale-100" />
                </Button>
              </div>
            </header>

            <div className="flex-1 overflow-auto p-6">
              {isAssets ? <AssetLibraryPage /> : <ComponentShowcasePage />}
            </div>
          </SidebarInset>
        </SidebarProvider>
      </TooltipProvider>
    </DirectionProvider>
  )
}
