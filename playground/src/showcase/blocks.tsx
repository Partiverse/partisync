import { Sidebar, SidebarContent, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuButton, SidebarMenuItem, SidebarMenuSub, SidebarMenuSubButton, SidebarMenuSubItem, SidebarProvider, SidebarRail } from "@/components/ui/sidebar"
import { Breadcrumb, BreadcrumbItem, BreadcrumbLink, BreadcrumbList, BreadcrumbPage, BreadcrumbSeparator } from "@/components/ui/breadcrumb"
import { Separator } from "@/components/ui/separator"
import { GalleryVerticalEnd } from "lucide-react"
import { LoginForm as LoginForm01 } from "@/blocks/login/login-01"
import { LoginForm as LoginForm02 } from "@/blocks/login/login-02"
import { LoginForm as LoginForm03 } from "@/blocks/login/login-03"
import { LoginForm as LoginForm04 } from "@/blocks/login/login-04"
import { LoginForm as LoginForm05 } from "@/blocks/login/login-05"
import DashboardPreview from "./dashboard"
import { Section } from "./shared"

/**
 * Block preview frame matching official block source utility patterns
 * (rounded-xl border bg-card / border-b bg-muted/40).
 */
function BlockFrame({
  title,
  description,
  children,
}: {
  title: string
  description?: string
  children: React.ReactNode
}) {
  return (
    <div className="overflow-hidden rounded-xl border bg-card">
      <div className="border-b bg-muted/40 px-4 py-2">
        <span className="font-mono text-xs text-muted-foreground">{title}</span>
      </div>
      {description && (
        <p className="px-4 py-2 text-sm text-muted-foreground">{description}</p>
      )}
      <div className="flex min-h-[420px] items-center justify-center p-4">{children}</div>
    </div>
  )
}

/**
 * sidebar-03 replica preview — 1:1 aligned with official
 * https://ui.shadcn.com/r/styles/base-nova/sidebar-03.json (page.tsx + app-sidebar.tsx)
 * （IconPlaceholder GalleryVerticalEndIcon → lucide-react GalleryVerticalEnd）。
 */
const sidebarData = {
  navMain: [
    {
      title: "Getting Started",
      url: "#",
      items: [
        { title: "Installation", url: "#" },
        { title: "Project Structure", url: "#" },
      ],
    },
    {
      title: "Build Your Application",
      url: "#",
      items: [
        { title: "Routing", url: "#" },
        { title: "Data Fetching", url: "#", isActive: true },
        { title: "Rendering", url: "#" },
        { title: "Caching", url: "#" },
        { title: "Styling", url: "#" },
        { title: "Optimizing", url: "#" },
      ],
    },
    {
      title: "API Reference",
      url: "#",
      items: [
        { title: "Components", url: "#" },
        { title: "File Conventions", url: "#" },
        { title: "Functions", url: "#" },
      ],
    },
  ],
}

function PreviewAppSidebar() {
  return (
    <Sidebar>
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" render={<a href="#" />}>
              <div className="flex aspect-square size-8 items-center justify-center rounded-lg bg-sidebar-primary text-sidebar-primary-foreground">
                <GalleryVerticalEnd className="size-4" />
              </div>
              <div className="flex flex-col gap-0.5 leading-none">
                <span className="font-medium">Documentation</span>
                <span className="">v1.0.0</span>
              </div>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarMenu>
            {sidebarData.navMain.map((item) => (
              <SidebarMenuItem key={item.title}>
                <SidebarMenuButton
                  render={<a href={item.url} className="font-medium" />}
                >
                  {item.title}
                </SidebarMenuButton>
                {item.items?.length ? (
                  <SidebarMenuSub>
                    {item.items.map((item) => (
                      <SidebarMenuSubItem key={item.title}>
                        <SidebarMenuSubButton
                          isActive={item.isActive}
                          render={<a href={item.url} />}
                        >
                          {item.title}
                        </SidebarMenuSubButton>
                      </SidebarMenuSubItem>
                    ))}
                  </SidebarMenuSub>
                ) : null}
              </SidebarMenuItem>
            ))}
          </SidebarMenu>
        </SidebarGroup>
      </SidebarContent>
      <SidebarRail />
    </Sidebar>
  )
}

