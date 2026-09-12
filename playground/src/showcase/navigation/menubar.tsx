import * as React from "react"
import { Pause, Play, Volume2, VolumeX } from "lucide-react"
import { Menubar, MenubarCheckboxItem, MenubarContent, MenubarItem, MenubarMenu, MenubarRadioGroup, MenubarRadioItem, MenubarSeparator, MenubarShortcut, MenubarSub, MenubarSubContent, MenubarSubTrigger, MenubarTrigger } from "@/components/ui/menubar"
import { DropdownMenuGroup } from "@/components/ui/dropdown-menu"
import { Demo, Section } from "../shared"

export function MenubarSection() {
  const [showBookmarks, setShowBookmarks] = React.useState(true)
  const [showUrls, setShowUrls] = React.useState(false)
  const [menubarProfile, setMenubarProfile] = React.useState("benoit")

  return (
<Section
        id="menubar"
        title="Menubar"
        group="examples"
        description="A visually persistent menu common in desktop applications that provides quick access to a consistent set of commands."
        fileKey="navigation"
      >
      {/* ========================================================================= */}
      {/* MENUBAR                                                                   */}
      {/* ========================================================================= */}

      {/* Menubar · Composition */}
      <Demo
        title="Menubar · Composition"
        description="A horizontal menubar with File, Edit, and View menus, submenus, and shortcuts."
      >
        <Menubar>
          <MenubarMenu>
            <MenubarTrigger>File</MenubarTrigger>
            <MenubarContent>
              <DropdownMenuGroup>
                <MenubarItem>
                  New Tab <MenubarShortcut>⌘T</MenubarShortcut>
                </MenubarItem>
                <MenubarItem>
                  New Window <MenubarShortcut>⌘N</MenubarShortcut>
                </MenubarItem>
                <MenubarItem disabled>New Incognito Window</MenubarItem>
              </DropdownMenuGroup>
              <MenubarSeparator />
              <MenubarSub>
                <MenubarSubTrigger>Share</MenubarSubTrigger>
                <MenubarSubContent>
                  <DropdownMenuGroup>
                    <MenubarItem>Email Link</MenubarItem>
                    <MenubarItem>Messages</MenubarItem>
                    <MenubarItem>Notes</MenubarItem>
                  </DropdownMenuGroup>
                </MenubarSubContent>
              </MenubarSub>
              <MenubarSeparator />
              <DropdownMenuGroup>
                <MenubarItem>
                  Print... <MenubarShortcut>⌘P</MenubarShortcut>
                </MenubarItem>
              </DropdownMenuGroup>
            </MenubarContent>
          </MenubarMenu>
          <MenubarMenu>
            <MenubarTrigger>Edit</MenubarTrigger>
            <MenubarContent>
              <DropdownMenuGroup>
                <MenubarItem>
                  Undo <MenubarShortcut>⌘Z</MenubarShortcut>
                </MenubarItem>
                <MenubarItem>
                  Redo <MenubarShortcut>⇧⌘Z</MenubarShortcut>
                </MenubarItem>
              </DropdownMenuGroup>
              <MenubarSeparator />
              <DropdownMenuGroup>
                <MenubarItem>Cut</MenubarItem>
                <MenubarItem>Copy</MenubarItem>
                <MenubarItem>Paste</MenubarItem>
              </DropdownMenuGroup>
            </MenubarContent>
          </MenubarMenu>
          <MenubarMenu>
            <MenubarTrigger>View</MenubarTrigger>
            <MenubarContent>
              <DropdownMenuGroup>
                <MenubarItem inset>Always Show Bookmarks Bar</MenubarItem>
                <MenubarItem inset>Always Show Full URLs</MenubarItem>
              </DropdownMenuGroup>
              <MenubarSeparator />
              <DropdownMenuGroup>
                <MenubarItem inset>Reload <MenubarShortcut>⌘R</MenubarShortcut></MenubarItem>
                <MenubarItem inset disabled>Force Reload <MenubarShortcut>⇧⌘R</MenubarShortcut></MenubarItem>
              </DropdownMenuGroup>
            </MenubarContent>
          </MenubarMenu>
        </Menubar>
      </Demo>

      {/* Menubar · Checkbox */}
      <Demo
        title="Menubar · Checkbox"
        description="A menubar with checkbox items that toggle view options."
      >
        <Menubar>
          <MenubarMenu>
            <MenubarTrigger>View</MenubarTrigger>
            <MenubarContent>
              <DropdownMenuGroup>
                <MenubarCheckboxItem
                  checked={showBookmarks}
                  onCheckedChange={setShowBookmarks}
                >
                  Always Show Bookmarks Bar
                </MenubarCheckboxItem>
                <MenubarCheckboxItem
                  checked={showUrls}
                  onCheckedChange={setShowUrls}
                >
                  Always Show Full URLs
                </MenubarCheckboxItem>
              </DropdownMenuGroup>
              <MenubarSeparator />
              <DropdownMenuGroup>
                <MenubarItem inset>
                  Reload <MenubarShortcut>⌘R</MenubarShortcut>
                </MenubarItem>
              </DropdownMenuGroup>
            </MenubarContent>
          </MenubarMenu>
        </Menubar>
      </Demo>

      {/* Menubar · Radio */}
      <Demo
        title="Menubar · Radio"
        description="A menubar with a radio group for selecting a single profile."
      >
        <Menubar>
          <MenubarMenu>
            <MenubarTrigger>Profiles</MenubarTrigger>
            <MenubarContent>
              <MenubarRadioGroup value={menubarProfile} onValueChange={setMenubarProfile}>
                <MenubarRadioItem value="andy">Andy</MenubarRadioItem>
                <MenubarRadioItem value="benoit">Benoit</MenubarRadioItem>
                <MenubarRadioItem value="Luis">Luis</MenubarRadioItem>
              </MenubarRadioGroup>
              <MenubarSeparator />
              <DropdownMenuGroup>
                <MenubarItem inset>Edit...</MenubarItem>
                <MenubarSeparator />
                <MenubarItem inset>Add Profile...</MenubarItem>
              </DropdownMenuGroup>
            </MenubarContent>
          </MenubarMenu>
        </Menubar>
      </Demo>

      {/* Menubar · Submenu */}
      <Demo
        title="Menubar · Submenu"
        description="A menubar containing a nested submenu of advanced tools."
      >
        <Menubar>
          <MenubarMenu>
            <MenubarTrigger>Actions</MenubarTrigger>
            <MenubarContent>
              <DropdownMenuGroup>
                <MenubarItem>Quick Search</MenubarItem>
              </DropdownMenuGroup>
              <MenubarSeparator />
              <MenubarSub>
                <MenubarSubTrigger>Advanced Tools</MenubarSubTrigger>
                <MenubarSubContent>
                  <DropdownMenuGroup>
                    <MenubarItem>Developer Tools</MenubarItem>
                    <MenubarItem>Task Manager</MenubarItem>
                    <MenubarItem>Inspect Elements</MenubarItem>
                  </DropdownMenuGroup>
                </MenubarSubContent>
              </MenubarSub>
              <MenubarSeparator />
              <DropdownMenuGroup>
                <MenubarItem>Settings</MenubarItem>
              </DropdownMenuGroup>
            </MenubarContent>
          </MenubarMenu>
        </Menubar>
      </Demo>

      {/* Menubar · With Icons */}
      <Demo
        title="Menubar · With Icons"
        description="A menubar whose items display leading icons."
      >
        <Menubar>
          <MenubarMenu>
            <MenubarTrigger>Media</MenubarTrigger>
            <MenubarContent>
              <DropdownMenuGroup>
                <MenubarItem>
                  <Play className="size-4" />
                  <span>Play</span>
                  <MenubarShortcut>Space</MenubarShortcut>
                </MenubarItem>
                <MenubarItem>
                  <Pause className="size-4" />
                  <span>Pause</span>
                </MenubarItem>
              </DropdownMenuGroup>
              <MenubarSeparator />
              <DropdownMenuGroup>
                <MenubarItem>
                  <Volume2 className="size-4" />
                  <span>Volume Up</span>
                </MenubarItem>
                <MenubarItem>
                  <VolumeX className="size-4" />
                  <span>Mute</span>
                </MenubarItem>
              </DropdownMenuGroup>
            </MenubarContent>
          </MenubarMenu>
        </Menubar>
      </Demo>

      </Section>
  )
}
