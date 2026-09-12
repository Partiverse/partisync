import { Button } from "@/components/ui/button"
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Field, FieldGroup } from "@/components/ui/field"
import { Demo, Section } from "../shared"

export function DialogSection() {
  return (
<Section
        id="dialog"
        title="Dialog"
        group="installation"
        description="A window overlaid on either the primary window or another dialog window, rendering the content underneath inert."
        fileKey="overlays"
      >
      {/* ========================================================= */}
      {/* ======================== DIALOG ========================= */}
      {/* ========================================================= */}
      <Demo
        title="Dialog · Composition"
        description="A dialog composed with a trigger, header, form fields, and footer actions."
        center
      >
        <Dialog>
          <form>
            <DialogTrigger render={<Button variant="outline">Open Dialog</Button>} />
            <DialogContent className="sm:max-w-sm">
              <DialogHeader>
                <DialogTitle>Edit profile</DialogTitle>
                <DialogDescription>
                  Make changes to your profile here. Click save when you're done.
                </DialogDescription>
              </DialogHeader>
              <FieldGroup>
                <Field>
                  <Label htmlFor="dialog-comp-name">Name</Label>
                  <Input id="dialog-comp-name" name="name" defaultValue="Pedro Duarte" />
                </Field>
                <Field>
                  <Label htmlFor="dialog-comp-username">Username</Label>
                  <Input id="dialog-comp-username" name="username" defaultValue="@peduarte" />
                </Field>
              </FieldGroup>
              <DialogFooter>
                <DialogClose render={<Button variant="outline">Cancel</Button>} />
                <Button type="submit">Save changes</Button>
              </DialogFooter>
            </DialogContent>
          </form>
        </Dialog>
      </Demo>

      <Demo
        title="Dialog · Custom Close Button"
        description="A share dialog with a custom close button placed at the start of the footer."
        center
      >
        <Dialog>
          <DialogTrigger render={<Button variant="outline">Share</Button>} />
          <DialogContent className="sm:max-w-md">
            <DialogHeader>
              <DialogTitle>Share link</DialogTitle>
              <DialogDescription>
                Anyone who has this link will be able to view this.
              </DialogDescription>
            </DialogHeader>
            <div className="flex items-center gap-2">
              <div className="grid flex-1 gap-2">
                <Label htmlFor="dialog-share-link" className="sr-only">
                  Link
                </Label>
                <Input
                  id="dialog-share-link"
                  defaultValue="https://ui.shadcn.com/docs/installation"
                  readOnly
                />
              </div>
            </div>
            <DialogFooter className="sm:justify-start">
              <DialogClose render={<Button type="button">Close</Button>} />
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </Demo>

      <Demo
        title="Dialog · No Close Button"
        description="A dialog without the default close button in the top-right corner."
        center
      >
        <Dialog>
          <DialogTrigger render={<Button variant="outline">No Close Button</Button>} />
          <DialogContent showCloseButton={false}>
            <DialogHeader>
              <DialogTitle>No Close Button</DialogTitle>
              <DialogDescription>
                This dialog doesn't have a close button in the top-right corner.
              </DialogDescription>
            </DialogHeader>
          </DialogContent>
        </Dialog>
      </Demo>

      <Demo
        title="Dialog · Sticky Footer"
        description="A dialog with a sticky footer that stays visible while the content scrolls."
        center
      >
        <Dialog>
          <DialogTrigger render={<Button variant="outline">Sticky Footer</Button>} />
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Sticky Footer</DialogTitle>
              <DialogDescription>
                This dialog has a sticky footer that stays visible while the content scrolls.
              </DialogDescription>
            </DialogHeader>
            <div className="-mx-4 no-scrollbar max-h-[50vh] overflow-y-auto px-4">
              {Array.from({ length: 10 }).map((_, index) => (
                <p key={index} className="mb-4 leading-normal">
                  Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do
                  eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut
                  enim ad minim veniam, quis nostrud exercitation ullamco laboris
                  nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in
                  reprehenderit in voluptate velit esse cillum dolore eu fugiat
                  nulla pariatur. Excepteur sint occaecat cupidatat non proident,
                  sunt in culpa qui officia deserunt mollit anim id est laborum.
                </p>
              ))}
            </div>
            <DialogFooter>
              <DialogClose render={<Button variant="outline">Close</Button>} />
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </Demo>

      <Demo
        title="Dialog · Scrollable Content"
        description="A dialog with a scrollable body constrained to a maximum height."
        center
      >
        <Dialog>
          <DialogTrigger render={<Button variant="outline">Scrollable Content</Button>} />
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Scrollable Content</DialogTitle>
              <DialogDescription>
                This is a dialog with scrollable content.
              </DialogDescription>
            </DialogHeader>
            <div className="-mx-4 no-scrollbar max-h-[50vh] overflow-y-auto px-4">
              {Array.from({ length: 10 }).map((_, index) => (
                <p key={index} className="mb-4 leading-normal">
                  Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do
                  eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut
                  enim ad minim veniam, quis nostrud exercitation ullamco laboris
                  nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in
                  reprehenderit in voluptate velit esse cillum dolore eu fugiat
                  nulla pariatur. Excepteur sint occaecat cupidatat non proident,
                  sunt in culpa qui officia deserunt mollit anim id est laborum.
                </p>
              ))}
            </div>
          </DialogContent>
        </Dialog>
      </Demo>

      </Section>
  )
}
