import { Layers } from "lucide-react"
import { NavigationMenu, NavigationMenuContent, NavigationMenuItem, NavigationMenuLink, NavigationMenuList, NavigationMenuTrigger, navigationMenuTriggerStyle } from "@/components/ui/navigation-menu"
import { Demo, Section } from "../shared"

export function NavigationMenuSection() {
  return (
<Section
        id="navigation-menu"
        title="Navigation Menu"
        group="examples"
        description="A collection of links for navigating websites."
        fileKey="navigation"
      >
      {/* ========================================================================= */}
      {/* NAVIGATION MENU                                                           */}
      {/* ========================================================================= */}

      {/* NavigationMenu · Composition */}
      <Demo
        title="NavigationMenu · Composition"
        description="A horizontal navigation menu with dropdown panels of featured links."
      >
        <NavigationMenu>
          <NavigationMenuList>
            <NavigationMenuItem>
              <NavigationMenuTrigger>Getting started</NavigationMenuTrigger>
              <NavigationMenuContent>
                <ul className="grid gap-3 p-4 md:w-[400px] lg:w-[500px] lg:grid-cols-[.75fr_1fr]">
                  <li className="row-span-3">
                    <NavigationMenuLink
                      render={
                        <a
                          className="flex h-full w-full select-none flex-col justify-end rounded-md bg-linear-to-b from-muted/50 to-muted p-6 no-underline outline-none focus:shadow-md"
                          href="/"
                        />
                      }
                    >
                      <Layers className="size-6" />
                      <div className="mb-2 mt-4 text-lg font-medium">shadcn/ui</div>
                      <p className="text-sm leading-tight text-muted-foreground">
                        Beautifully designed components that you can copy and paste into your apps.
                      </p>
                    </NavigationMenuLink>
                  </li>
                  <li>
                    <NavigationMenuLink render={<a href="#introduction" />}>
                      <div className="text-sm font-medium leading-none">Introduction</div>
                      <p className="line-clamp-2 text-sm leading-snug text-muted-foreground">
                        Re-usable components built using Base UI and Tailwind CSS.
                      </p>
                    </NavigationMenuLink>
                  </li>
                  <li>
                    <NavigationMenuLink render={<a href="#installation" />}>
                      <div className="text-sm font-medium leading-none">Installation</div>
                      <p className="line-clamp-2 text-sm leading-snug text-muted-foreground">
                        How to install dependencies and structure your app.
                      </p>
                    </NavigationMenuLink>
                  </li>
                  <li>
                    <NavigationMenuLink render={<a href="#typography" />}>
                      <div className="text-sm font-medium leading-none">Typography</div>
                      <p className="line-clamp-2 text-sm leading-snug text-muted-foreground">
                        Styles for headings, paragraphs, lists, and code.
                      </p>
                    </NavigationMenuLink>
                  </li>
                </ul>
              </NavigationMenuContent>
            </NavigationMenuItem>
            <NavigationMenuItem>
              <NavigationMenuTrigger>Components</NavigationMenuTrigger>
              <NavigationMenuContent>
                <ul className="grid w-[400px] gap-3 p-4 md:w-[500px] md:grid-cols-2 lg:w-[600px]">
                  <li>
                    <NavigationMenuLink render={<a href="#alert" />}>
                      <div className="text-sm font-medium leading-none">Alert Dialog</div>
                      <p className="line-clamp-2 text-sm leading-snug text-muted-foreground">
                        A modal dialog that interrupts the user with important content.
                      </p>
                    </NavigationMenuLink>
                  </li>
                  <li>
                    <NavigationMenuLink render={<a href="#hover-card" />}>
                      <div className="text-sm font-medium leading-none">Hover Card</div>
                      <p className="line-clamp-2 text-sm leading-snug text-muted-foreground">
                        For sighted users to preview content available behind a link.
                      </p>
                    </NavigationMenuLink>
                  </li>
                  <li>
                    <NavigationMenuLink render={<a href="#progress" />}>
                      <div className="text-sm font-medium leading-none">Progress</div>
                      <p className="line-clamp-2 text-sm leading-snug text-muted-foreground">
                        Displays an indicator showing the completion progress of a task.
                      </p>
                    </NavigationMenuLink>
                  </li>
                  <li>
                    <NavigationMenuLink render={<a href="#scroll-area" />}>
                      <div className="text-sm font-medium leading-none">Scroll Area</div>
                      <p className="line-clamp-2 text-sm leading-snug text-muted-foreground">
                        Augments native scroll functionality for custom cross-browser styling.
                      </p>
                    </NavigationMenuLink>
                  </li>
                </ul>
              </NavigationMenuContent>
            </NavigationMenuItem>
            <NavigationMenuItem>
              <NavigationMenuLink
                className={navigationMenuTriggerStyle()}
                render={<a href="#documentation" />}
              >
                Documentation
              </NavigationMenuLink>
            </NavigationMenuItem>
          </NavigationMenuList>
        </NavigationMenu>
      </Demo>

      {/* NavigationMenu · Link Component */}
      <Demo
        title="NavigationMenu · Link Component"
        description="A navigation menu of plain trigger-styled links without dropdowns."
      >
        <NavigationMenu>
          <NavigationMenuList>
            <NavigationMenuItem>
              <NavigationMenuLink
                className={navigationMenuTriggerStyle()}
                render={<a href="#overview" />}
              >
                Overview
              </NavigationMenuLink>
            </NavigationMenuItem>
            <NavigationMenuItem>
              <NavigationMenuLink
                className={navigationMenuTriggerStyle()}
                render={<a href="#pricing" />}
              >
                Pricing
              </NavigationMenuLink>
            </NavigationMenuItem>
            <NavigationMenuItem>
              <NavigationMenuLink
                className={navigationMenuTriggerStyle()}
                render={<a href="#blog" />}
              >
                Blog
              </NavigationMenuLink>
            </NavigationMenuItem>
          </NavigationMenuList>
        </NavigationMenu>
      </Demo>

      </Section>
  )
}
