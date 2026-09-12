import { ArrowRight, Bot as BotIcon, ChevronRight, Clock as ClockIcon, Minus, Plus as PlusIcon, Plus, Search as SearchIcon, Search } from "lucide-react"
import { Button, buttonVariants } from "@/components/ui/button"
import { ButtonGroup, ButtonGroupSeparator, ButtonGroupText } from "@/components/ui/button-group"
import { Input } from "@/components/ui/input"
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
import { DropdownMenu, DropdownMenuContent, DropdownMenuGroup, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Demo, Section } from "../shared"

export function ButtonGroupSection() {
  return (
<Section
        id="button-group"
        title="Button Group"
        group="examples"
        description="A container that groups related buttons together with consistent styling."
        fileKey="basics"
      >

      <Demo
        title="ButtonGroup · Composition"
        description="ButtonGroup composes buttons, text, and separators."
        center      >
        <ButtonGroup>
          <Button variant="outline">Button</Button>
          <ButtonGroupSeparator />
          <Button variant="outline">Button</Button>
        </ButtonGroup>
        <ButtonGroup>
          <Button variant="outline">Button</Button>
          <ButtonGroupText>Text</ButtonGroupText>
          <Button variant="outline">Button</Button>
        </ButtonGroup>
      </Demo>

      <Demo
        title="ButtonGroup · Accessibility"
        description="Wrap a ButtonGroup with an aria-label for accessibility."
        center      >
        <ButtonGroup aria-label="Button group">
          <Button variant="outline">Button 1</Button>
          <Button variant="outline">Button 2</Button>
          <Button variant="outline">Button 3</Button>
        </ButtonGroup>
      </Demo>

      <Demo
        title="ButtonGroup · Orientation"
        description="ButtonGroup supports horizontal and vertical orientations."
        center      >
        <div className="flex gap-6">
          <ButtonGroup orientation="vertical" aria-label="Media controls" className="h-fit">
            <Button variant="outline" size="icon" aria-label="Plus"><Plus /></Button>
            <Button variant="outline" size="icon" aria-label="Minus"><Minus /></Button>
          </ButtonGroup>
        </div>
      </Demo>

      <Demo
        title="ButtonGroup · Size"
        description="Set a size on the group to scale every child control."
        center      >
        <ButtonGroup>
          <Button variant="outline" size="icon" aria-label="Zoom in"><Search /></Button>
          <Button variant="outline" size="icon" aria-label="Zoom out"><Search /></Button>
        </ButtonGroup>
        <ButtonGroup>
          <Button variant="outline" size="sm" className="size-8">Button</Button>
        </ButtonGroup>
        <ButtonGroup>
          <Button variant="outline" size="lg" className="size-10">Button</Button>
        </ButtonGroup>
      </Demo>

      <Demo
        title="ButtonGroup · Nested"
        description="Button groups can be nested to build composite toolbars."
        center      >
        <ButtonGroup>
          <ButtonGroup>
            <Button variant="outline" size="icon" aria-label="Volume up"><ChevronRight /></Button>
            <ButtonGroupText>10</ButtonGroupText>
            <Button variant="outline" size="icon" aria-label="Volume down"><ChevronRight /></Button>
          </ButtonGroup>
          <ButtonGroupSeparator />
          <ButtonGroup>
            <Button variant="outline" size="icon" aria-label="Add"><ChevronRight /></Button>
            <Button variant="outline" size="icon" aria-label="Subtract"><ChevronRight /></Button>
          </ButtonGroup>
        </ButtonGroup>
      </Demo>

      <Demo
        title="ButtonGroup · Separator"
        description="Use separators to divide the items in a group."
        center      >
        <ButtonGroup>
          <Button variant="outline">Button</Button>
          <ButtonGroupSeparator />
          <Button variant="outline">Button</Button>
          <ButtonGroupSeparator />
          <Button variant="outline">Button</Button>
        </ButtonGroup>
        <ButtonGroup orientation="horizontal">
          <Button variant="outline">Button</Button>
          <ButtonGroupSeparator orientation="vertical" />
          <Button variant="outline">Button</Button>
        </ButtonGroup>
      </Demo>

      <Demo
        title="ButtonGroup · Split"
        description="A split button pairs a primary action with extra actions."
        center      >
        <ButtonGroup>
          <Button variant="outline">Button</Button>
          <ButtonGroupSeparator />
          <Button variant="outline" size="icon" aria-label="Add"><Plus /></Button>
        </ButtonGroup>
        <ButtonGroup>
          <Button variant="outline">Button</Button>
          <ButtonGroupSeparator />
          <Button variant="outline" size="icon" aria-label="Add"><Plus /></Button>
          <ButtonGroupSeparator />
          <Button variant="outline" size="icon" aria-label="Add"><Plus /></Button>
        </ButtonGroup>
      </Demo>

      <Demo
        title="ButtonGroup · Input"
        description="ButtonGroup can combine buttons with an input."
        center      >
        <ButtonGroup>
          <Button variant="outline" size="icon" aria-label="Search"><Search /></Button>
          <Input placeholder="Search..." />
          <Button>Go <ArrowRight data-icon="inline-end" /></Button>
        </ButtonGroup>
      </Demo>

      <Demo
        title="ButtonGroup · Input Group"
        description="ButtonGroup works with InputGroup for rich search fields."
        center      >
        <ButtonGroup className="w-full">
          <InputGroup>
            <InputGroupInput placeholder="Search..." />
            <InputGroupAddon>
              <SearchIcon />
            </InputGroupAddon>
          </InputGroup>
          <Button>Search</Button>
        </ButtonGroup>
      </Demo>

      <Demo
        title="ButtonGroup · Dropdown Menu"
        description="ButtonGroup can host a dropdown menu attached to the last item."
        center      >
        <ButtonGroup>
          <Button variant="outline">Snooze</Button>
          <DropdownMenu>
            <DropdownMenuTrigger
              render={<Button variant="outline" size="icon" aria-label="More Options"><ChevronRight /></Button>}
            />
            <DropdownMenuContent align="end" className="w-40">
              <DropdownMenuGroup>
                <DropdownMenuItem>Archive</DropdownMenuItem>
                <DropdownMenuItem>Report</DropdownMenuItem>
              </DropdownMenuGroup>
              <DropdownMenuSeparator />
              <DropdownMenuGroup>
                <DropdownMenuItem>
                  <ClockIcon /> Snooze
                </DropdownMenuItem>
                <DropdownMenuItem>
                  <PlusIcon /> Add to Calendar
                </DropdownMenuItem>
                <DropdownMenuItem>
                  <PlusIcon /> Add to List
                </DropdownMenuItem>
              </DropdownMenuGroup>
            </DropdownMenuContent>
          </DropdownMenu>
        </ButtonGroup>
        <ButtonGroup>
          <Button variant="outline">Button</Button>
          <DropdownMenu>
            <DropdownMenuTrigger
              render={<Button variant="outline" size="icon" aria-label="More Options"><ChevronRight /></Button>}
            />
            <DropdownMenuContent align="end" className="w-40">
              <DropdownMenuItem>
                <ClockIcon /> Snooze
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </ButtonGroup>
      </Demo>

      <Demo
        title="ButtonGroup · Select"
        description="ButtonGroup can pair a select with an action button."
        center      >
        <ButtonGroup>
          <Select>
            <SelectTrigger>
              <SelectValue placeholder="Select" />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value="all">All</SelectItem>
                <SelectItem value="unread">Unread</SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
          <ButtonGroupSeparator />
          <Button variant="outline">Export</Button>
        </ButtonGroup>
      </Demo>

      <Demo
        title="ButtonGroup · Popover"
        description="ButtonGroup can pair a popover with an action button."
        center      >
        <ButtonGroup>
          <Popover>
            <PopoverTrigger
              render={<Button variant="outline"><BotIcon /> Status</Button>}
            />
            <PopoverContent align="end" className="w-48">
              <div className="space-y-2">
                <p className="text-sm font-medium">All systems normal</p>
                <p className="text-xs text-muted-foreground">Last checked 2 min ago</p>
              </div>
            </PopoverContent>
          </Popover>
          <ButtonGroupSeparator />
          <Button variant="outline">View logs</Button>
        </ButtonGroup>
      </Demo>

      <Demo
        title="Button · As Link / Custom Element"
        description="Buttons can render as links or custom elements."
        center      >
        <Button render={<a href="#" />}>Button</Button>
        <Button variant="outline" render={<a href="#" />}>a tag</Button>
        <Button variant="link" render={<a href="#" />}>a tag</Button>
        <a href="#" className={buttonVariants({ variant: "outline" })}>Pure className call</a>
      </Demo>
      </Section>
  )
}
