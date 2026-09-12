export { DialogSection } from "./dialog"
export { AlertDialogSection } from "./alert-dialog"
export { SheetSection } from "./sheet"
export { DrawerSection } from "./drawer"
export { PopoverSection } from "./popover"
export { HoverCardSection } from "./hover-card"
export { TooltipSection } from "./tooltip"
export { CommandSection } from "./command"
import { DialogSection } from "./dialog"
import { AlertDialogSection } from "./alert-dialog"
import { SheetSection } from "./sheet"
import { DrawerSection } from "./drawer"
import { PopoverSection } from "./popover"
import { HoverCardSection } from "./hover-card"
import { TooltipSection } from "./tooltip"
import { CommandSection } from "./command"

export default function OverlaysSection() {
  return (
    <div className="flex flex-col gap-12">
      <DialogSection />
      <AlertDialogSection />
      <SheetSection />
      <DrawerSection />
      <PopoverSection />
      <HoverCardSection />
      <TooltipSection />
      <CommandSection />
    </div>
  )
}
