/* Separator: Base UI (@base-ui/react/separator) */
"use client"

import * as React from "react"
import { Separator } from "@base-ui/react/separator"
import { cn } from "cn"

function Separator_({
  className,
  orientation = "horizontal",
  ...props
}: React.ComponentProps<typeof Separator>) {
  return (
    <Separator
      data-slot="separator"
      orientation={orientation}
      className={cn(
        "shrink-0 bg-border data-[orientation=horizontal]:h-px data-[orientation=horizontal]:w-full data-[orientation=vertical]:w-px data-[orientation=vertical]:self-stretch",
        className
      )}
      {...props}
    />
  )
}

export { Separator_ as Separator }
