import { Save } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { Kbd } from "@/components/ui/kbd"
import { Demo, Section } from "../shared"

export function TooltipSection() {
  return (
<Section
        id="tooltip"
        title="Tooltip"
        group="examples"
        description="A popup that displays information related to an element when the element receives keyboard focus or the mouse hovers over it."
        fileKey="overlays"
      >
      {/* ========================================================= */}
      {/* ======================== TOOLTIP ======================== */}
      {/* ========================================================= */}
      <Demo
        title="Tooltip · Composition"
        description="A tooltip that appears above a button trigger."
        center
      >
        <Tooltip>
          <TooltipTrigger render={<Button variant="outline">Hover</Button>} />
          <TooltipContent>
            <p>Add to library</p>
          </TooltipContent>
        </Tooltip>
      </Demo>

      <Demo
        title="Tooltip · Side"
        description="Tooltips positioned on the left, top, bottom, or right side of the trigger."
        center
      >
        <div className="flex flex-wrap gap-2">
          {( ["left", "top", "bottom", "right"] as const).map((side) => (
            <Tooltip key={side}>
              <TooltipTrigger render={<Button variant="outline" className="w-fit capitalize">{side}</Button>} />
              <TooltipContent side={side}>
                <p>Add to library</p>
              </TooltipContent>
            </Tooltip>
          ))}
        </div>
      </Demo>

      <Demo
        title="Tooltip · With Keyboard Shortcut"
        description="A tooltip that displays a keyboard shortcut next to its label."
        center
      >
        <Tooltip>
          <TooltipTrigger render={<Button variant="outline" size="icon"><Save /></Button>} />
          <TooltipContent>
            Save Changes <Kbd>S</Kbd>
          </TooltipContent>
        </Tooltip>
      </Demo>

      <Demo
        title="Tooltip · Disabled Button"
        description="A tooltip attached to a disabled button via a wrapper element."
        center
      >
        <Tooltip>
          <TooltipTrigger render={<span className="inline-block w-fit"><Button variant="outline" disabled>Disabled</Button></span>} />
          <TooltipContent>
            <p>This feature is currently unavailable</p>
          </TooltipContent>
        </Tooltip>
      </Demo>

      </Section>
  )
}
