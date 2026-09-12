import { Button } from "@/components/ui/button"
import { Sheet, SheetClose, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle, SheetTrigger } from "@/components/ui/sheet"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Demo, Section } from "../shared"

const SHEET_SIDES = ["top", "right", "bottom", "left"] as const

export function SheetSection() {
  return (
<Section
        id="sheet"
        title="Sheet"
        group="examples"
        description="Extends the Dialog component to display content that complements the main content of the screen."
        fileKey="overlays"
      >
      {/* ========================================================= */}
      {/* ========================= SHEET ========================= */}
      {/* ========================================================= */}
      <Demo
        title="Sheet · Composition"
        description="A sheet composed with a header, form fields, and footer actions."
        center
      >
        <Sheet>
          <SheetTrigger render={<Button variant="outline">Open</Button>} />
          <SheetContent>
            <SheetHeader>
              <SheetTitle>Edit profile</SheetTitle>
              <SheetDescription>
                Make changes to your profile here. Click save when you're done.
              </SheetDescription>
            </SheetHeader>
            <div className="grid flex-1 auto-rows-min gap-6 px-4">
              <div className="grid gap-3">
                <Label htmlFor="sheet-demo-name">Name</Label>
                <Input id="sheet-demo-name" defaultValue="Pedro Duarte" />
              </div>
              <div className="grid gap-3">
                <Label htmlFor="sheet-demo-username">Username</Label>
                <Input id="sheet-demo-username" defaultValue="@peduarte" />
              </div>
            </div>
            <SheetFooter>
              <Button type="submit">Save changes</Button>
              <SheetClose render={<Button variant="outline">Close</Button>} />
            </SheetFooter>
          </SheetContent>
        </Sheet>
      </Demo>

      <Demo
        title="Sheet · Side"
        description="A sheet that slides in from the top, right, bottom, or left side."
        center
      >
        <div className="flex flex-wrap gap-2">
          {SHEET_SIDES.map((side) => (
            <Sheet key={side}>
              <SheetTrigger render={<Button variant="outline" className="capitalize">{side}</Button>} />
              <SheetContent
                side={side}
                className="data-[side=bottom]:max-h-[50vh] data-[side=top]:max-h-[50vh]"
              >
                <SheetHeader>
                  <SheetTitle>Edit profile</SheetTitle>
                  <SheetDescription>
                    Make changes to your profile here. Click save when you're done.
                  </SheetDescription>
                </SheetHeader>
                <div className="no-scrollbar overflow-y-auto px-4">
                  {Array.from({ length: 10 }).map((_, index) => (
                    <p key={index} className="mb-2 leading-relaxed">
                      Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed
                      do eiusmod tempor incididunt ut labore et dolore magna aliqua.
                      Ut enim ad minim veniam, quis nostrud exercitation ullamco
                      laboris nisi ut aliquip ex ea commodo consequat. Duis aute
                      irure dolor in reprehenderit in voluptate velit esse cillum
                      dolore eu fugiat nulla pariatur. Excepteur sint occaecat
                      cupidatat non proident, sunt in culpa qui officia deserunt
                      mollit anim id est laborum.
                    </p>
                  ))}
                </div>
                <SheetFooter>
                  <Button type="submit">Save changes</Button>
                  <SheetClose render={<Button variant="outline">Cancel</Button>} />
                </SheetFooter>
              </SheetContent>
            </Sheet>
          ))}
        </div>
      </Demo>

      <Demo
        title="Sheet · No Close Button"
        description="A sheet without the default close button; click outside to dismiss."
        center
      >
        <Sheet>
          <SheetTrigger render={<Button variant="outline">Open Sheet</Button>} />
          <SheetContent showCloseButton={false}>
            <SheetHeader>
              <SheetTitle>No Close Button</SheetTitle>
              <SheetDescription>
                This sheet doesn't have a close button in the top-right corner. Click outside to close.
              </SheetDescription>
            </SheetHeader>
          </SheetContent>
        </Sheet>
      </Demo>

      </Section>
  )
}
