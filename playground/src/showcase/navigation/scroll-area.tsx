import { FileText } from "lucide-react"
import { ScrollArea, ScrollBar } from "@/components/ui/scroll-area"
import { Demo, Section } from "../shared"

export function ScrollAreaSection() {
  return (
<Section
        id="scroll-area"
        title="Scroll Area"
        group="examples"
        description="Augments native scroll functionality for custom, cross-browser styling."
        fileKey="navigation"
      >
      {/* ========================================================================= */}
      {/* SCROLL AREA                                                               */}
      {/* ========================================================================= */}

      {/* ScrollArea · Composition */}
      <Demo
        title="ScrollArea · Composition"
        description="A vertically scrollable area listing tagged versions."
      >
        <ScrollArea className="h-72 w-full max-w-sm rounded-md border p-4">
          <div className="space-y-4">
            <h4 className="text-sm font-semibold leading-none">Tags</h4>
            {Array.from({ length: 25 }).map((_, i) => (
              <div key={i} className="text-sm border-b pb-2 last:border-0 last:pb-0">
                v1.2.0-beta.{25 - i}
              </div>
            ))}
          </div>
        </ScrollArea>
      </Demo>

      {/* ScrollArea · Horizontal */}
      <Demo
        title="ScrollArea · Horizontal"
        description="A horizontally scrollable area with a horizontal scrollbar."
      >
        <ScrollArea className="w-full max-w-md whitespace-nowrap rounded-md border p-4">
          <div className="flex w-max space-x-4">
            {Array.from({ length: 15 }).map((_, i) => (
              <div
                key={i}
                className="flex h-32 w-28 shrink-0 flex-col items-center justify-center rounded-md bg-muted p-2"
              >
                <FileText className="size-8 text-muted-foreground mb-2" />
                <span className="text-xs font-medium">Artwork #{i + 1}</span>
              </div>
            ))}
          </div>
          <ScrollBar orientation="horizontal" />
        </ScrollArea>
      </Demo>

      </Section>
  )
}
