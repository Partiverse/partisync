import { ArrowRight, Check, CircleFadingArrowUp } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Spinner } from "@/components/ui/spinner"
import { Demo, Section } from "../shared"

export function BadgeSection() {
  return (
<Section
        id="badge"
        title="Badge"
        group="installation"
        description="Displays a badge or a component that looks like a badge."
        fileKey="basics"
      >

      <Demo
        title="Badge · Variants"
        description="Badge supports six visual variants."
        center      >
        <Badge variant="default">Default</Badge>
        <Badge variant="secondary">Secondary</Badge>
        <Badge variant="destructive">Destructive</Badge>
        <Badge variant="outline">Outline</Badge>
        <Badge variant="ghost">Ghost</Badge>
        <Badge variant="link">Link</Badge>
      </Demo>

      <Demo
        title="Badge · With Icon"
        description="Badges can include leading or trailing icons."
        center      >
        <Badge>
          <Check data-icon="inline-start" /> Verified
        </Badge>
        <Badge variant="secondary">
          Archived <ArrowRight data-icon="inline-end" />
        </Badge>
        <Badge variant="outline">
          <CircleFadingArrowUp data-icon="inline-start" /> New
        </Badge>
      </Demo>

      <Demo
        title="Badge · With Spinner"
        description="Badges can show a spinner to indicate an in-progress state."
        center      >
        <Badge>
          <Spinner data-icon="inline-start" /> Syncing
        </Badge>
        <Badge variant="secondary">
          <Spinner data-icon="inline-start" /> Updating
        </Badge>
      </Demo>

      <Demo
        title="Badge · Link"
        description="Badges can be rendered as links."
        center      >
        <Badge render={<a href="#" />}>Link</Badge>
        <Badge variant="secondary" render={<a href="#" />}>Link</Badge>
        <Badge variant="outline" render={<a href="#" />}>Link</Badge>
      </Demo>

      <Demo
        title="Badge · Custom Colors"
        description="Badge colors can be overridden with utility classes."
        center      >
        <Badge className="bg-blue-500 text-white dark:bg-blue-600">Custom</Badge>
        <Badge className="bg-emerald-500 text-white dark:bg-emerald-600">Custom</Badge>
        <Badge className="bg-amber-500 text-white dark:bg-amber-600">Custom</Badge>
        <Badge className="bg-pink-500 text-white dark:bg-pink-600">Custom</Badge>
      </Demo>
      </Section>
  )
}
