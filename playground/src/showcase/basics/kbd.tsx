import { Button } from "@/components/ui/button"
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { Kbd, KbdGroup } from "@/components/ui/kbd"
import { Demo, Section } from "../shared"

export function KbdSection() {
  return (
<Section
        id="kbd"
        title="Kbd"
        group="examples"
        description="Used to display textual user input from keyboard."
        fileKey="basics"
      >

      <Demo
        title="Kbd · Composition"
        description="Kbd renders keyboard keys inline with text."
        center      >
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          Press <Kbd>⌘</Kbd> + <Kbd>K</Kbd> to open the command palette
        </div>
      </Demo>

      <Demo
        title="Kbd · Group"
        description="KbdGroup combines multiple keys into one shortcut."
        center      >
        <KbdGroup>
          <Kbd>Ctrl</Kbd>
          <Kbd>Shift</Kbd>
          <Kbd>P</Kbd>
        </KbdGroup>
      </Demo>

      <Demo
        title="Kbd · Button"
        description="Kbd can be embedded in a button to show its shortcut."
        center      >
        <Button variant="outline">
          Save changes
          <KbdGroup>
            <Kbd>⌘</Kbd>
            <Kbd>S</Kbd>
          </KbdGroup>
        </Button>
      </Demo>

      <Demo
        title="Kbd · Tooltip"
        description="Kbd can be shown inside a tooltip."      >
        <Tooltip>
          <TooltipTrigger
            render={<Button variant="outline">Save</Button>}
          />
          <TooltipContent>
            <p>Save changes</p>
            <KbdGroup>
              <Kbd>⌘</Kbd>
              <Kbd>S</Kbd>
            </KbdGroup>
          </TooltipContent>
        </Tooltip>
      </Demo>

      <Demo
        title="Kbd · Input Group"
        description="InputGroup can show a keyboard shortcut as a suffix."
        center      >
        <InputGroup>
          <InputGroupInput placeholder="Search..." />
          <InputGroupAddon align="inline-end">
            <Kbd>⌘</Kbd>
            <Kbd>K</Kbd>
          </InputGroupAddon>
        </InputGroup>
      </Demo>
      </Section>
  )
}
