import { Field, FieldDescription, FieldLabel, FieldTitle } from "@/components/ui/field"
import { Checkbox } from "@/components/ui/checkbox"
import { Demo, Section } from "../shared"

export function CheckboxSection() {
  return (
<Section
        id="checkbox"
        title="Checkbox"
        group="examples"
        description="A control that allows the user to toggle between checked and not checked."
        fileKey="forms"
      >
      {/* ============ Checkbox · Official H2 demos ============ */}
      <Demo
        title="Checkbox · Checked State"
        description="Checkbox supports checked and unchecked states."      >
        <div className="flex flex-col gap-3">
          <label className="flex items-center gap-2 text-sm">
            <Checkbox defaultChecked /> Checked
          </label>
          <label className="flex items-center gap-2 text-sm">
            <Checkbox /> Unchecked
          </label>
        </div>
      </Demo>

      <Demo
        title="Checkbox · Invalid State"
        description="Use aria-invalid to mark a checkbox as invalid."      >
        <label className="flex items-center gap-2 text-sm text-destructive">
          <Checkbox aria-invalid /> Terms not accepted
        </label>
      </Demo>

      <Demo
        title="Checkbox · Basic"
        description="A checkbox with a plain text label."      >
        <label className="flex items-center gap-2 text-sm">
          <Checkbox id="cb-basic" /> Accept terms and conditions
        </label>
      </Demo>

      <Demo
        title="Checkbox · Description"
        description="FieldLabel composes a title and description."      >
        <Field orientation="horizontal">
          <Checkbox id="cb-desc" />
          <FieldLabel htmlFor="cb-desc">
            <FieldTitle>Marketing emails</FieldTitle>
            <FieldDescription>Receive emails about new products and features.</FieldDescription>
          </FieldLabel>
        </Field>
      </Demo>

      <Demo
        title="Checkbox · Disabled"
        description="Disabled checkboxes are non-interactive and dimmed."      >
        <div className="flex flex-col gap-3">
          <label className="flex items-center gap-2 text-sm opacity-50">
            <Checkbox disabled /> Disabled unchecked
          </label>
          <label className="flex items-center gap-2 text-sm opacity-50">
            <Checkbox disabled defaultChecked /> Disabled checked
          </label>
        </div>
      </Demo>

      <Demo
        title="Checkbox · Group"
        description="Render a list of checkboxes from data."      >
        <div className="space-y-3">
          <p className="text-sm font-medium">Notification channels</p>
          {["Email", "SMS", "Push"].map((t) => (
            <label key={t} className="flex items-center gap-2 text-sm">
              <Checkbox defaultChecked={t === "Email"} /> {t}
            </label>
          ))}
        </div>
      </Demo>

      <Demo
        title="Checkbox · Table"
        description="Checkboxes select rows in a table."      >
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b">
              <th className="w-10 py-2"><Checkbox aria-label="Select all" /></th>
              <th className="py-2 text-left font-medium">Name</th>
              <th className="py-2 text-left font-medium">Status</th>
            </tr>
          </thead>
          <tbody>
            {[
              { n: "Alice", s: "Active" },
              { n: "Bob", s: "Invited" },
              { n: "Carol", s: "Active" },
            ].map(({ n, s }) => (
              <tr key={n} className="border-b last:border-0">
                <td className="py-2"><Checkbox aria-label={`Select ${n}`} /></td>
                <td className="py-2">{n}</td>
                <td className="py-2 text-muted-foreground">{s}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </Demo>

      </Section>
  )
}
