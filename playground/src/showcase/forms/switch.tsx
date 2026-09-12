import { Field, FieldContent, FieldDescription, FieldLabel, FieldTitle } from "@/components/ui/field"
import { Switch } from "@/components/ui/switch"
import { Demo, Section } from "../shared"

export function SwitchSection() {
  return (
<Section
        id="switch"
        title="Switch"
        group="examples"
        description="A control that allows the user to toggle between checked and not checked."
        fileKey="forms"
      >
      {/* ============ Switch · Official H2 demos ============ */}
      <Demo
        title="Switch · Description"
        description="FieldLabel composes a title and description."      >
        <Field orientation="horizontal">
          <Switch id="sw-d" defaultChecked />
          <FieldLabel htmlFor="sw-d">
            <FieldTitle>Dark mode</FieldTitle>
            <FieldDescription>Switch between light and dark themes.</FieldDescription>
          </FieldLabel>
        </Field>
      </Demo>

      <Demo
        title="Switch · Choice Card"
        description="A switch rendered as a choice card."      >
        <FieldLabel>
          <Field orientation="horizontal">
            <Switch />
            <FieldContent>
              <FieldTitle>Pro workspace</FieldTitle>
              <FieldDescription>Enable advanced collaboration features.</FieldDescription>
            </FieldContent>
          </Field>
        </FieldLabel>
      </Demo>

      <Demo
        title="Switch · Disabled"
        description="Disabled switches are non-interactive and dimmed."      >
        <div className="flex flex-col gap-3">
          <label className="flex items-center justify-between gap-2 text-sm opacity-50">
            Auto-sync
            <Switch disabled />
          </label>
          <label className="flex items-center justify-between gap-2 text-sm opacity-50">
            Auto-sync
            <Switch disabled defaultChecked />
          </label>
        </div>
      </Demo>

      <Demo
        title="Switch · Invalid"
        description="Use aria-invalid to mark a switch as invalid."      >
        <Field data-invalid="true" orientation="horizontal">
          <Switch aria-invalid id="sw-inv" />
          <FieldLabel htmlFor="sw-inv">Unsaved settings</FieldLabel>
        </Field>
      </Demo>

      <Demo
        title="Switch · Size"
        description="Switch size can be scaled with utility classes."      >
        <div className="flex items-center gap-4">
          <Switch className="scale-75" defaultChecked />
          <Switch defaultChecked />
          <Switch className="scale-125" defaultChecked />
        </div>
      </Demo>

      </Section>
  )
}
