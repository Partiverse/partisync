import * as React from "react"
import { Select, SelectContent, SelectGroup, SelectItem, SelectLabel, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Demo, Section } from "../shared"

export function SelectSection() {
  const [select, setSelect] = React.useState("")

  return (
<Section
        id="select"
        title="Select"
        group="usage"
        description="Displays a list of options for the user to pick from—triggered by a button."
        fileKey="forms"
      >
      {/* ============ Select · Official H2 demos ============ */}
      <Demo
        title="Select · Composition"
        description="Select renders a trigger and a list of options."      >
        <Select value={select} onValueChange={(v) => setSelect(v ?? "medium")}>
          <SelectTrigger className="w-48">
            <SelectValue placeholder="Select..." />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="low">Low</SelectItem>
            <SelectItem value="medium">Medium</SelectItem>
            <SelectItem value="high">High</SelectItem>
          </SelectContent>
        </Select>
      </Demo>

      <Demo
        title="Select · Groups"
        description="SelectGroup groups options with labels."      >
        <Select defaultValue="apple">
          <SelectTrigger className="w-48">
            <SelectValue placeholder="Fruit" />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              <SelectLabel>Fruits</SelectLabel>
              <SelectItem value="apple">Apple</SelectItem>
              <SelectItem value="banana">Banana</SelectItem>
            </SelectGroup>
            <SelectGroup>
              <SelectLabel>Vegetables</SelectLabel>
              <SelectItem value="carrot">Carrot</SelectItem>
              <SelectItem value="potato">Potato</SelectItem>
            </SelectGroup>
          </SelectContent>
        </Select>
      </Demo>

      <Demo
        title="Select · Scrollable"
        description="Set a max-height on the content for long lists."      >
        <Select defaultValue="utc-8">
          <SelectTrigger className="w-48">
            <SelectValue placeholder="Timezone" />
          </SelectTrigger>
          <SelectContent className="max-h-56">
            {Array.from({ length: 24 }).map((_, i) => (
              <SelectItem key={i} value={`utc-${i}`}>UTC{i - 11 >= 0 ? `+${i - 11}` : i - 11}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Demo>

      <Demo
        title="Select · Disabled"
        description="Use the disabled attribute to prevent interaction."      >
        <Select disabled defaultValue="a">
          <SelectTrigger className="w-48">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="a">Option A</SelectItem>
          </SelectContent>
        </Select>
      </Demo>

      <Demo
        title="Select · Invalid"
        description="Use aria-invalid to mark a select as invalid."      >
        <Select defaultValue="">
          <SelectTrigger aria-invalid className="w-48">
            <SelectValue placeholder="Pick one" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="a">Option A</SelectItem>
            <SelectItem value="b">Option B</SelectItem>
          </SelectContent>
        </Select>
      </Demo>

      </Section>
  )
}
