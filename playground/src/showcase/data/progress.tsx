import * as React from "react"
import { Progress, ProgressLabel, ProgressValue } from "@/components/ui/progress"
import { Slider } from "@/components/ui/slider"
import { Demo, Section } from "../shared"

function ProgressControlledDemo() {
  const [value, setValue] = React.useState<number>(50)

  return (
    <div className="flex w-full max-w-sm flex-col gap-4">
      <Progress value={value} className="w-full" />
      <Slider
        value={value}
        onValueChange={(val) => setValue(val as number)}
        min={0}
        max={100}
        step={1}
      />
    </div>
  )
}

export function ProgressSection() {
  return (
<Section
        id="progress"
        title="Progress"
        group="installation"
        description="Displays an indicator showing the completion progress of a task, typically displayed as a progress bar."
        fileKey="data"
      >
      {/* ========================================================================= */}
      {/* Progress                                                                  */}
      {/* ========================================================================= */}
      <Demo
        title="Progress · Composition"
        description="Composes Progress with a value bar."
        center      >
        <Progress value={56} className="w-full max-w-sm">
          <ProgressLabel>Upload progress</ProgressLabel>
          <ProgressValue />
        </Progress>
      </Demo>

      <Demo
        title="Progress · Label"
        description="Pairs a Progress with a visible label."
        center      >
        <Progress value={60} className="w-full max-w-sm">
          <ProgressLabel>Syncing files</ProgressLabel>
          <ProgressValue />
        </Progress>
      </Demo>

      <Demo
        title="Progress · Controlled"
        description="Controls the Progress value from state."
        center      >
        <ProgressControlledDemo />
      </Demo>

      </Section>
  )
}