function Sidebar03Preview() {
  return (
    <SidebarProvider className="min-h-[560px] rounded-xl border">
      <PreviewAppSidebar />
      <div className="grid flex-1">
        <header className="flex h-16 shrink-0 items-center gap-2 border-b">
          <div className="flex items-center gap-2 px-3">
            <Separator
              orientation="vertical"
              className="mr-2 data-[orientation=vertical]:h-4 data-[orientation=vertical]:self-auto"
            />
            <Breadcrumb>
              <BreadcrumbList>
                <BreadcrumbItem className="hidden md:block">
                  <BreadcrumbLink href="#">Build Your Application</BreadcrumbLink>
                </BreadcrumbItem>
                <BreadcrumbSeparator className="hidden md:block" />
                <BreadcrumbItem>
                  <BreadcrumbPage>Data Fetching</BreadcrumbPage>
                </BreadcrumbItem>
              </BreadcrumbList>
            </Breadcrumb>
          </div>
        </header>
        <div className="flex flex-1 flex-col gap-4 p-4">
          <div className="grid auto-rows-min gap-4 md:grid-cols-3">
            <div className="aspect-video rounded-xl bg-muted/50" />
            <div className="aspect-video rounded-xl bg-muted/50" />
            <div className="aspect-video rounded-xl bg-muted/50" />
          </div>
          <div className="min-h-[100vh] flex-1 rounded-xl bg-muted/50 md:min-h-min" />
        </div>
      </div>
    </SidebarProvider>
  )
}

export default function BlocksSection() {
  return (
    <Section
      id="blocks-login"
      title="Blocks"
      description="Official base-nova block previews: sidebar-03 / login-01 ~ login-05 / dashboard-01"
    >
      <div className="xl:col-span-2 grid grid-cols-1 gap-4">
        <BlockFrame
          title="sidebar-03 · Documentation Navigation (Official Template)"
          description="A collapsible documentation sidebar with nested nav groups, header branding, and a breadcrumb header — the official sidebar-03 block."
        >
          <div className="w-full">
            <Sidebar03Preview />
          </div>
        </BlockFrame>
      </div>
      <div className="xl:col-span-2 grid grid-cols-1 gap-4 lg:grid-cols-2">
        <BlockFrame
          title="login-01"
          description="A minimal centered login form with email and password fields."
        >
          <div className="w-full max-w-sm">
            <LoginForm01 />
          </div>
        </BlockFrame>
        <BlockFrame
          title="login-02"
          description="A two-column login layout with a form on one side and a muted brand panel."
        >
          <div className="w-full max-w-sm">
            <LoginForm02 />
          </div>
        </BlockFrame>
        <BlockFrame
          title="login-03"
          description="A wider login form with social providers and a full-width submit button."
        >
          <div className="w-full max-w-2xl">
            <LoginForm03 />
          </div>
        </BlockFrame>
        <BlockFrame
          title="login-04"
          description="A login form paired with a quote panel for a marketing-style split layout."
        >
          <div className="w-full max-w-2xl">
            <LoginForm04 />
          </div>
        </BlockFrame>
        <div className="lg:col-span-2">
          <BlockFrame
            title="login-05"
            description="A full-width split login: form on the left, image panel on the right."
          >
            <div className="w-full max-w-4xl">
              <LoginForm05 />
            </div>
          </BlockFrame>
        </div>
      </div>
      <div className="xl:col-span-2 grid grid-cols-1 gap-4">
        <BlockFrame
          title="dashboard-01 · Sidebar Inset + Metrics + Interactive Chart + Data Table"
          description="The official dashboard-01 block: a sidebar inset with metric cards, an interactive area chart, and a recent-activity data table."
        >
          <div className="w-full">
            <DashboardPreview />
          </div>
        </BlockFrame>
      </div>
    </Section>
  )
}
