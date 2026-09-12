import { Button } from "@/components/ui/button"
import { HoverCard, HoverCardContent, HoverCardTrigger } from "@/components/ui/hover-card"
import { Demo, Section } from "../shared"

const HOVER_CARD_SIDES = ["left", "top", "bottom", "right"] as const

export function HoverCardSection() {
  return (
<Section
        id="hover-card"
        title="Hover Card"
        group="examples"
        description="For sighted users to preview content available behind a link."
        fileKey="overlays"
      >
      {/* ========================================================= */}
      {/* ======================= HOVER CARD ====================== */}
      {/* ========================================================= */}
      <Demo
        title="HoverCard · Composition"
        description="A hover card that previews profile details when hovering a link."
        center
      >
        <HoverCard>
          <HoverCardTrigger delay={10} closeDelay={100} render={<Button variant="link">Hover Here</Button>} />
          <HoverCardContent className="flex w-64 flex-col gap-0.5">
            <div className="font-semibold">@nextjs</div>
            <div>The React Framework – created and maintained by @vercel.</div>
            <div className="mt-1 text-xs text-muted-foreground">
              Joined December 2021
            </div>
          </HoverCardContent>
        </HoverCard>
      </Demo>

      <Demo
        title="HoverCard · Trigger Delays"
        description="A hover card with custom open and close delays on the trigger."
        center
      >
        <HoverCard>
          <HoverCardTrigger delay={100} closeDelay={200} render={<Button variant="outline">Hover (100ms delay)</Button>} />
          <HoverCardContent className="w-64">
            <p className="text-sm">Triggered with delay=100 and closeDelay=200.</p>
          </HoverCardContent>
        </HoverCard>
      </Demo>

      <Demo
        title="HoverCard · Positioning"
        description="A hover card positioned with side and align relative to its trigger."
        center
      >
        <HoverCard>
          <HoverCardTrigger render={<Button variant="outline">Top / Start</Button>} />
          <HoverCardContent side="top" align="start" className="w-64">
            <p className="text-sm">Positioned with side=&quot;top&quot; and align=&quot;start&quot;.</p>
          </HoverCardContent>
        </HoverCard>
      </Demo>

      <Demo
        title="HoverCard · Basic"
        description="A simple hover card showing a user handle and joined date."
        center
      >
        <HoverCard>
          <HoverCardTrigger delay={10} closeDelay={100} render={<Button variant="link">Hover Here</Button>} />
          <HoverCardContent className="flex w-64 flex-col gap-0.5">
            <div className="font-semibold">@shadcn</div>
            <div>Beautifully designed components that you can copy and paste into your apps.</div>
            <div className="mt-1 text-xs text-muted-foreground">
              Joined March 2023
            </div>
          </HoverCardContent>
        </HoverCard>
      </Demo>

      <Demo
        title="HoverCard · Sides"
        description="Hover cards appearing from each side of their trigger."
        center
      >
        <div className="flex flex-wrap justify-center gap-2">
          {HOVER_CARD_SIDES.map((side) => (
            <HoverCard key={side}>
              <HoverCardTrigger delay={100} closeDelay={100} render={<Button variant="outline" className="capitalize">{side}</Button>} />
              <HoverCardContent side={side}>
                <div className="flex flex-col gap-1">
                  <h4 className="font-medium">Hover Card</h4>
                  <p className="text-sm text-muted-foreground">This hover card appears on the {side} side of the trigger.</p>
                </div>
              </HoverCardContent>
            </HoverCard>
          ))}
        </div>
      </Demo>

      </Section>
  )
}
