import { AlertCircle } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Alert, AlertAction, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Demo, Section } from "../shared"

export function AlertSection() {
  return (
<Section
        id="alert"
        title="Alert"
        group="examples"
        description="Displays a callout for user attention."
        fileKey="basics"
      >

      <Demo
        title="Alert · Basic"
        description="Alert displays a short, important message."      >
        <div className="w-full space-y-3">
          <Alert>
            <AlertCircle />
            <AlertTitle>Heads up!</AlertTitle>
            <AlertDescription>
              You can add components to your app using the cli.
            </AlertDescription>
          </Alert>
        </div>
      </Demo>

      <Demo
        title="Alert · Destructive"
        description="Use the destructive variant for error messages."      >
        <div className="w-full space-y-3">
          <Alert variant="destructive">
            <AlertCircle />
            <AlertTitle>Unable to process your payment.</AlertTitle>
            <AlertDescription>
              Please verify your billing information and try again.
            </AlertDescription>
          </Alert>
        </div>
      </Demo>

      <Demo
        title="Alert · Action"
        description="AlertAction renders buttons aligned with the alert."      >
        <div className="w-full space-y-3">
          <Alert>
            <AlertCircle />
            <AlertTitle>New update available.</AlertTitle>
            <AlertDescription>
              A new version of the app is ready to install.
            </AlertDescription>
            <AlertAction>
              <Button size="sm" variant="outline">Later</Button>
              <Button size="sm">Install</Button>
            </AlertAction>
          </Alert>
          <Alert variant="destructive">
            <AlertCircle />
            <AlertTitle>Delete this account?</AlertTitle>
            <AlertDescription>
              This will permanently remove the account and all data.
            </AlertDescription>
            <AlertAction>
              <Button size="sm" variant="ghost">Cancel</Button>
              <Button size="sm" variant="destructive">Delete</Button>
            </AlertAction>
          </Alert>
        </div>
      </Demo>

      <Demo
        title="Alert · Custom Colors"
        description="Alert colors can be overridden with utility classes."      >
        <div className="w-full space-y-3">
          <Alert className="border-blue-500 bg-blue-50 text-blue-900 dark:bg-blue-950 dark:text-blue-100 *:data-[slot=alert-description]:text-blue-700 dark:*:data-[slot=alert-description]:text-blue-200">
            <AlertCircle />
            <AlertTitle>Info</AlertTitle>
            <AlertDescription>This is an info alert with custom colors.</AlertDescription>
          </Alert>
          <Alert className="border-emerald-500 bg-emerald-50 text-emerald-900 dark:bg-emerald-950 dark:text-emerald-100 *:data-[slot=alert-description]:text-emerald-700 dark:*:data-[slot=alert-description]:text-emerald-200">
            <AlertCircle />
            <AlertTitle>Success</AlertTitle>
            <AlertDescription>This is a success alert with custom colors.</AlertDescription>
          </Alert>
          <Alert className="border-amber-500 bg-amber-50 text-amber-900 dark:bg-amber-950 dark:text-amber-100 *:data-[slot=alert-description]:text-amber-700 dark:*:data-[slot=alert-description]:text-amber-200">
            <AlertCircle />
            <AlertTitle>Warning</AlertTitle>
            <AlertDescription>This is a warning alert with custom colors.</AlertDescription>
          </Alert>
        </div>
      </Demo>
      </Section>
  )
}
