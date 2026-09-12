import * as React from "react"
import { Bold, Italic, Underline } from "lucide-react"
import { FieldLabel } from "@/components/ui/field"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { Demo, Section } from "../shared"

function ToggleGroupCustomDemo() {
  const [weight, setWeight] = React.useState<string[]>(["normal"])

  return (
    <div className="w-full max-w-xs space-y-2">
      <FieldLabel>Font Weight</FieldLabel>
      <ToggleGroup
        value={weight}
        onValueChange={(val) => setWeight(val as string[])}
        variant="outline"
        className="w-full justify-between"
      >
        <ToggleGroupItem value="light" className="flex-1">
          Light
        </ToggleGroupItem>
        <ToggleGroupItem value="normal" className="flex-1">
          Regular
        </ToggleGroupItem>
        <ToggleGroupItem value="bold" className="flex-1">
          Bold
        </ToggleGroupItem>
      </ToggleGroup>
    </div>
  )
}

export function ToggleGroupSection() {
  return (
<Section
        id="toggle-group"
        title="Toggle Group"
        group="examples"
        description="A set of two-state buttons that can be toggled on or off."
        fileKey="data"
      >
      {/* ========================================================================= */}
      {/* ToggleGroup                                                               */}
      {/* ========================================================================= */}
      <Demo
        title="ToggleGroup · Composition"
        description="Composes a ToggleGroup with items."
        center      >
        <ToggleGroup defaultValue={["bold", "italic"]}>
          <ToggleGroupItem value="bold" aria-label="Toggle bold">
            <Bold className="size-4" />
          </ToggleGroupItem>
          <ToggleGroupItem value="italic" aria-label="Toggle italic">
            <Italic className="size-4" />
          </ToggleGroupItem>
          <ToggleGroupItem value="underline" aria-label="Toggle underline">
            <Underline className="size-4" />
          </ToggleGroupItem>
        </ToggleGroup>
      </Demo>

      <Demo
        title="ToggleGroup · Outline"
        description="An outline variant of the ToggleGroup."
        center      >
        <ToggleGroup variant="outline" defaultValue={["bold"]}>
          <ToggleGroupItem value="bold" aria-label="Toggle bold">
            <Bold className="size-4" />
          </ToggleGroupItem>
          <ToggleGroupItem value="italic" aria-label="Toggle italic">
            <Italic className="size-4" />
          </ToggleGroupItem>
          <ToggleGroupItem value="underline" aria-label="Toggle underline">
            <Underline className="size-4" />
          </ToggleGroupItem>
        </ToggleGroup>
      </Demo>

      <Demo
        title="ToggleGroup · Size"
        description="ToggleGroup scales all items with a size."
        center      >
        <div className="flex flex-col items-center gap-4">
          <ToggleGroup size="sm" variant="outline">
            <ToggleGroupItem value="bold" aria-label="Toggle bold">
              <Bold className="size-3.5" />
            </ToggleGroupItem>
            <ToggleGroupItem value="italic" aria-label="Toggle italic">
              <Italic className="size-3.5" />
            </ToggleGroupItem>
          </ToggleGroup>
          <ToggleGroup size="default" variant="outline">
            <ToggleGroupItem value="bold" aria-label="Toggle bold">
              <Bold className="size-4" />
            </ToggleGroupItem>
            <ToggleGroupItem value="italic" aria-label="Toggle italic">
              <Italic className="size-4" />
            </ToggleGroupItem>
          </ToggleGroup>
          <ToggleGroup size="lg" variant="outline">
            <ToggleGroupItem value="bold" aria-label="Toggle bold">
              <Bold className="size-4" />
            </ToggleGroupItem>
            <ToggleGroupItem value="italic" aria-label="Toggle italic">
              <Italic className="size-4" />
            </ToggleGroupItem>
          </ToggleGroup>
        </div>
      </Demo>

      <Demo
        title="ToggleGroup · Spacing"
        description="Adjusts the gap between ToggleGroup items."
        center      >
        <div className="flex flex-col items-center gap-4">
          <ToggleGroup spacing={0} variant="outline" defaultValue={["left"]}>
            <ToggleGroupItem value="left">Left</ToggleGroupItem>
            <ToggleGroupItem value="center">Center</ToggleGroupItem>
            <ToggleGroupItem value="right">Right</ToggleGroupItem>
          </ToggleGroup>
          <ToggleGroup spacing={2} variant="outline" defaultValue={["left"]}>
            <ToggleGroupItem value="left">Left</ToggleGroupItem>
            <ToggleGroupItem value="center">Center</ToggleGroupItem>
            <ToggleGroupItem value="right">Right</ToggleGroupItem>
          </ToggleGroup>
        </div>
      </Demo>

      <Demo
        title="ToggleGroup · Vertical"
        description="A vertical ToggleGroup orientation."
        center      >
        <ToggleGroup orientation="vertical" variant="outline" defaultValue={["top"]}>
          <ToggleGroupItem value="top">Top</ToggleGroupItem>
          <ToggleGroupItem value="middle">Middle</ToggleGroupItem>
          <ToggleGroupItem value="bottom">Bottom</ToggleGroupItem>
        </ToggleGroup>
      </Demo>

      <Demo
        title="ToggleGroup · Disabled"
        description="Disables the whole ToggleGroup."
        center      >
        <ToggleGroup disabled variant="outline" defaultValue={["bold"]}>
          <ToggleGroupItem value="bold" aria-label="Toggle bold">
            <Bold className="size-4" />
          </ToggleGroupItem>
          <ToggleGroupItem value="italic" aria-label="Toggle italic">
            <Italic className="size-4" />
          </ToggleGroupItem>
          <ToggleGroupItem value="underline" aria-label="Toggle underline">
            <Underline className="size-4" />
          </ToggleGroupItem>
        </ToggleGroup>
      </Demo>

      <Demo
        title="ToggleGroup · Custom"
        description="Custom styled ToggleGroup items."
        center      >
        <ToggleGroupCustomDemo />
      </Demo>

      </Section>
  )
}
