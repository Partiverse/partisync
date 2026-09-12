import { Separator } from "@/components/ui/separator"
import { Demo, Section } from "../shared"

export function SeparatorSection() {
  return (
<Section
        id="separator"
        title="Separator"
        group="examples"
        description="Visually or semantically separates content."
        fileKey="basics"
      >

      <Demo
        title="Separator · Vertical"
        description="Separator supports vertical orientation."      >
        <div className="flex w-full h-12 items-center gap-3">
          <div className="text-sm">Account</div>
          <Separator orientation="vertical" />
          <div className="text-sm">Billing</div>
          <Separator orientation="vertical" />
          <div className="text-sm">Notifications</div>
        </div>
      </Demo>

      <Demo
        title="Separator · Menu"
        description="Separator can divide the sections of a menu."      >
        <div className="w-full rounded-md border p-3 text-sm space-y-2">
          <div>Profile</div>
          <div>Billing</div>
          <Separator className="my-2" />
          <div>Settings</div>
          <div>Help</div>
          <Separator className="my-2" />
          <div>Logout</div>
        </div>
      </Demo>

      <Demo
        title="Separator · List"
        description="Use divide-y or Separator to divide the rows of a list."      >
        <ul className="w-full divide-y rounded-md border">
          {['Apple', 'Banana', 'Cherry', 'Date'].map((f) => (
            <li key={f} className="px-4 py-2 text-sm">{f}</li>
          ))}
        </ul>
      </Demo>
      </Section>
  )
}
