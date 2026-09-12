"use client"

import * as React from "react"
import { cn } from "cn"

function Item({
  className,
  ...props
}: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="item"
      className={cn(
        "group/item relative flex flex-col gap-1",
        className
      )}
      {...props}
    />
  )
}

function ItemIndicator({
  className,
  ...props
}: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="item-indicator"
      className={cn(
        "absolute start-0 top-1 flex size-5 items-center justify-center",
        className
      )}
      {...props}
    />
  )
}

function ItemContent({
  className,
  ...props
}: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="item-content"
      className={cn("flex flex-col gap-0.5 ps-8", className)}
      {...props}
    />
  )
}

function ItemTitle({
  className,
  ...props
}: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="item-title"
      className={cn("text-sm font-medium leading-snug", className)}
      {...props}
    />
  )
}

function ItemDescription({
  className,
  ...props
}: React.ComponentProps<"p">) {
  return (
    <p
      data-slot="item-description"
      className={cn("text-sm text-muted-foreground leading-normal", className)}
      {...props}
    />
  )
}

export {
  Item,
  ItemIndicator,
  ItemContent,
  ItemTitle,
  ItemDescription,
}
