import { Item, ItemContent, ItemDescription, ItemIndicator, ItemTitle } from "@/components/ui/item"
import { Demo, Section } from "../shared"
import { CheckIcon } from "lucide-react"

export function ItemSection() {
  return (
    <Section
      id="item"
      title="Item"
      group="installation"
      description="A base component for composing list items with indicators, titles, and descriptions."
      fileKey="basics"
    >
      <Demo
        title="Item · Composition"
        description="Composes Item from indicator, title, and description parts."
      >
        <div className="w-full max-w-sm space-y-4">
          <Item>
            <ItemIndicator>
              <div className="flex size-5 items-center justify-center rounded-full bg-primary text-primary-foreground">
                <CheckIcon className="size-3" />
              </div>
            </ItemIndicator>
            <ItemContent>
              <ItemTitle>Item Title</ItemTitle>
              <ItemDescription>
                This is a description for the item. It provides additional context.
              </ItemDescription>
            </ItemContent>
          </Item>

          <Item>
            <ItemIndicator>
              <div className="flex size-5 items-center justify-center rounded-full bg-muted border border-border">
                <span className="text-xs font-medium text-muted-foreground">2</span>
              </div>
            </ItemIndicator>
            <ItemContent>
              <ItemTitle>List Item</ItemTitle>
              <ItemDescription>
                Another example of an item component with a different indicator.
              </ItemDescription>
            </ItemContent>
          </Item>
        </div>
      </Demo>

      <Demo
        title="Item · Indicator States"
        description="Items with different indicator states."
      >
        <div className="w-full max-w-sm space-y-4">
          <Item>
            <ItemIndicator>
              <div className="flex size-5 items-center justify-center rounded-full border-2 border-primary" />
            </ItemIndicator>
            <ItemContent>
              <ItemTitle>Unchecked Item</ItemTitle>
              <ItemDescription>Pending state indicator.</ItemDescription>
            </ItemContent>
          </Item>

          <Item>
            <ItemIndicator>
              <div className="flex size-5 items-center justify-center rounded-full bg-green-500 text-white">
                <CheckIcon className="size-3" />
              </div>
            </ItemIndicator>
            <ItemContent>
              <ItemTitle>Completed Task</ItemTitle>
              <ItemDescription>Success state with green indicator.</ItemDescription>
            </ItemContent>
          </Item>

          <Item>
            <ItemIndicator>
              <div className="flex size-5 items-center justify-center rounded-full bg-amber-500 text-white">
                <span className="text-xs font-bold">!</span>
              </div>
            </ItemIndicator>
            <ItemContent>
              <ItemTitle>Warning Item</ItemTitle>
              <ItemDescription>Warning state with amber indicator.</ItemDescription>
            </ItemContent>
          </Item>
        </div>
      </Demo>

      <Demo
        title="Item · Simple"
        description="A minimal item without an indicator."
      >
        <div className="w-full max-w-sm space-y-3">
          <Item>
            <ItemContent>
              <ItemTitle>Simple Item Title</ItemTitle>
            </ItemContent>
          </Item>

          <Item>
            <ItemContent>
              <ItemTitle>Another Simple Item</ItemTitle>
              <ItemDescription>A simple item with both title and description.</ItemDescription>
            </ItemContent>
          </Item>
        </div>
      </Demo>
    </Section>
  )
}
