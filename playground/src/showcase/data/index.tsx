export { ProgressSection } from "./progress"
export { AccordionSection } from "./accordion"
export { CollapsibleSection } from "./collapsible"
export { ToggleSection } from "./toggle"
export { ToggleGroupSection } from "./toggle-group"
export { TableSection } from "./table"
export { SonnerSection } from "./sonner"
export { ChartSection } from "./chart"
export { TimelineSection } from "./timeline"
import { ProgressSection } from "./progress"
import { AccordionSection } from "./accordion"
import { CollapsibleSection } from "./collapsible"
import { ToggleSection } from "./toggle"
import { ToggleGroupSection } from "./toggle-group"
import { TableSection } from "./table"
import { SonnerSection } from "./sonner"
import { ChartSection } from "./chart"
import { TimelineSection } from "./timeline"

export default function DataSection() {
  return (
    <div className="flex flex-col gap-12">
      <ProgressSection />
      <AccordionSection />
      <CollapsibleSection />
      <ToggleSection />
      <ToggleGroupSection />
      <TableSection />
      <SonnerSection />
      <ChartSection />
      <TimelineSection />
    </div>
  )
}
