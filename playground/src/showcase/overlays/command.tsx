import * as React from "react"
import { Bell, Calculator, Calendar, ClipboardPaste, Code, Copy, CreditCard, FileText, Folder, FolderPlus, HelpCircle, Home, Image as ImageIcon, Inbox, LayoutGrid, List, Plus, Scissors, Settings, Smile, Trash, User, ZoomIn, ZoomOut } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Command, CommandDialog, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList, CommandSeparator, CommandShortcut } from "@/components/ui/command"
import { Demo, Section } from "../shared"

export function CommandSection() {
  const [cmdBasicOpen, setCmdBasicOpen] = React.useState(false)
  const [cmdShortcutsOpen, setCmdShortcutsOpen] = React.useState(false)
  const [cmdGroupsOpen, setCmdGroupsOpen] = React.useState(false)
  const [cmdScrollableOpen, setCmdScrollableOpen] = React.useState(false)

  return (
<Section
        id="command"
        title="Command"
        group="examples"
        description="Command menu for search and quick actions."
        fileKey="overlays"
      >
      {/* ========================================================= */}
      {/* ======================== COMMAND ======================== */}
      {/* ========================================================= */}
      <Demo
        title="Command · Composition"
        description="A command menu composed with an input, groups, separators, and shortcuts."
        className="flex justify-center"
      >
        <Command className="max-w-sm rounded-lg border">
          <CommandInput placeholder="Type a command or search..." />
          <CommandList>
            <CommandEmpty>No results found.</CommandEmpty>
            <CommandGroup heading="Suggestions">
              <CommandItem>
                <Calendar />
                <span>Calendar</span>
              </CommandItem>
              <CommandItem>
                <Smile />
                <span>Search Emoji</span>
              </CommandItem>
              <CommandItem disabled>
                <Calculator />
                <span>Calculator</span>
              </CommandItem>
            </CommandGroup>
            <CommandSeparator />
            <CommandGroup heading="Settings">
              <CommandItem>
                <User />
                <span>Profile</span>
                <CommandShortcut>⌘P</CommandShortcut>
              </CommandItem>
              <CommandItem>
                <CreditCard />
                <span>Billing</span>
                <CommandShortcut>⌘B</CommandShortcut>
              </CommandItem>
              <CommandItem>
                <Settings />
                <span>Settings</span>
                <CommandShortcut>⌘S</CommandShortcut>
              </CommandItem>
            </CommandGroup>
          </CommandList>
        </Command>
      </Demo>

      <Demo
        title="Command · Basic"
        description="A command dialog opened from a button with a single suggestion group."
        center
      >
        <div className="flex flex-col gap-4">
          <Button onClick={() => setCmdBasicOpen(true)} variant="outline" className="w-fit">
            Open Menu
          </Button>
          <CommandDialog open={cmdBasicOpen} onOpenChange={setCmdBasicOpen}>
            <Command>
              <CommandInput placeholder="Type a command or search..." />
              <CommandList>
                <CommandEmpty>No results found.</CommandEmpty>
                <CommandGroup heading="Suggestions">
                  <CommandItem>Calendar</CommandItem>
                  <CommandItem>Search Emoji</CommandItem>
                  <CommandItem>Calculator</CommandItem>
                </CommandGroup>
              </CommandList>
            </Command>
          </CommandDialog>
        </div>
      </Demo>

      <Demo
        title="Command · Command Palette Shortcuts"
        description="A command palette whose items display keyboard shortcuts."
        center
      >
        <div className="flex flex-col gap-4">
          <Button onClick={() => setCmdShortcutsOpen(true)} variant="outline" className="w-fit">
            Open Menu
          </Button>
          <CommandDialog open={cmdShortcutsOpen} onOpenChange={setCmdShortcutsOpen}>
            <Command>
              <CommandInput placeholder="Type a command or search..." />
              <CommandList>
                <CommandEmpty>No results found.</CommandEmpty>
                <CommandGroup heading="Settings">
                  <CommandItem>
                    <User />
                    <span>Profile</span>
                    <CommandShortcut>⌘P</CommandShortcut>
                  </CommandItem>
                  <CommandItem>
                    <CreditCard />
                    <span>Billing</span>
                    <CommandShortcut>⌘B</CommandShortcut>
                  </CommandItem>
                  <CommandItem>
                    <Settings />
                    <span>Settings</span>
                    <CommandShortcut>⌘S</CommandShortcut>
                  </CommandItem>
                </CommandGroup>
              </CommandList>
            </Command>
          </CommandDialog>
        </div>
      </Demo>

      <Demo
        title="Command · Command Palette Groups"
        description="A command palette organizing items into multiple labeled groups."
        center
      >
        <div className="flex flex-col gap-4">
          <Button onClick={() => setCmdGroupsOpen(true)} variant="outline" className="w-fit">
            Open Menu
          </Button>
          <CommandDialog open={cmdGroupsOpen} onOpenChange={setCmdGroupsOpen}>
            <Command>
              <CommandInput placeholder="Type a command or search..." />
              <CommandList>
                <CommandEmpty>No results found.</CommandEmpty>
                <CommandGroup heading="Suggestions">
                  <CommandItem>
                    <Calendar />
                    <span>Calendar</span>
                  </CommandItem>
                  <CommandItem>
                    <Smile />
                    <span>Search Emoji</span>
                  </CommandItem>
                  <CommandItem>
                    <Calculator />
                    <span>Calculator</span>
                  </CommandItem>
                </CommandGroup>
                <CommandSeparator />
                <CommandGroup heading="Settings">
                  <CommandItem>
                    <User />
                    <span>Profile</span>
                    <CommandShortcut>⌘P</CommandShortcut>
                  </CommandItem>
                  <CommandItem>
                    <CreditCard />
                    <span>Billing</span>
                    <CommandShortcut>⌘B</CommandShortcut>
                  </CommandItem>
                  <CommandItem>
                    <Settings />
                    <span>Settings</span>
                    <CommandShortcut>⌘S</CommandShortcut>
                  </CommandItem>
                </CommandGroup>
              </CommandList>
            </Command>
          </CommandDialog>
        </div>
      </Demo>

      <Demo
        title="Command · Command Palette Scrollable"
        description="A scrollable command palette with navigation, actions, view, account, and tools groups."
        center
      >
        <div className="flex flex-col gap-4">
          <Button onClick={() => setCmdScrollableOpen(true)} variant="outline" className="w-fit">
            Open Menu
          </Button>
          <CommandDialog open={cmdScrollableOpen} onOpenChange={setCmdScrollableOpen}>
            <Command>
              <CommandInput placeholder="Type a command or search..." />
              <CommandList>
                <CommandEmpty>No results found.</CommandEmpty>
                <CommandGroup heading="Navigation">
                  <CommandItem>
                    <Home />
                    <span>Home</span>
                    <CommandShortcut>⌘H</CommandShortcut>
                  </CommandItem>
                  <CommandItem>
                    <Inbox />
                    <span>Inbox</span>
                    <CommandShortcut>⌘I</CommandShortcut>
                  </CommandItem>
                  <CommandItem>
                    <FileText />
                    <span>Documents</span>
                    <CommandShortcut>⌘D</CommandShortcut>
                  </CommandItem>
                  <CommandItem>
                    <Folder />
                    <span>Folders</span>
                    <CommandShortcut>⌘F</CommandShortcut>
                  </CommandItem>
                </CommandGroup>
                <CommandSeparator />
                <CommandGroup heading="Actions">
                  <CommandItem>
                    <Plus />
                    <span>New File</span>
                    <CommandShortcut>⌘N</CommandShortcut>
                  </CommandItem>
                  <CommandItem>
                    <FolderPlus />
                    <span>New Folder</span>
                    <CommandShortcut>⇧⌘N</CommandShortcut>
                  </CommandItem>
                  <CommandItem>
                    <Copy />
                    <span>Copy</span>
                    <CommandShortcut>⌘C</CommandShortcut>
                  </CommandItem>
                  <CommandItem>
                    <Scissors />
                    <span>Cut</span>
                    <CommandShortcut>⌘X</CommandShortcut>
                  </CommandItem>
                  <CommandItem>
                    <ClipboardPaste />
                    <span>Paste</span>
                    <CommandShortcut>⌘V</CommandShortcut>
                  </CommandItem>
                  <CommandItem>
                    <Trash />
                    <span>Delete</span>
                    <CommandShortcut>⌫</CommandShortcut>
                  </CommandItem>
                </CommandGroup>
                <CommandSeparator />
                <CommandGroup heading="View">
                  <CommandItem>
                    <LayoutGrid />
                    <span>Grid View</span>
                  </CommandItem>
                  <CommandItem>
                    <List />
                    <span>List View</span>
                  </CommandItem>
                  <CommandItem>
                    <ZoomIn />
                    <span>Zoom In</span>
                    <CommandShortcut>⌘+</CommandShortcut>
                  </CommandItem>
                  <CommandItem>
                    <ZoomOut />
                    <span>Zoom Out</span>
                    <CommandShortcut>⌘-</CommandShortcut>
                  </CommandItem>
                </CommandGroup>
                <CommandSeparator />
                <CommandGroup heading="Account">
                  <CommandItem>
                    <User />
                    <span>Profile</span>
                    <CommandShortcut>⌘P</CommandShortcut>
                  </CommandItem>
                  <CommandItem>
                    <CreditCard />
                    <span>Billing</span>
                    <CommandShortcut>⌘B</CommandShortcut>
                  </CommandItem>
                  <CommandItem>
                    <Settings />
                    <span>Settings</span>
                    <CommandShortcut>⌘S</CommandShortcut>
                  </CommandItem>
                  <CommandItem>
                    <Bell />
                    <span>Notifications</span>
                  </CommandItem>
                  <CommandItem>
                    <HelpCircle />
                    <span>Help & Support</span>
                  </CommandItem>
                </CommandGroup>
                <CommandSeparator />
                <CommandGroup heading="Tools">
                  <CommandItem>
                    <Calculator />
                    <span>Calculator</span>
                  </CommandItem>
                  <CommandItem>
                    <Calendar />
                    <span>Calendar</span>
                  </CommandItem>
                  <CommandItem>
                    <ImageIcon />
                    <span>Image Editor</span>
                  </CommandItem>
                  <CommandItem>
                    <Code />
                    <span>Code Editor</span>
                  </CommandItem>
                </CommandGroup>
              </CommandList>
            </Command>
          </CommandDialog>
        </div>
      </Demo>
      </Section>
  )
}
