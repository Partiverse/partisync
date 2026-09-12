import * as React from "react"
import { Slider } from "@/components/ui/slider"
import { Demo, Section } from "../shared"

export function SliderSection() {
  const [slider, setSlider] = React.useState(50)
  const [sliderRange, setSliderRange] = React.useState([25, 75])

  return (
<Section
        id="slider"
        title="Slider"
        group="examples"
        description="An input where the user selects a value from within a given range."
        fileKey="forms"
      >
      {/* ============ Slider · Official H2 demos ============ */}
      <Demo
        title="Slider · Range"
        description="A slider with two thumbs selects a range."      >
        <Slider value={sliderRange} onValueChange={(v) => setSliderRange(Array.isArray(v) ? v : [v as number])} />
        <p className="text-xs text-muted-foreground">
          Range: {sliderRange[0]} – {sliderRange[1]}
        </p>
      </Demo>

      <Demo
        title="Slider · Multiple Thumbs"
        description="A slider supports multiple thumbs."      >
        <Slider defaultValue={[20, 40, 80]} />
      </Demo>

      <Demo
        title="Slider · Vertical"
        description="Set orientation to vertical for a vertical slider."      >
        <div className="flex h-40 items-center justify-center">
          <Slider orientation="vertical" defaultValue={[50]} className="h-full" />
        </div>
      </Demo>

      <Demo
        title="Slider · Controlled"
        description="The slider value can be controlled with state."      >
        <Slider value={[slider]} onValueChange={(v) => setSlider(Array.isArray(v) ? v[0] : (v as number))} />
        <p className="text-center text-xs text-muted-foreground">
          Volume: <span className="font-mono font-semibold text-foreground">{slider}%</span>
        </p>
      </Demo>

      <Demo
        title="Slider · Disabled"
        description="Use the disabled attribute to prevent interaction."      >
        <Slider disabled defaultValue={[60]} />
      </Demo>

      </Section>
  )
}
