import { AspectRatio } from "@/components/ui/aspect-ratio"
import { Demo, Section } from "../shared"

export function AspectRatioSection() {
  return (
<Section
        id="aspect-ratio"
        title="Aspect Ratio"
        group="examples"
        description="Displays content within a desired ratio."
        fileKey="basics"
      >

      <Demo
        title="AspectRatio · Square（1:1）"
        description="AspectRatio constrains its content to a 1:1 ratio."      >
        <AspectRatio ratio={1} className="w-40 overflow-hidden rounded-lg bg-muted">
          <div className="flex h-full w-full items-center justify-center">
            <span className="font-mono text-xs text-muted-foreground">1:1</span>
          </div>
        </AspectRatio>
      </Demo>

      <Demo
        title="AspectRatio · Portrait（4:5）"
        description="AspectRatio constrains its content to a 4:5 ratio."      >
        <AspectRatio ratio={4 / 5} className="w-40 overflow-hidden rounded-lg bg-muted">
          <div className="flex h-full w-full items-center justify-center">
            <span className="font-mono text-xs text-muted-foreground">4:5</span>
          </div>
        </AspectRatio>
      </Demo>

      <Demo
        title="AspectRatio · Landscape（16:9）"
        description="AspectRatio constrains its content to a 16:9 ratio."      >
        <AspectRatio ratio={16 / 9} className="w-full overflow-hidden rounded-lg bg-muted">
          <div className="flex h-full w-full items-center justify-center">
            <span className="font-mono text-xs text-muted-foreground">16:9</span>
          </div>
        </AspectRatio>
      </Demo>
      </Section>
  )
}
