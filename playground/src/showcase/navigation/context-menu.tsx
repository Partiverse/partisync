import * as React from "react"
import { Copy, Download, Share2, Trash2 } from "lucide-react"
import { ContextMenu, ContextMenuCheckboxItem, ContextMenuContent, ContextMenuGroup, ContextMenuItem, ContextMenuLabel, ContextMenuRadioGroup, ContextMenuRadioItem, ContextMenuSeparator, ContextMenuShortcut, ContextMenuSub, ContextMenuSubContent, ContextMenuSubTrigger, ContextMenuTrigger } from "@/components/ui/context-menu"
import { Demo, Section } from "../shared"

export function ContextMenuSection() {
  const [ctxBookmarks, setCtxBookmarks] = React.useState(false)
  const [ctxUrls, setCtxUrls] = React.useState(false)
  const [ctxPerson, setCtxPerson] = React.useState("pedro")

  return (
<Section
        id="context-menu"
        title="Context Menu"
        group="examples"
        description="Displays a menu of actions triggered by a right click."
        fileKey="navigation"
      >
      {/* ========================================================================= */}
      {/* CONTEXT MENU                                                              */}
      {/* ========================================================================= */}

      {/* ContextMenu · Composition */}
      <Demo
        title="ContextMenu · Composition"
        description="A context menu with items, a submenu, checkboxes, and a radio group."
      >
        <ContextMenu>
          <ContextMenuTrigger className="flex h-[150px] w-full items-center justify-center rounded-md border border-dashed text-sm">
            Right click here
          </ContextMenuTrigger>
          <ContextMenuContent className="w-64">
            <ContextMenuGroup>
              <ContextMenuItem inset>
                Back <ContextMenuShortcut>⌘[</ContextMenuShortcut>
              </ContextMenuItem>
              <ContextMenuItem inset disabled>
                Forward <ContextMenuShortcut>⌘]</ContextMenuShortcut>
              </ContextMenuItem>
              <ContextMenuItem inset>
                Reload <ContextMenuShortcut>⌘R</ContextMenuShortcut>
              </ContextMenuItem>
            </ContextMenuGroup>
            <ContextMenuSub>
              <ContextMenuSubTrigger inset>More Tools</ContextMenuSubTrigger>
              <ContextMenuSubContent className="w-48">
                <ContextMenuGroup>
                  <ContextMenuItem>
                    Save Page As... <ContextMenuShortcut>⇧⌘S</ContextMenuShortcut>
                  </ContextMenuItem>
                  <ContextMenuItem>Create Shortcut...</ContextMenuItem>
                  <ContextMenuItem>Name Window...</ContextMenuItem>
                </ContextMenuGroup>
                <ContextMenuSeparator />
                <ContextMenuGroup>
                  <ContextMenuItem>Developer Tools</ContextMenuItem>
                </ContextMenuGroup>
              </ContextMenuSubContent>
            </ContextMenuSub>
            <ContextMenuSeparator />
            <ContextMenuGroup>
              <ContextMenuCheckboxItem
                checked={ctxBookmarks}
                onCheckedChange={setCtxBookmarks}
              >
                Show Bookmarks Bar <ContextMenuShortcut>⌘⇧B</ContextMenuShortcut>
              </ContextMenuCheckboxItem>
              <ContextMenuCheckboxItem
                checked={ctxUrls}
                onCheckedChange={setCtxUrls}
              >
                Show Full URLs
              </ContextMenuCheckboxItem>
            </ContextMenuGroup>
            <ContextMenuSeparator />
            <ContextMenuRadioGroup value={ctxPerson} onValueChange={setCtxPerson}>
              <ContextMenuLabel inset>People</ContextMenuLabel>
              <ContextMenuSeparator />
              <ContextMenuRadioItem value="pedro">Pedro Duarte</ContextMenuRadioItem>
              <ContextMenuRadioItem value="colm">Colm Tuite</ContextMenuRadioItem>
            </ContextMenuRadioGroup>
          </ContextMenuContent>
        </ContextMenu>
      </Demo>

      {/* ContextMenu · Basic */}
      <Demo
        title="ContextMenu · Basic"
        description="A basic context menu with a single group of actions."
      >
        <ContextMenu>
          <ContextMenuTrigger className="flex h-[120px] w-full items-center justify-center rounded-md border border-dashed text-sm">
            Right click for basic actions
          </ContextMenuTrigger>
          <ContextMenuContent className="w-48">
            <ContextMenuGroup>
              <ContextMenuItem>Copy Link</ContextMenuItem>
              <ContextMenuItem>Save Image</ContextMenuItem>
              <ContextMenuItem>Inspect</ContextMenuItem>
            </ContextMenuGroup>
          </ContextMenuContent>
        </ContextMenu>
      </Demo>

      {/* ContextMenu · Submenu */}
      <Demo
        title="ContextMenu · Submenu"
        description="A context menu containing a nested submenu."
      >
        <ContextMenu>
          <ContextMenuTrigger className="flex h-[120px] w-full items-center justify-center rounded-md border border-dashed text-sm">
            Right click for submenus
          </ContextMenuTrigger>
          <ContextMenuContent className="w-48">
            <ContextMenuGroup>
              <ContextMenuItem>Open</ContextMenuItem>
              <ContextMenuSub>
                <ContextMenuSubTrigger>Share</ContextMenuSubTrigger>
                <ContextMenuSubContent className="w-40">
                  <ContextMenuGroup>
                    <ContextMenuItem>AirDrop</ContextMenuItem>
                    <ContextMenuItem>Messages</ContextMenuItem>
                    <ContextMenuItem>Mail</ContextMenuItem>
                  </ContextMenuGroup>
                </ContextMenuSubContent>
              </ContextMenuSub>
            </ContextMenuGroup>
          </ContextMenuContent>
        </ContextMenu>
      </Demo>

      {/* ContextMenu · Shortcuts */}
      <Demo
        title="ContextMenu · Shortcuts"
        description="A context menu listing items with keyboard shortcuts."
      >
        <ContextMenu>
          <ContextMenuTrigger className="flex h-[120px] w-full items-center justify-center rounded-md border border-dashed text-sm">
            Right click to see keyboard shortcuts
          </ContextMenuTrigger>
          <ContextMenuContent className="w-52">
            <ContextMenuGroup>
              <ContextMenuItem>
                Cut <ContextMenuShortcut>⌘X</ContextMenuShortcut>
              </ContextMenuItem>
              <ContextMenuItem>
                Copy <ContextMenuShortcut>⌘C</ContextMenuShortcut>
              </ContextMenuItem>
              <ContextMenuItem>
                Paste <ContextMenuShortcut>⌘V</ContextMenuShortcut>
              </ContextMenuItem>
            </ContextMenuGroup>
          </ContextMenuContent>
        </ContextMenu>
      </Demo>

      {/* ContextMenu · Groups */}
      <Demo
        title="ContextMenu · Groups"
        description="A context menu organizing items into labeled, separated groups."
      >
        <ContextMenu>
          <ContextMenuTrigger className="flex h-[120px] w-full items-center justify-center rounded-md border border-dashed text-sm">
            Right click for grouped items
          </ContextMenuTrigger>
          <ContextMenuContent className="w-52">
            <ContextMenuGroup>
              <ContextMenuLabel>Editing</ContextMenuLabel>
              <ContextMenuItem>Select All</ContextMenuItem>
              <ContextMenuItem>Invert Selection</ContextMenuItem>
            </ContextMenuGroup>
            <ContextMenuSeparator />
            <ContextMenuGroup>
              <ContextMenuLabel>View</ContextMenuLabel>
              <ContextMenuItem>Zoom In</ContextMenuItem>
              <ContextMenuItem>Zoom Out</ContextMenuItem>
            </ContextMenuGroup>
          </ContextMenuContent>
        </ContextMenu>
      </Demo>

      {/* ContextMenu · Icons */}
      <Demo
        title="ContextMenu · Icons"
        description="A context menu whose items display leading icons."
      >
        <ContextMenu>
          <ContextMenuTrigger className="flex h-[120px] w-full items-center justify-center rounded-md border border-dashed text-sm">
            Right click for icon options
          </ContextMenuTrigger>
          <ContextMenuContent className="w-48">
            <ContextMenuGroup>
              <ContextMenuItem>
                <Copy className="size-4" />
                <span>Duplicate</span>
              </ContextMenuItem>
              <ContextMenuItem>
                <Download className="size-4" />
                <span>Download</span>
              </ContextMenuItem>
              <ContextMenuItem>
                <Share2 className="size-4" />
                <span>Share</span>
              </ContextMenuItem>
            </ContextMenuGroup>
          </ContextMenuContent>
        </ContextMenu>
      </Demo>

      {/* ContextMenu · Checkboxes */}
      <Demo
        title="ContextMenu · Checkboxes"
        description="A context menu with checkbox items for toggling layers."
      >
        <ContextMenu>
          <ContextMenuTrigger className="flex h-[120px] w-full items-center justify-center rounded-md border border-dashed text-sm">
            Right click for toggle options
          </ContextMenuTrigger>
          <ContextMenuContent className="w-52">
            <ContextMenuGroup>
              <ContextMenuLabel>Layers</ContextMenuLabel>
              <ContextMenuCheckboxItem checked={ctxBookmarks} onCheckedChange={setCtxBookmarks}>
                Grid
              </ContextMenuCheckboxItem>
              <ContextMenuCheckboxItem checked={ctxUrls} onCheckedChange={setCtxUrls}>
                Rulers
              </ContextMenuCheckboxItem>
            </ContextMenuGroup>
          </ContextMenuContent>
        </ContextMenu>
      </Demo>

      {/* ContextMenu · Radio */}
      <Demo
        title="ContextMenu · Radio"
        description="A context menu with a radio group for choosing one alignment."
      >
        <ContextMenu>
          <ContextMenuTrigger className="flex h-[120px] w-full items-center justify-center rounded-md border border-dashed text-sm">
            Right click to choose alignment
          </ContextMenuTrigger>
          <ContextMenuContent className="w-48">
            <ContextMenuRadioGroup value={ctxPerson} onValueChange={setCtxPerson}>
              <ContextMenuLabel>Alignment</ContextMenuLabel>
              <ContextMenuRadioItem value="left">Left</ContextMenuRadioItem>
              <ContextMenuRadioItem value="center">Center</ContextMenuRadioItem>
              <ContextMenuRadioItem value="right">Right</ContextMenuRadioItem>
            </ContextMenuRadioGroup>
          </ContextMenuContent>
        </ContextMenu>
      </Demo>

      {/* ContextMenu · Destructive */}
      <Demo
        title="ContextMenu · Destructive"
        description="A context menu separating destructive actions into their own group."
      >
        <ContextMenu>
          <ContextMenuTrigger className="flex h-[120px] w-full items-center justify-center rounded-md border border-dashed text-sm">
            Right click for destructive actions
          </ContextMenuTrigger>
          <ContextMenuContent className="w-48">
            <ContextMenuGroup>
              <ContextMenuItem>Inspect</ContextMenuItem>
              <ContextMenuItem>Rename</ContextMenuItem>
            </ContextMenuGroup>
            <ContextMenuSeparator />
            <ContextMenuGroup>
              <ContextMenuItem variant="destructive">
                <Trash2 className="size-4" />
                <span>Delete</span>
              </ContextMenuItem>
            </ContextMenuGroup>
          </ContextMenuContent>
        </ContextMenu>
      </Demo>

      {/* ContextMenu · Sides */}
      <Demo
        title="ContextMenu · Sides"
        description="A context menu with aligned content positioning."
      >
        <ContextMenu>
          <ContextMenuTrigger className="flex h-[120px] w-full items-center justify-center rounded-md border border-dashed text-sm">
            Right click (aligned content)
          </ContextMenuTrigger>
          <ContextMenuContent className="w-48">
            <ContextMenuGroup>
              <ContextMenuItem>Top Aligned</ContextMenuItem>
              <ContextMenuItem>Bottom Aligned</ContextMenuItem>
              <ContextMenuItem>Custom Inset</ContextMenuItem>
            </ContextMenuGroup>
          </ContextMenuContent>
        </ContextMenu>
      </Demo>

      </Section>
  )
}
