import * as React from "react"
import { ChevronDown, ChevronRight, ChevronsUpDown, File, Folder } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Demo, Section } from "../shared"

function CollapsibleControlledDemo() {
  const [open, setOpen] = React.useState(false)

  return (
    <Collapsible open={open} onOpenChange={setOpen} className="w-full max-w-sm space-y-2">
      <div className="flex items-center justify-between space-x-4 px-1">
        <h4 className="text-sm font-semibold">@peduarte starred 3 repositories</h4>
        <CollapsibleTrigger render={<Button variant="ghost" size="sm" className="w-9 p-0" />}>
          <ChevronsUpDown className="size-4" />
          <span className="sr-only">Toggle</span>
        </CollapsibleTrigger>
      </div>
      <div className="rounded-md border px-4 py-3 font-mono text-sm">
        @radix-ui/primitives
      </div>
      <CollapsibleContent className="space-y-2">
        <div className="rounded-md border px-4 py-3 font-mono text-sm">
          @radix-ui/colors
        </div>
        <div className="rounded-md border px-4 py-3 font-mono text-sm">
          @stitches/react
        </div>
      </CollapsibleContent>
    </Collapsible>
  )
}

function CollapsibleSettingsDemo() {
  const [open, setOpen] = React.useState(false)

  return (
    <Card className="w-full max-w-sm">
      <CardHeader>
        <CardTitle>Notifications</CardTitle>
        <CardDescription>
          Choose what you want to be notified about.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <Collapsible open={open} onOpenChange={setOpen} className="space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium">Advanced settings</span>
            <CollapsibleTrigger render={<Button variant="ghost" size="sm" />}>
              {open ? "Hide" : "Show"}
            </CollapsibleTrigger>
          </div>
          <CollapsibleContent className="space-y-3 pt-2">
            <FieldGroup>
              <Field>
                <FieldLabel>Webhook URL</FieldLabel>
                <Input placeholder="https://api.example.com/webhook" />
              </Field>
            </FieldGroup>
          </CollapsibleContent>
        </Collapsible>
      </CardContent>
    </Card>
  )
}

type FileTreeItem = { name: string; items?: FileTreeItem[] }

function FileTreeNode({ item }: { item: FileTreeItem }) {
  const [open, setOpen] = React.useState(false)

  if (!item.items) {
    return (
      <div className="flex items-center gap-2 py-1 text-sm text-muted-foreground">
        <File className="size-4" />
        <span>{item.name}</span>
      </div>
    )
  }

  return (
    <Collapsible open={open} onOpenChange={setOpen} className="w-full">
      <CollapsibleTrigger className="flex w-full items-center gap-2 py-1 text-sm font-medium hover:underline text-left">
        <ChevronRight className={`size-4 transition-transform ${open ? "rotate-90" : ""}`} />
        <Folder className="size-4" />
        <span>{item.name}</span>
      </CollapsibleTrigger>
      <CollapsibleContent className="pl-6 space-y-1">
        {item.items.map((child) => (
          <FileTreeNode key={child.name} item={child} />
        ))}
      </CollapsibleContent>
    </Collapsible>
  )
}

function CollapsibleFileTreeDemo() {
  const fileTree: FileTreeItem[] = [
    {
      name: "components",
      items: [
        {
          name: "ui",
          items: [
            { name: "button.tsx" },
            { name: "card.tsx" },
            { name: "dialog.tsx" },
          ],
        },
        { name: "navbar.tsx" },
      ],
    },
    {
      name: "lib",
      items: [{ name: "utils.ts" }],
    },
    { name: "package.json" },
  ]

  return (
    <div className="w-full max-w-sm rounded-lg border p-4">
      <div className="space-y-1">
        {fileTree.map((item) => (
          <FileTreeNode key={item.name} item={item} />
        ))}
      </div>
    </div>
  )
}

export function CollapsibleSection() {
  return (
<Section
        id="collapsible"
        title="Collapsible"
        group="examples"
        description="An interactive component which expands/collapses a panel."
        fileKey="data"
      >
      {/* ========================================================================= */}
      {/* Collapsible                                                               */}
      {/* ========================================================================= */}
      <Demo
        title="Collapsible · Composition"
        description="Composes a collapsible region from its parts."      >
        <Collapsible className="w-full max-w-sm space-y-2">
          <CollapsibleTrigger render={<Button variant="outline" className="w-full justify-between" />}>
            Can I use this in my project?
            <ChevronDown className="size-4" />
          </CollapsibleTrigger>
          <CollapsibleContent className="rounded-md border p-3 text-sm text-muted-foreground">
            Yes. Free to use for personal and commercial projects. No attribution
            required.
          </CollapsibleContent>
        </Collapsible>
      </Demo>

      <Demo
        title="Collapsible · Controlled State"
        description="Controls the collapsible open state."      >
        <CollapsibleControlledDemo />
      </Demo>

      <Demo
        title="Collapsible · Basic"
        description="A simple collapsible content region."      >
        <Collapsible className="w-full max-w-sm space-y-2">
          <div className="flex items-center justify-between space-x-4 px-1">
            <h4 className="text-sm font-semibold">Notifications</h4>
            <CollapsibleTrigger render={<Button variant="ghost" size="sm" className="w-9 p-0" />}>
              <ChevronsUpDown className="size-4" />
              <span className="sr-only">Toggle</span>
            </CollapsibleTrigger>
          </div>
          <div className="rounded-md border px-4 py-3 font-mono text-sm">
            Default channel notifications enabled
          </div>
          <CollapsibleContent className="space-y-2">
            <div className="rounded-md border px-4 py-3 font-mono text-sm">
              Email digests sent weekly
            </div>
            <div className="rounded-md border px-4 py-3 font-mono text-sm">
              Push notifications active
            </div>
          </CollapsibleContent>
        </Collapsible>
      </Demo>

      <Demo
        title="Collapsible · Settings Panel"
        description="Builds a settings panel with collapsible sections."      >
        <CollapsibleSettingsDemo />
      </Demo>

      <Demo
        title="Collapsible · File Tree"
        description="Renders a nested file tree with collapsible folders."      >
        <CollapsibleFileTreeDemo />
      </Demo>

      </Section>
  )
}
