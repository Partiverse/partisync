import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion"
import { Demo, Section } from "../shared"

export function AccordionSection() {
  return (
<Section
        id="accordion"
        title="Accordion"
        group="examples"
        description="A vertically stacked set of interactive headings that each reveal a section of content."
        fileKey="data"
      >
      {/* ========================================================================= */}
      {/* Accordion                                                                 */}
      {/* ========================================================================= */}
      <Demo
        title="Accordion · Composition"
        description="Composes Accordion items from its parts."      >
        <div className="w-full max-w-sm">
          <Accordion defaultValue={["item-1"]}>
            <AccordionItem value="item-1">
              <AccordionTrigger>Is it accessible?</AccordionTrigger>
              <AccordionContent>
                Yes. It adheres to the WAI-ARIA design pattern.
              </AccordionContent>
            </AccordionItem>
            <AccordionItem value="item-2">
              <AccordionTrigger>Is it styled?</AccordionTrigger>
              <AccordionContent>
                Yes. It comes with default styles that match the other components&apos;
                aesthetic.
              </AccordionContent>
            </AccordionItem>
          </Accordion>
        </div>
      </Demo>

      <Demo
        title="Accordion · Basic"
        description="A single-expand accordion."      >
        <div className="w-full max-w-sm">
          <Accordion defaultValue={["item-1"]}>
            <AccordionItem value="item-1">
              <AccordionTrigger>Is it accessible?</AccordionTrigger>
              <AccordionContent>
                Yes. It adheres to the WAI-ARIA design pattern.
              </AccordionContent>
            </AccordionItem>
            <AccordionItem value="item-2">
              <AccordionTrigger>Is it styled?</AccordionTrigger>
              <AccordionContent>
                Yes. It comes with default styles that match the other components&apos;
                aesthetic.
              </AccordionContent>
            </AccordionItem>
            <AccordionItem value="item-3">
              <AccordionTrigger>Is it animated?</AccordionTrigger>
              <AccordionContent>
                Yes. It&apos;s animated by default, but you can disable it if you
                prefer.
              </AccordionContent>
            </AccordionItem>
          </Accordion>
        </div>
      </Demo>

      <Demo
        title="Accordion · Multiple"
        description="Allows multiple accordion items open at once."      >
        <div className="w-full max-w-sm">
          <Accordion multiple defaultValue={["item-1", "item-2"]}>
            <AccordionItem value="item-1">
              <AccordionTrigger>Can I open multiple items?</AccordionTrigger>
              <AccordionContent>
                Yes. Set the multiple prop to allow multiple items to be expanded
                simultaneously.
              </AccordionContent>
            </AccordionItem>
            <AccordionItem value="item-2">
              <AccordionTrigger>Is it animated?</AccordionTrigger>
              <AccordionContent>
                Yes. Each item animates independently during expand and collapse.
              </AccordionContent>
            </AccordionItem>
          </Accordion>
        </div>
      </Demo>

      <Demo
        title="Accordion · Disabled"
        description="Disables individual or all accordion items."      >
        <div className="w-full max-w-sm">
          <Accordion defaultValue={["item-1"]}>
            <AccordionItem value="item-1">
              <AccordionTrigger>Active item</AccordionTrigger>
              <AccordionContent>
                This item can be interacted with normally.
              </AccordionContent>
            </AccordionItem>
            <AccordionItem value="item-2" disabled>
              <AccordionTrigger>Disabled item</AccordionTrigger>
              <AccordionContent>
                This item is disabled and cannot be expanded.
              </AccordionContent>
            </AccordionItem>
          </Accordion>
        </div>
      </Demo>

      <Demo
        title="Accordion · Borders"
        description="Adds borders between accordion items."      >
        <div className="w-full max-w-sm">
          <Accordion defaultValue={["item-1"]} className="border rounded-lg px-4">
            <AccordionItem value="item-1" className="border-b last:border-b-0">
              <AccordionTrigger>First section</AccordionTrigger>
              <AccordionContent>
                Content with explicit borders on items.
              </AccordionContent>
            </AccordionItem>
            <AccordionItem value="item-2" className="border-b last:border-b-0">
              <AccordionTrigger>Second section</AccordionTrigger>
              <AccordionContent>
                Border disappears on the last item in the list.
              </AccordionContent>
            </AccordionItem>
          </Accordion>
        </div>
      </Demo>

      <Demo
        title="Accordion · Card"
        description="Renders an accordion inside a Card."      >
        <Card className="w-full max-w-sm">
          <CardHeader>
            <CardTitle>FAQ</CardTitle>
            <CardDescription>Frequently asked questions.</CardDescription>
          </CardHeader>
          <CardContent>
            <Accordion defaultValue={["item-1"]}>
              <AccordionItem value="item-1">
                <AccordionTrigger>What is shadcn/ui?</AccordionTrigger>
                <AccordionContent>
                  Beautifully designed components that you can copy and paste into
                  your apps. Accessible. Customizable. Open Source.
                </AccordionContent>
              </AccordionItem>
              <AccordionItem value="item-2">
                <AccordionTrigger>What is base-nova?</AccordionTrigger>
                <AccordionContent>
                  The modern theme based on Base UI primitives and Tailwind CSS.
                </AccordionContent>
              </AccordionItem>
            </Accordion>
          </CardContent>
        </Card>
      </Demo>

      </Section>
  )
}
