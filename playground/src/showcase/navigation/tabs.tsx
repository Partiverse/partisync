import { Home, Settings, Users } from "lucide-react"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Demo, Section } from "../shared"

export function TabsSection() {
  return (
<Section
        id="tabs"
        title="Tabs"
        group="installation"
        description="A set of layered sections of content—known as tab panels—that are displayed one at a time."
        fileKey="navigation"
      >
      {/* ========================================================================= */}
      {/* TABS                                                                      */}
      {/* ========================================================================= */}

      {/* Tabs · Composition */}
      <Demo
        title="Tabs · Composition"
        description="Tabs composed with a two-column trigger list and content panels."
      >
        <Tabs defaultValue="account" className="w-full max-w-md">
          <TabsList className="grid w-full grid-cols-2">
            <TabsTrigger value="account">Account</TabsTrigger>
            <TabsTrigger value="password">Password</TabsTrigger>
          </TabsList>
          <TabsContent value="account" className="space-y-2 pt-4">
            <h3 className="font-semibold text-sm">Account Settings</h3>
            <p className="text-muted-foreground text-sm">
              Make changes to your account here. Click save when you're done.
            </p>
          </TabsContent>
          <TabsContent value="password" className="space-y-2 pt-4">
            <h3 className="font-semibold text-sm">Password Management</h3>
            <p className="text-muted-foreground text-sm">
              Change your password here. After saving, you'll be logged out.
            </p>
          </TabsContent>
        </Tabs>
      </Demo>

      {/* Tabs · Line */}
      <Demo
        title="Tabs · Line"
        description="Tabs with an underline indicator instead of a segmented background."
      >
        <Tabs defaultValue="overview" className="w-full max-w-md">
          <TabsList variant="line">
            <TabsTrigger value="overview">Overview</TabsTrigger>
            <TabsTrigger value="analytics">Analytics</TabsTrigger>
            <TabsTrigger value="reports">Reports</TabsTrigger>
            <TabsTrigger value="notifications">Notifications</TabsTrigger>
          </TabsList>
          <TabsContent value="overview" className="pt-4">
            <p className="text-muted-foreground text-sm">Overview content with underlying line indicator.</p>
          </TabsContent>
          <TabsContent value="analytics" className="pt-4">
            <p className="text-muted-foreground text-sm">Detailed performance analytics and metrics.</p>
          </TabsContent>
          <TabsContent value="reports" className="pt-4">
            <p className="text-muted-foreground text-sm">Monthly generated reports and export options.</p>
          </TabsContent>
          <TabsContent value="notifications" className="pt-4">
            <p className="text-muted-foreground text-sm">Configure email and push notification settings.</p>
          </TabsContent>
        </Tabs>
      </Demo>

      {/* Tabs · Vertical */}
      <Demo
        title="Tabs · Vertical"
        description="Tabs with a vertical trigger list beside the content panel."
      >
        <Tabs defaultValue="general" orientation="vertical" className="w-full max-w-md flex-row">
          <TabsList className="w-36">
            <TabsTrigger value="general">General</TabsTrigger>
            <TabsTrigger value="security">Security</TabsTrigger>
            <TabsTrigger value="billing">Billing</TabsTrigger>
            <TabsTrigger value="advanced">Advanced</TabsTrigger>
          </TabsList>
          <div className="flex-1 pl-4">
            <TabsContent value="general">
              <h4 className="font-medium text-sm">General Information</h4>
              <p className="text-muted-foreground text-sm mt-1">Configure profile settings and workspace defaults.</p>
            </TabsContent>
            <TabsContent value="security">
              <h4 className="font-medium text-sm">Security Controls</h4>
              <p className="text-muted-foreground text-sm mt-1">Enable two-factor authentication and manage active sessions.</p>
            </TabsContent>
            <TabsContent value="billing">
              <h4 className="font-medium text-sm">Billing Plans</h4>
              <p className="text-muted-foreground text-sm mt-1">Manage payment methods, invoices, and plan subscriptions.</p>
            </TabsContent>
            <TabsContent value="advanced">
              <h4 className="font-medium text-sm">Advanced Options</h4>
              <p className="text-muted-foreground text-sm mt-1">Developer settings, webhooks, and API key management.</p>
            </TabsContent>
          </div>
        </Tabs>
      </Demo>

      {/* Tabs · Disabled */}
      <Demo
        title="Tabs · Disabled"
        description="Tabs with a disabled trigger that cannot be selected."
      >
        <Tabs defaultValue="tab1" className="w-full max-w-md">
          <TabsList>
            <TabsTrigger value="tab1">Active</TabsTrigger>
            <TabsTrigger value="tab2" disabled>Disabled</TabsTrigger>
            <TabsTrigger value="tab3">Pending</TabsTrigger>
          </TabsList>
          <TabsContent value="tab1" className="pt-4">
            <p className="text-muted-foreground text-sm">This tab is active and selectable.</p>
          </TabsContent>
          <TabsContent value="tab2" className="pt-4">
            <p className="text-muted-foreground text-sm">This tab is disabled and cannot be reached.</p>
          </TabsContent>
          <TabsContent value="tab3" className="pt-4">
            <p className="text-muted-foreground text-sm">Another selectable tab option.</p>
          </TabsContent>
        </Tabs>
      </Demo>

      {/* Tabs · Icons */}
      <Demo
        title="Tabs · Icons"
        description="Tabs with icons displayed next to their labels."
      >
        <Tabs defaultValue="home" className="w-full max-w-md">
          <TabsList className="grid w-full grid-cols-3">
            <TabsTrigger value="home" className="gap-2">
              <Home className="size-4" />
              <span>Home</span>
            </TabsTrigger>
            <TabsTrigger value="settings" className="gap-2">
              <Settings className="size-4" />
              <span>Settings</span>
            </TabsTrigger>
            <TabsTrigger value="users" className="gap-2">
              <Users className="size-4" />
              <span>Team</span>
            </TabsTrigger>
          </TabsList>
          <TabsContent value="home" className="pt-4">
            <p className="text-muted-foreground text-sm">Home dashboard panel with summary stats.</p>
          </TabsContent>
          <TabsContent value="settings" className="pt-4">
            <p className="text-muted-foreground text-sm">Application preferences and configuration.</p>
          </TabsContent>
          <TabsContent value="users" className="pt-4">
            <p className="text-muted-foreground text-sm">Manage workspace members and permissions.</p>
          </TabsContent>
        </Tabs>
      </Demo>

      </Section>
  )
}
