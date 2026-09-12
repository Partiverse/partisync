import * as React from "react"
import {
  IconGitBranch,
  IconGitFork,
} from "@tabler/icons-react"
import {
  ArchiveIcon,
  ArrowLeftIcon,
  ArrowUpIcon,
  ArrowUpRightIcon,
  CalendarPlusIcon,
  CircleFadingArrowUpIcon,
  ClockIcon,
  ListFilterIcon,
  MailCheckIcon,
  MoreHorizontalIcon,
  TagIcon,
  Trash2Icon,
} from "lucide-react"
import { Button, buttonVariants } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { ButtonGroup } from "@/components/ui/button-group"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Demo, Section } from "../shared"

export function ButtonSection() {
  const [label, setLabel] = React.useState("personal")

  return (
    <>
      {/* 官方页头：H1 + 描述 */}
      <div className="flex flex-col gap-2">
        <h1 className="text-3xl font-semibold tracking-tight">Button</h1>
        <p>Displays a button or a component that looks like a button.</p>
      </div>

      {/* ========== Usage ========== */}
      <Section id="usage" title="Usage" group="usage" fileKey="basics">
        <Demo title="Button · Usage" docs center>
          <div className="flex flex-wrap items-center gap-2 md:flex-row">
            <Button variant="outline">Button</Button>
            <Button variant="outline" size="icon" aria-label="Submit">
              <ArrowUpIcon />
            </Button>
          </div>
        </Demo>
      </Section>

      {/* ========== Size ========== */}
      <Section
        id="size"
        title="Size"
        group="examples"
        description="Use the size prop to change the size of the button."
        fileKey="basics"
      >
        <Demo title="Button · Size" docs center>
          <div className="flex flex-col items-start gap-8 sm:flex-row">
            <div className="flex items-start gap-2">
              <Button size="xs" variant="outline">
                Extra Small
              </Button>
              <Button size="icon-xs" aria-label="Submit" variant="outline">
                <ArrowUpRightIcon />
              </Button>
            </div>
            <div className="flex items-start gap-2">
              <Button size="sm" variant="outline">
                Small
              </Button>
              <Button size="icon-sm" aria-label="Submit" variant="outline">
                <ArrowUpRightIcon />
              </Button>
            </div>
            <div className="flex items-start gap-2">
              <Button variant="outline">Default</Button>
              <Button size="icon" aria-label="Submit" variant="outline">
                <ArrowUpRightIcon />
              </Button>
            </div>
            <div className="flex items-start gap-2">
              <Button variant="outline" size="lg">
                Large
              </Button>
              <Button size="icon-lg" aria-label="Submit" variant="outline">
                <ArrowUpRightIcon />
              </Button>
            </div>
          </div>
        </Demo>
      </Section>

      {/* ========== Default ========== */}
      <Section id="default" title="Default" group="examples" fileKey="basics">
        <Demo title="Button · Default" docs center>
          <Button>Button</Button>
        </Demo>
      </Section>

      {/* ========== Outline ========== */}
      <Section id="outline" title="Outline" group="examples" fileKey="basics">
        <Demo title="Button · Outline" docs center>
          <Button variant="outline">Outline</Button>
        </Demo>
      </Section>

      {/* ========== Secondary ========== */}
      <Section id="secondary" title="Secondary" group="examples" fileKey="basics">
        <Demo title="Button · Secondary" docs center>
          <Button variant="secondary">Secondary</Button>
        </Demo>
      </Section>

      {/* ========== Ghost ========== */}
      <Section id="ghost" title="Ghost" group="examples" fileKey="basics">
        <Demo title="Button · Ghost" docs center>
          <Button variant="ghost">Ghost</Button>
        </Demo>
      </Section>

      {/* ========== Destructive ========== */}
      <Section id="destructive" title="Destructive" group="examples" fileKey="basics">
        <Demo title="Button · Destructive" docs center>
          <Button variant="destructive">Destructive</Button>
        </Demo>
      </Section>

      {/* ========== Link ========== */}
      <Section id="link" title="Link" group="examples" fileKey="basics">
        <Demo title="Button · Link" docs center>
          <Button variant="link">Link</Button>
        </Demo>
      </Section>

      {/* ========== Icon ========== */}
      <Section id="icon" title="Icon" group="examples" fileKey="basics">
        <Demo title="Button · Icon" docs center>
          <Button variant="outline" size="icon">
            <CircleFadingArrowUpIcon />
          </Button>
        </Demo>
      </Section>

      {/* ========== With Icon ========== */}
      <Section
        id="with-icon"
        title="With Icon"
        group="examples"
        description='Remember to add the data-icon="inline-start" or data-icon="inline-end" attribute to the icon for the correct spacing.'
        fileKey="basics"
      >
        <Demo title="Button · With Icon" docs center>
          <div className="flex gap-2">
            <Button variant="outline">
              <IconGitBranch data-icon="inline-start" /> New Branch
            </Button>
            <Button variant="outline">
              Fork <IconGitFork data-icon="inline-end" />
            </Button>
          </div>
        </Demo>
      </Section>

      {/* ========== Rounded ========== */}
      <Section
        id="rounded"
        title="Rounded"
        group="examples"
        description="Use the rounded-full class to make the button rounded."
        fileKey="basics"
      >
        <Demo title="Button · Rounded" docs center>
          <div className="flex gap-2">
            <Button className="rounded-full">Get Started</Button>
            <Button variant="outline" size="icon" className="rounded-full">
              <ArrowUpIcon />
            </Button>
          </div>
        </Demo>
      </Section>

      {/* ========== Spinner ========== */}
      <Section
        id="spinner"
        title="Spinner"
        group="examples"
        description='Render a Spinner component inside the button to show a loading state. Remember to add the data-icon="inline-start" or data-icon="inline-end" attribute to the spinner for the correct spacing.'
        fileKey="basics"
      >
        <Demo title="Button · Spinner" docs center>
          <div className="flex gap-2">
            <Button variant="outline" disabled>
              <Spinner data-icon="inline-start" /> Generating
            </Button>
            <Button variant="secondary" disabled>
              Downloading <Spinner data-icon="inline-start" />
            </Button>
          </div>
        </Demo>
      </Section>

      {/* ========== Button Group ========== */}
      <Section
        id="button-group"
        title="Button Group"
        group="examples"
        description="To create a button group, use the ButtonGroup component. See the Button Group documentation for more details."
        fileKey="basics"
      >
        <Demo title="Button · Button Group" docs center>
          <ButtonGroup>
            <ButtonGroup className="hidden sm:flex">
              <Button variant="outline" size="icon" aria-label="Go Back">
                <ArrowLeftIcon />
              </Button>
            </ButtonGroup>
            <ButtonGroup>
              <Button variant="outline">Archive</Button>
              <Button variant="outline">Report</Button>
            </ButtonGroup>
            <ButtonGroup>
              <Button variant="outline">Snooze</Button>
              <DropdownMenu>
                <DropdownMenuTrigger
                  render={
                    <Button variant="outline" size="icon" aria-label="More Options">
                      <MoreHorizontalIcon />
                    </Button>
                  }
                />
                <DropdownMenuContent align="end" className="w-40">
                  <DropdownMenuGroup>
                    <DropdownMenuItem>
                      <MailCheckIcon />
                      Mark as Read
                    </DropdownMenuItem>
                    <DropdownMenuItem>
                      <ArchiveIcon />
                      Archive
                    </DropdownMenuItem>
                  </DropdownMenuGroup>
                  <DropdownMenuSeparator />
                  <DropdownMenuGroup>
                    <DropdownMenuItem>
                      <ClockIcon />
                      Snooze
                    </DropdownMenuItem>
                    <DropdownMenuItem>
                      <CalendarPlusIcon />
                      Add to Calendar
                    </DropdownMenuItem>
                    <DropdownMenuItem>
                      <ListFilterIcon />
                      Add to List
                    </DropdownMenuItem>
                    <DropdownMenuSub>
                      <DropdownMenuSubTrigger>
                        <TagIcon />
                        Label As...
                      </DropdownMenuSubTrigger>
                      <DropdownMenuSubContent>
                        <DropdownMenuRadioGroup
                          value={label}
                          onValueChange={setLabel}
                        >
                          <DropdownMenuRadioItem value="personal">
                            Personal
                          </DropdownMenuRadioItem>
                          <DropdownMenuRadioItem value="work">
                            Work
                          </DropdownMenuRadioItem>
                          <DropdownMenuRadioItem value="other">
                            Other
                          </DropdownMenuRadioItem>
                        </DropdownMenuRadioGroup>
                      </DropdownMenuSubContent>
                    </DropdownMenuSub>
                  </DropdownMenuGroup>
                  <DropdownMenuSeparator />
                  <DropdownMenuGroup>
                    <DropdownMenuItem variant="destructive">
                      <Trash2Icon />
                      Trash
                    </DropdownMenuItem>
                  </DropdownMenuGroup>
                </DropdownMenuContent>
              </DropdownMenu>
            </ButtonGroup>
          </ButtonGroup>
        </Demo>
      </Section>

      {/* ========== As Link ========== */}
      <Section
        id="as-link"
        title="As Link"
        group="examples"
        description="You can use the buttonVariants helper to make a link look like a button."
        fileKey="basics"
      >
        <Demo title="Button · As Link" docs center>
          <a
            href="#"
            className={buttonVariants({ variant: "secondary", size: "sm" })}
          >
            Login
          </a>
        </Demo>
      </Section>

      {/* ========== API Reference ========== */}
      <Section id="api-reference" title="API Reference" group="api-reference" fileKey="basics">
        <div className="flex flex-col gap-4">
          <h3 className="text-[1.125em] font-semibold leading-[1.45]">Button</h3>
          <p className="text-muted-foreground">
            The{" "}
            <code className="rounded-[min(calc(var(--radius)*0.6),0.35em)] border bg-muted px-0.5 py-px font-mono text-[0.85em]">Button</code>{" "}
            component is a wrapper around the{" "}
            <code className="rounded-[min(calc(var(--radius)*0.6),0.35em)] border bg-muted px-0.5 py-px font-mono text-[0.85em]">button</code>{" "}
            element that adds a variety of styles and functionality.
          </p>
          {/* 官方 typeset 表格：无外框，thead th .65em/1em，td border-top 分隔，首列无左 padding */}
          <div className="typeset-scroll w-full overflow-x-auto">
            <table className="w-full max-w-full border-collapse border-b border-border text-left [font-variant-numeric:tabular-nums]">
              <thead>
                <tr>
                  <th className="whitespace-nowrap py-[0.65em] pr-[1em] pl-0 text-start font-medium">
                    Prop
                  </th>
                  <th className="whitespace-nowrap px-[1em] py-[0.65em] text-start font-medium">
                    Type
                  </th>
                  <th className="whitespace-nowrap px-[1em] py-[0.65em] text-start font-medium">
                    Default
                  </th>
                </tr>
              </thead>
              <tbody>
                <tr className="border-t border-border">
                  <td className="py-[0.75em] pr-[1em] pl-0 align-top font-mono text-[0.85em]">
                    variant
                  </td>
                  <td className="px-[1em] py-[0.75em] align-top font-mono text-[0.85em] text-muted-foreground">
                    &quot;default&quot; | &quot;outline&quot; | &quot;ghost&quot; | &quot;destructive&quot; | &quot;secondary&quot; | &quot;link&quot;
                  </td>
                  <td className="px-[1em] py-[0.75em] align-top font-mono text-[0.85em] text-muted-foreground">
                    &quot;default&quot;
                  </td>
                </tr>
                <tr className="border-t border-border">
                  <td className="py-[0.75em] pr-[1em] pl-0 align-top font-mono text-[0.85em]">
                    size
                  </td>
                  <td className="px-[1em] py-[0.75em] align-top font-mono text-[0.85em] text-muted-foreground">
                    &quot;default&quot; | &quot;xs&quot; | &quot;sm&quot; | &quot;lg&quot; | &quot;icon&quot; | &quot;icon-xs&quot; | &quot;icon-sm&quot; | &quot;icon-lg&quot;
                  </td>
                  <td className="px-[1em] py-[0.75em] align-top font-mono text-[0.85em] text-muted-foreground">
                    &quot;default&quot;
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </Section>
    </>
  )
}
