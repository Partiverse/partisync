import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverDescription, PopoverHeader, PopoverTitle, PopoverTrigger } from "@/components/ui/popover"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Demo, Section } from "../shared"

export function PopoverSection() {
  return (
<Section
        id="popover"
        title="Popover"
        group="usage"
        description="Displays rich content in a portal, triggered by a button."
        fileKey="overlays"
      >
      {/* ========================================================= */}
      {/* ======================== POPOVER ======================== */}
      {/* ========================================================= */}
      <Demo
        title="Popover · Composition"
        description="A popover composed with a trigger and a dimensions form."
        center
      >
        <Popover>
          <PopoverTrigger render={<Button variant="outline">Open popover</Button>} />
          <PopoverContent className="w-80">
            <div className="grid gap-4">
              <div className="space-y-2">
                <h4 className="leading-none font-medium">Dimensions</h4>
                <p className="text-sm text-muted-foreground">
                  Set the dimensions for the layer.
                </p>
              </div>
              <div className="grid gap-2">
                <div className="grid grid-cols-3 items-center gap-4">
                  <Label htmlFor="popover-comp-width">Width</Label>
                  <Input
                    id="popover-comp-width"
                    defaultValue="100%"
                    className="col-span-2 h-8"
                  />
                </div>
                <div className="grid grid-cols-3 items-center gap-4">
                  <Label htmlFor="popover-comp-maxWidth">Max. width</Label>
                  <Input
                    id="popover-comp-maxWidth"
                    defaultValue="300px"
                    className="col-span-2 h-8"
                  />
                </div>
                <div className="grid grid-cols-3 items-center gap-4">
                  <Label htmlFor="popover-comp-height">Height</Label>
                  <Input
                    id="popover-comp-height"
                    defaultValue="25px"
                    className="col-span-2 h-8"
                  />
                </div>
                <div className="grid grid-cols-3 items-center gap-4">
                  <Label htmlFor="popover-comp-maxHeight">Max. height</Label>
                  <Input
                    id="popover-comp-maxHeight"
                    defaultValue="none"
                    className="col-span-2 h-8"
                  />
                </div>
              </div>
            </div>
          </PopoverContent>
        </Popover>
      </Demo>

      <Demo
        title="Popover · Basic"
        description="A basic popover with a header title and description."
        center
      >
        <Popover>
          <PopoverTrigger render={<Button variant="outline" className="w-fit">Open Popover</Button>} />
          <PopoverContent align="start">
            <PopoverHeader>
              <PopoverTitle>Dimensions</PopoverTitle>
              <PopoverDescription>
                Set the dimensions for the layer.
              </PopoverDescription>
            </PopoverHeader>
          </PopoverContent>
        </Popover>
      </Demo>

      <Demo
        title="Popover · Align"
        description="Popover content aligned to the start, center, or end of the trigger."
        center
      >
        <div className="flex gap-6">
          <Popover>
            <PopoverTrigger render={<Button variant="outline" size="sm">Start</Button>} />
            <PopoverContent align="start" className="w-40">
              Aligned to start
            </PopoverContent>
          </Popover>
          <Popover>
            <PopoverTrigger render={<Button variant="outline" size="sm">Center</Button>} />
            <PopoverContent align="center" className="w-40">
              Aligned to center
            </PopoverContent>
          </Popover>
          <Popover>
            <PopoverTrigger render={<Button variant="outline" size="sm">End</Button>} />
            <PopoverContent align="end" className="w-40">
              Aligned to end
            </PopoverContent>
          </Popover>
        </div>
      </Demo>

      <Demo
        title="Popover · With Form"
        description="A popover containing a small form built with Field components."
        center
      >
        <Popover>
          <PopoverTrigger render={<Button variant="outline">Open Popover</Button>} />
          <PopoverContent className="w-64" align="start">
            <PopoverHeader>
              <PopoverTitle>Dimensions</PopoverTitle>
              <PopoverDescription>
                Set the dimensions for the layer.
              </PopoverDescription>
            </PopoverHeader>
            <FieldGroup className="gap-4">
              <Field orientation="horizontal">
                <FieldLabel htmlFor="popover-form-width" className="w-1/2">
                  Width
                </FieldLabel>
                <Input id="popover-form-width" defaultValue="100%" />
              </Field>
              <Field orientation="horizontal">
                <FieldLabel htmlFor="popover-form-height" className="w-1/2">
                  Height
                </FieldLabel>
                <Input id="popover-form-height" defaultValue="25px" />
              </Field>
            </FieldGroup>
          </PopoverContent>
        </Popover>
      </Demo>

      </Section>
  )
}
