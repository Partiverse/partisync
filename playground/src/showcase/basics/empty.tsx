import { Cloud } from "lucide-react"
import { Button } from "@/components/ui/button"
import { InputGroup, InputGroupInput } from "@/components/ui/input-group"
import { Avatar, AvatarFallback, AvatarGroup, AvatarImage } from "@/components/ui/avatar"
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import { Demo, Section } from "../shared"

export function EmptySection() {
  return (
<Section
        id="empty"
        title="Empty"
        group="examples"
        description="Use the Empty component to display an empty state."
        fileKey="basics"
      >

      <Demo
        title="Empty · Composition"
        description="Empty combines media, title, description, and actions."      >
        <Empty className="w-full border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <Cloud />
            </EmptyMedia>
            <EmptyTitle>No projects yet</EmptyTitle>
            <EmptyDescription>Create your first project or start from a template.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <div className="flex gap-2">
              <Button size="sm">New project</Button>
              <Button size="sm" variant="outline">Browse templates</Button>
            </div>
          </EmptyContent>
        </Empty>
      </Demo>

      <Demo
        title="Empty · Outline"
        description="Empty supports a dashed outline style."      >
        <Empty className="w-full border-2 border-dashed">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <Cloud />
            </EmptyMedia>
            <EmptyTitle>No projects yet</EmptyTitle>
            <EmptyDescription>Create your first project or start from a template.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button size="sm">New project</Button>
          </EmptyContent>
        </Empty>
      </Demo>

      <Demo
        title="Empty · Background"
        description="Empty can be rendered on a muted background."      >
        <Empty className="w-full bg-muted">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <Cloud />
            </EmptyMedia>
            <EmptyTitle>No data</EmptyTitle>
            <EmptyDescription>There is no data to display at this time.</EmptyDescription>
          </EmptyHeader>
        </Empty>
      </Demo>

      <Demo
        title="Empty · Avatar"
        description="Empty can use an avatar as its media."
        center      >
        <Empty className="w-full border-none">
          <EmptyHeader>
            <EmptyMedia>
              <Avatar>
                <AvatarImage src="https://github.com/shadcn.png" alt="@shadcn" />
                <AvatarFallback>CN</AvatarFallback>
              </Avatar>
            </EmptyMedia>
            <EmptyTitle>No users found</EmptyTitle>
            <EmptyDescription>Invite your first user to get started.</EmptyDescription>
          </EmptyHeader>
        </Empty>
      </Demo>

      <Demo
        title="Empty · Avatar Group"
        description="Empty can use an avatar group as its media."
        center      >
        <Empty className="w-full border-none">
          <EmptyHeader>
            <EmptyMedia>
              <AvatarGroup>
                <Avatar><AvatarFallback>U1</AvatarFallback></Avatar>
                <Avatar><AvatarFallback>U2</AvatarFallback></Avatar>
                <Avatar><AvatarFallback>U3</AvatarFallback></Avatar>
              </AvatarGroup>
            </EmptyMedia>
            <EmptyTitle>No teammates</EmptyTitle>
            <EmptyDescription>Invite people to collaborate with.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button size="sm">Invite</Button>
          </EmptyContent>
        </Empty>
      </Demo>

      <Demo
        title="Empty · InputGroup"
        description="Empty can include an input group for search refinement."
        center      >
        <Empty className="w-full border-none">
          <EmptyHeader>
            <EmptyTitle>No results</EmptyTitle>
            <EmptyDescription>Try adjusting your search or filters.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <InputGroup>
              <InputGroupInput placeholder="Search..." />
            </InputGroup>
          </EmptyContent>
        </Empty>
      </Demo>
      </Section>
  )
}
