import { Button } from "@/components/ui/button"
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
import { Badge } from "@/components/ui/badge"
import { Spinner } from "@/components/ui/spinner"
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import { Demo, Section } from "../shared"

export function SpinnerSection() {
  return (
<Section
        id="spinner"
        title="Spinner"
        group="examples"
        description="An indicator that can be used to show a loading state."
        fileKey="basics"
      >

      <Demo
        title="Spinner · Customization"
        description="Spinner color can be customized with utility classes."      >
        <Spinner />
        <Spinner className="text-muted-foreground" />
      </Demo>

      <Demo
        title="Spinner · Size"
        description="Spinner size can be adjusted with utility classes."
        center      >
        <Spinner className="size-3" />
        <Spinner className="size-4" />
        <Spinner className="size-6" />
        <Spinner className="size-8" />
      </Demo>

      <Demo
        title="Spinner · Button"
        description="Buttons can show a spinner while an action is pending."      >
        <Button disabled size="sm">
          <Spinner data-icon="inline-start" /> Loading...
        </Button>
        <Button disabled variant="outline" size="sm">
          <Spinner data-icon="inline-start" /> Please wait
        </Button>
      </Demo>

      <Demo
        title="Spinner · Badge"
        description="Badges can show a spinner while a task is syncing."
        center      >
        <Badge>
          <Spinner data-icon="inline-start" /> Syncing
        </Badge>
        <Badge variant="secondary">
          <Spinner data-icon="inline-start" /> Updating
        </Badge>
      </Demo>

      <Demo
        title="Spinner · Input Group"
        description="InputGroup can show a spinner as an addon."      >
        <InputGroup>
          <InputGroupInput placeholder="Search..." />
          <InputGroupAddon>
            <Spinner />
          </InputGroupAddon>
        </InputGroup>
      </Demo>

      <Demo
        title="Spinner · Empty"
        description="Empty can show a spinner for a loading state."
        center      >
        <Empty className="w-full border-none">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <Spinner />
            </EmptyMedia>
            <EmptyTitle>Loading...</EmptyTitle>
            <EmptyDescription>Please wait while we load your data.</EmptyDescription>
          </EmptyHeader>
        </Empty>
      </Demo>
      </Section>
  )
}
