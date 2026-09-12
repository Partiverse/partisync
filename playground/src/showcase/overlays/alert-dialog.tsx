import { Bluetooth, CircleFadingPlus, Trash2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogMedia, AlertDialogTitle, AlertDialogTrigger } from "@/components/ui/alert-dialog"
import { Demo, Section } from "../shared"

export function AlertDialogSection() {
  return (
<Section
        id="alert-dialog"
        title="Alert Dialog"
        group="examples"
        description="A modal dialog that interrupts the user with important content and expects a response."
        fileKey="overlays"
      >
      {/* ========================================================= */}
      {/* ===================== ALERT DIALOG ====================== */}
      {/* ========================================================= */}
      <Demo
        title="AlertDialog · Composition"
        description="An alert dialog composed with a title, description, and cancel/continue actions."
        center
      >
        <AlertDialog>
          <AlertDialogTrigger render={<Button variant="outline">Show Dialog</Button>} />
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Are you absolutely sure?</AlertDialogTitle>
              <AlertDialogDescription>
                This action cannot be undone. This will permanently delete your
                account from our servers.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Cancel</AlertDialogCancel>
              <AlertDialogAction>Continue</AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </Demo>

      <Demo
        title="AlertDialog · Basic"
        description="A basic alert dialog that asks the user to confirm an irreversible action."
        center
      >
        <AlertDialog>
          <AlertDialogTrigger render={<Button variant="outline">Show Dialog</Button>} />
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Are you absolutely sure?</AlertDialogTitle>
              <AlertDialogDescription>
                This action cannot be undone. This will permanently delete your
                account and remove your data from our servers.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Cancel</AlertDialogCancel>
              <AlertDialogAction>Continue</AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </Demo>

      <Demo
        title="AlertDialog · Small"
        description="A small alert dialog for quick confirmations like connecting an accessory."
        center
      >
        <AlertDialog>
          <AlertDialogTrigger render={<Button variant="outline">Show Dialog</Button>} />
          <AlertDialogContent size="sm">
            <AlertDialogHeader>
              <AlertDialogTitle>Allow accessory to connect?</AlertDialogTitle>
              <AlertDialogDescription>
                Do you want to allow the USB accessory to connect to this device?
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Don't allow</AlertDialogCancel>
              <AlertDialogAction>Allow</AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </Demo>

      <Demo
        title="AlertDialog · Media"
        description="An alert dialog with a media icon displayed above the title."
        center
      >
        <AlertDialog>
          <AlertDialogTrigger render={<Button variant="outline">Share Project</Button>} />
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogMedia>
                <CircleFadingPlus />
              </AlertDialogMedia>
              <AlertDialogTitle>Share this project?</AlertDialogTitle>
              <AlertDialogDescription>
                Anyone with the link will be able to view and edit this project.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Cancel</AlertDialogCancel>
              <AlertDialogAction>Share</AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </Demo>

      <Demo
        title="AlertDialog · Small with Media"
        description="A small alert dialog that combines a media icon with a compact layout."
        center
      >
        <AlertDialog>
          <AlertDialogTrigger render={<Button variant="outline">Show Dialog</Button>} />
          <AlertDialogContent size="sm">
            <AlertDialogHeader>
              <AlertDialogMedia>
                <Bluetooth />
              </AlertDialogMedia>
              <AlertDialogTitle>Allow accessory to connect?</AlertDialogTitle>
              <AlertDialogDescription>
                Do you want to allow the USB accessory to connect to this device?
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Don't allow</AlertDialogCancel>
              <AlertDialogAction>Allow</AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </Demo>

      <Demo
        title="AlertDialog · Destructive"
        description="A destructive alert dialog with a tinted media icon and a destructive action."
        center
      >
        <AlertDialog>
          <AlertDialogTrigger render={<Button variant="destructive">Delete Chat</Button>} />
          <AlertDialogContent size="sm">
            <AlertDialogHeader>
              <AlertDialogMedia className="bg-destructive/10 text-destructive dark:bg-destructive/20 dark:text-destructive">
                <Trash2 />
              </AlertDialogMedia>
              <AlertDialogTitle>Delete chat?</AlertDialogTitle>
              <AlertDialogDescription>
                This will permanently delete this chat conversation. View{" "}
                <a href="#" className="underline underline-offset-3 hover:text-foreground">Settings</a> delete any memories saved during this chat.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel variant="outline">Cancel</AlertDialogCancel>
              <AlertDialogAction variant="destructive">Delete</AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </Demo>

      </Section>
  )
}
