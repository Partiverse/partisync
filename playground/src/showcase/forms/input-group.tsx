import { AtSign, Clock, Search } from "lucide-react"
import { Textarea } from "@/components/ui/textarea"
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput, InputGroupText } from "@/components/ui/input-group"
import { Spinner } from "@/components/ui/spinner"
import { Kbd } from "@/components/ui/kbd"
import { DropdownMenu, DropdownMenuContent, DropdownMenuGroup, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { Demo, Section } from "../shared"

export function InputGroupSection() {
  return (
<Section
        id="input-group"
        title="Input Group"
        group="examples"
        description="Add addons, buttons, and helper content to inputs."
        fileKey="forms"
      >
      {/* ============ InputGroup · Official H2 demos ============ */}
      <Demo
        title="InputGroup · Composition"
        description="InputGroup combines addons, input, and shortcuts."      >
        <InputGroup>
          <InputGroupAddon align="inline-start">
            <InputGroupText>
              <Search />
            </InputGroupText>
          </InputGroupAddon>
          <InputGroupInput placeholder="Search..." />
          <InputGroupAddon align="inline-end">
            <Kbd>⌘K</Kbd>
          </InputGroupAddon>
        </InputGroup>
      </Demo>

      <Demo
        title="InputGroup · Align (Prefix / Suffix)"
        description="Addons can align to the inline start and end of the input."      >
        <InputGroup>
          <InputGroupAddon align="inline-start">
            <InputGroupText>https://</InputGroupText>
          </InputGroupAddon>
          <InputGroupInput placeholder="example.com" />
          <InputGroupAddon align="inline-end">
            <InputGroupText className="text-muted-foreground">.app</InputGroupText>
          </InputGroupAddon>
        </InputGroup>
      </Demo>

      <Demo
        title="InputGroup · Icon"
        description="InputGroupAddon can render an icon."      >
        <InputGroup>
          <InputGroupAddon align="inline-start">
            <InputGroupText>
              <AtSign />
            </InputGroupText>
          </InputGroupAddon>
          <InputGroupInput placeholder="username" />
        </InputGroup>
      </Demo>

      <Demo
        title="InputGroup · Text"
        description="InputGroupAddon can render plain text on both sides."      >
        <InputGroup>
          <InputGroupAddon align="inline-start">
            <InputGroupText>From</InputGroupText>
          </InputGroupAddon>
          <InputGroupInput placeholder="0" />
          <InputGroupAddon align="inline-end">
            <InputGroupText>USD</InputGroupText>
          </InputGroupAddon>
        </InputGroup>
      </Demo>

      <Demo
        title="InputGroup · Button"
        description="InputGroupButton renders an action inside the input."      >
        <InputGroup>
          <InputGroupInput placeholder="partisync.app" />
          <InputGroupAddon align="inline-end">
            <InputGroupButton variant="default">Save</InputGroupButton>
          </InputGroupAddon>
        </InputGroup>
      </Demo>

      <Demo
        title="InputGroup · Kbd"
        description="InputGroupAddon can render keyboard shortcuts."      >
        <InputGroup>
          <InputGroupInput placeholder="Search..." />
          <InputGroupAddon align="inline-end">
            <Kbd>⌘</Kbd>
            <Kbd>K</Kbd>
          </InputGroupAddon>
        </InputGroup>
      </Demo>

      <Demo
        title="InputGroup · Dropdown"
        description="InputGroupAddon can host a dropdown menu."      >
        <InputGroup>
          <InputGroupInput placeholder="Search all files..." />
          <InputGroupAddon align="inline-end">
            <DropdownMenu>
              <DropdownMenuTrigger render={<InputGroupButton variant="ghost" aria-label="Filter" />}>
                <Search />
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-40">
                <DropdownMenuGroup>
                  <DropdownMenuItem>All files</DropdownMenuItem>
                  <DropdownMenuItem>Documents</DropdownMenuItem>
                  <DropdownMenuItem>Images</DropdownMenuItem>
                </DropdownMenuGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          </InputGroupAddon>
        </InputGroup>
      </Demo>

      <Demo
        title="InputGroup · Spinner"
        description="InputGroupAddon can show a spinner while validating."      >
        <InputGroup>
          <InputGroupInput placeholder="Validating..." defaultValue="checking-domain" />
          <InputGroupAddon align="inline-end">
            <Spinner />
          </InputGroupAddon>
        </InputGroup>
      </Demo>

      <Demo
        title="InputGroup · Textarea"
        description="InputGroup can wrap a textarea with a footer addon."      >
        <InputGroup>
          <Textarea placeholder="Write a comment..." rows={3} className="border-0 focus-visible:ring-0" />
          <InputGroupAddon align="block-end">
            <InputGroupText className="text-muted-foreground">0 / 280</InputGroupText>
            <InputGroupButton variant="default" className="ml-auto">Post</InputGroupButton>
          </InputGroupAddon>
        </InputGroup>
      </Demo>

      <Demo
        title="InputGroup · Custom Input"
        description="InputGroup works with native input types like number."      >
        <InputGroup>
          <InputGroupInput placeholder="Time" type="number" min={1} max={24} defaultValue={9} />
          <InputGroupAddon align="inline-end">
            <InputGroupText>
              <Clock /> hours
            </InputGroupText>
          </InputGroupAddon>
        </InputGroup>
      </Demo>

      </Section>
  )
}
