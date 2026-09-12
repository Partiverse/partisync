export { TabsSection } from "./tabs"
export { BreadcrumbSection } from "./breadcrumb"
export { PaginationSection } from "./pagination"
export { MenubarSection } from "./menubar"
export { NavigationMenuSection } from "./navigation-menu"
export { DropdownMenuSection } from "./dropdown-menu"
export { ContextMenuSection } from "./context-menu"
export { ScrollAreaSection } from "./scroll-area"
export { ResizableSection } from "./resizable"
import { TabsSection } from "./tabs"
import { BreadcrumbSection } from "./breadcrumb"
import { PaginationSection } from "./pagination"
import { MenubarSection } from "./menubar"
import { NavigationMenuSection } from "./navigation-menu"
import { DropdownMenuSection } from "./dropdown-menu"
import { ContextMenuSection } from "./context-menu"
import { ScrollAreaSection } from "./scroll-area"
import { ResizableSection } from "./resizable"

export default function NavigationSection() {
  return (
    <div className="flex flex-col gap-12">
      <TabsSection />
      <BreadcrumbSection />
      <PaginationSection />
      <MenubarSection />
      <NavigationMenuSection />
      <DropdownMenuSection />
      <ContextMenuSection />
      <ScrollAreaSection />
      <ResizableSection />
    </div>
  )
}
