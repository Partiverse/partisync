import { Italic, Underline } from "lucide-react"
import { Toggle } from "@/components/ui/toggle"
import { Demo, Section } from "../shared"

export function ToggleSection() {
  return (
<Section
        id="toggle"
        title="Toggle"
        group="examples"
        description="A two-state button that can be either on or off."
        fileKey="data"
      >
      {/* ========================================================================= */}
      {/* Toggle                                                                    */}
      {/* ========================================================================= */}
      <Demo
        title="Toggle · Outline"
        description="An outline variant of the Toggle."
        center      >
        <Toggle variant="outline" aria-label="Toggle italic">
          <Italic className="size-4" />
        </Toggle>
      </Demo>

      <Demo
        title="Toggle · With Text"
        description="A Toggle with a text label."
        center      >
        <Toggle aria-label="Toggle italic">
          <Italic className="size-4" />
          Italic
        </Toggle>
      </Demo>

      <Demo
        title="Toggle · Size"
        description="Toggle supports sm, default, and lg sizes."
        center      >
        <Toggle size="sm" aria-label="Toggle italic">
          <Italic className="size-3.5" />
        </Toggle>
        <Toggle size="default" aria-label="Toggle italic">
          <Italic className="size-4" />
        </Toggle>
        <Toggle size="lg" aria-label="Toggle italic">
          <Italic className="size-4" />
        </Toggle>
      </Demo>

      <Demo
        title="Toggle · Disabled"
        description="A Toggle in the disabled state."
        center      >
        <Toggle disabled aria-label="Toggle underline">
          <Underline className="size-4" />
        </Toggle>
      </Demo>

      </Section>
  )
}
